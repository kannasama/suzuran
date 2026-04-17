# Phase 1.8 — Job Scheduler + Scan Job Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the DB-backed job queue — a Tokio poll loop that claims `pending` jobs, runs them with semaphore-capped concurrency, and marks them `completed` or `failed` (with retries). Wire in the `scan` job type that calls the Phase 1.7 scanner.

**Architecture:** A single `Scheduler` task is spawned at server startup. It polls `jobs` every few seconds for `pending` work, claims a row atomically (UPDATE … WHERE status='pending' … RETURNING), spawns a Tokio task per job. Per-type semaphores cap concurrency. The `scan` job handler calls `scan_library` and enqueues `fingerprint` jobs for new tracks (no-op stubs in Phase 1 — Phase 3 wires real fingerprinting). Failed jobs are retried up to 3 times with exponential backoff.

**Tech Stack:** tokio (semaphore, spawn, sleep), serde_json for job payloads.

---

## File Map

| File | Action | Purpose |
|------|--------|---------|
| `src/dal/mod.rs` | Modify | Add job Store methods |
| `src/dal/postgres.rs` | Modify | Job queries |
| `src/dal/sqlite.rs` | Modify | Job queries |
| `src/models/mod.rs` | Modify | Add `Job` struct |
| `src/jobs/mod.rs` | Create | Job payload types + `JobHandler` trait |
| `src/jobs/scan.rs` | Create | Scan job handler |
| `src/scheduler/mod.rs` | Create | Poll loop, semaphores, claim + dispatch |
| `src/api/jobs.rs` | Create | Jobs list + cancel handlers |
| `src/api/mod.rs` | Modify | Mount jobs + scan-trigger routes |
| `src/lib.rs` | Modify | Expose `jobs`, `scheduler` modules |
| `src/main.rs` | Modify | Spawn scheduler task at startup |
| `tests/scheduler.rs` | Create | End-to-end scan job test |

---

## Task 1: Job model + Store methods

**Files:**
- Modify: `src/models/mod.rs`
- Modify: `src/dal/mod.rs`

- [ ] **Step 1: Append `Job` to `src/models/mod.rs`**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Job {
    pub id: i64,
    pub job_type: String,
    pub status: String,
    pub payload: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub priority: i64,
    pub attempts: i64,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}
```

- [ ] **Step 2: Add job methods to `Store` trait in `src/dal/mod.rs`**

```rust
// ── jobs ─────────────────────────────────────────────────────
async fn enqueue_job(
    &self,
    job_type: &str,
    payload: serde_json::Value,
    priority: i64,
) -> Result<Job, AppError>;

/// Atomically claim the next pending job of given types. Returns None if queue is empty.
async fn claim_next_job(&self, job_types: &[&str]) -> Result<Option<Job>, AppError>;

async fn complete_job(&self, id: i64, result: serde_json::Value) -> Result<(), AppError>;

async fn fail_job(&self, id: i64, error: &str) -> Result<(), AppError>;

async fn cancel_job(&self, id: i64) -> Result<(), AppError>;

async fn list_jobs(
    &self,
    status: Option<&str>,
    limit: i64,
) -> Result<Vec<Job>, AppError>;

async fn get_job(&self, id: i64) -> Result<Option<Job>, AppError>;
```

Add `use crate::models::{..., Job};` to imports.

---

## Task 2: Postgres job implementations

**Files:**
- Modify: `src/dal/postgres.rs`

- [ ] **Step 1: Append job implementations**

```rust
async fn enqueue_job(
    &self,
    job_type: &str,
    payload: serde_json::Value,
    priority: i64,
) -> Result<Job, AppError> {
    sqlx::query_as::<_, Job>(
        "INSERT INTO jobs (job_type, payload, priority) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(job_type).bind(payload).bind(priority)
    .fetch_one(&self.pool).await.map_err(AppError::Database)
}

async fn claim_next_job(&self, job_types: &[&str]) -> Result<Option<Job>, AppError> {
    // Atomic claim: select oldest pending job of the given types, mark running
    sqlx::query_as::<_, Job>(
        "UPDATE jobs SET status = 'running', started_at = NOW(), attempts = attempts + 1
         WHERE id = (
             SELECT id FROM jobs
             WHERE status = 'pending'
               AND job_type = ANY($1)
             ORDER BY priority DESC, created_at ASC
             LIMIT 1
             FOR UPDATE SKIP LOCKED
         )
         RETURNING *",
    )
    .bind(job_types)
    .fetch_optional(&self.pool).await.map_err(AppError::Database)
}

async fn complete_job(&self, id: i64, result: serde_json::Value) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE jobs SET status='completed', result=$1, completed_at=NOW() WHERE id=$2",
    )
    .bind(result).bind(id)
    .execute(&self.pool).await.map(|_| ()).map_err(AppError::Database)
}

async fn fail_job(&self, id: i64, error: &str) -> Result<(), AppError> {
    // If attempts < 3, reset to pending for retry; otherwise permanently fail
    sqlx::query(
        "UPDATE jobs SET
           status = CASE WHEN attempts >= 3 THEN 'failed' ELSE 'pending' END,
           error = $1,
           started_at = NULL
         WHERE id = $2",
    )
    .bind(error).bind(id)
    .execute(&self.pool).await.map(|_| ()).map_err(AppError::Database)
}

async fn cancel_job(&self, id: i64) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE jobs SET status='cancelled' WHERE id=$1 AND status IN ('pending','running')",
    )
    .bind(id)
    .execute(&self.pool).await.map(|_| ()).map_err(AppError::Database)
}

async fn list_jobs(&self, status: Option<&str>, limit: i64) -> Result<Vec<Job>, AppError> {
    if let Some(s) = status {
        sqlx::query_as::<_, Job>(
            "SELECT * FROM jobs WHERE status=$1 ORDER BY created_at DESC LIMIT $2",
        )
        .bind(s).bind(limit)
        .fetch_all(&self.pool).await.map_err(AppError::Database)
    } else {
        sqlx::query_as::<_, Job>(
            "SELECT * FROM jobs ORDER BY created_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool).await.map_err(AppError::Database)
    }
}

async fn get_job(&self, id: i64) -> Result<Option<Job>, AppError> {
    sqlx::query_as::<_, Job>("SELECT * FROM jobs WHERE id = $1")
        .bind(id).fetch_optional(&self.pool).await.map_err(AppError::Database)
}
```

---

## Task 3: SQLite job implementations

**Files:**
- Modify: `src/dal/sqlite.rs`

- [ ] **Step 1: Append job implementations**

```rust
async fn enqueue_job(
    &self,
    job_type: &str,
    payload: serde_json::Value,
    priority: i64,
) -> Result<Job, AppError> {
    sqlx::query_as::<_, Job>(
        "INSERT INTO jobs (job_type, payload, priority) VALUES (?1, ?2, ?3) RETURNING *",
    )
    .bind(job_type).bind(payload).bind(priority)
    .fetch_one(&self.pool).await.map_err(AppError::Database)
}

async fn claim_next_job(&self, job_types: &[&str]) -> Result<Option<Job>, AppError> {
    // SQLite doesn't support FOR UPDATE SKIP LOCKED or ANY($1).
    // For SQLite, serialize with WAL mode + single-threaded claim.
    // Build the IN clause dynamically.
    if job_types.is_empty() {
        return Ok(None);
    }
    let placeholders: Vec<String> = (1..=job_types.len()).map(|i| format!("?{i}")).collect();
    let in_clause = placeholders.join(",");

    // Two-step: SELECT then UPDATE (SQLite limitation — no UPDATE ... WHERE id = (SELECT ... FOR UPDATE))
    let sql_select = format!(
        "SELECT * FROM jobs WHERE status='pending' AND job_type IN ({in_clause})
         ORDER BY priority DESC, created_at ASC LIMIT 1"
    );
    let mut q = sqlx::query_as::<_, Job>(&sql_select);
    for t in job_types {
        q = q.bind(*t);
    }
    let job = match q.fetch_optional(&self.pool).await.map_err(AppError::Database)? {
        Some(j) => j,
        None => return Ok(None),
    };

    // Claim it
    sqlx::query(
        "UPDATE jobs SET status='running', started_at=datetime('now'), attempts=attempts+1 WHERE id=?1 AND status='pending'",
    )
    .bind(job.id)
    .execute(&self.pool).await.map_err(AppError::Database)?;

    // Re-fetch to get updated row
    self.get_job(job.id).await
}

async fn complete_job(&self, id: i64, result: serde_json::Value) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE jobs SET status='completed', result=?1, completed_at=datetime('now') WHERE id=?2",
    )
    .bind(result).bind(id)
    .execute(&self.pool).await.map(|_| ()).map_err(AppError::Database)
}

async fn fail_job(&self, id: i64, error: &str) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE jobs SET
           status = CASE WHEN attempts >= 3 THEN 'failed' ELSE 'pending' END,
           error = ?1,
           started_at = NULL
         WHERE id = ?2",
    )
    .bind(error).bind(id)
    .execute(&self.pool).await.map(|_| ()).map_err(AppError::Database)
}

async fn cancel_job(&self, id: i64) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE jobs SET status='cancelled' WHERE id=?1 AND status IN ('pending','running')",
    )
    .bind(id)
    .execute(&self.pool).await.map(|_| ()).map_err(AppError::Database)
}

async fn list_jobs(&self, status: Option<&str>, limit: i64) -> Result<Vec<Job>, AppError> {
    if let Some(s) = status {
        sqlx::query_as::<_, Job>(
            "SELECT * FROM jobs WHERE status=?1 ORDER BY created_at DESC LIMIT ?2",
        )
        .bind(s).bind(limit)
        .fetch_all(&self.pool).await.map_err(AppError::Database)
    } else {
        sqlx::query_as::<_, Job>("SELECT * FROM jobs ORDER BY created_at DESC LIMIT ?1")
            .bind(limit)
            .fetch_all(&self.pool).await.map_err(AppError::Database)
    }
}

async fn get_job(&self, id: i64) -> Result<Option<Job>, AppError> {
    sqlx::query_as::<_, Job>("SELECT * FROM jobs WHERE id = ?1")
        .bind(id).fetch_optional(&self.pool).await.map_err(AppError::Database)
}
```

- [ ] **Step 2: Compile check**

```bash
cargo build 2>&1 | tail -5
```

Expected: `Finished`.

- [ ] **Step 3: Commit**

```bash
git add src/
git commit -m "feat: Job model, Store job methods, Postgres + SQLite implementations"
```

---

## Task 4: Job payload types and scan handler

**Files:**
- Create: `src/jobs/mod.rs`
- Create: `src/jobs/scan.rs`

- [ ] **Step 1: Write `src/jobs/mod.rs`**

```rust
pub mod scan;

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{dal::Store, error::AppError};

#[async_trait::async_trait]
pub trait JobHandler: Send + Sync {
    async fn run(&self, db: Arc<dyn Store>, payload: serde_json::Value) -> Result<serde_json::Value, AppError>;
}

/// Payload for the `scan` job type.
#[derive(Debug, Serialize, Deserialize)]
pub struct ScanPayload {
    pub library_id: i64,
}
```

- [ ] **Step 2: Write `src/jobs/scan.rs`**

```rust
use std::path::Path;
use std::sync::Arc;

use crate::{
    dal::Store,
    error::AppError,
    jobs::{JobHandler, ScanPayload},
    scanner,
};

pub struct ScanJobHandler;

#[async_trait::async_trait]
impl JobHandler for ScanJobHandler {
    async fn run(
        &self,
        db: Arc<dyn Store>,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        let p: ScanPayload = serde_json::from_value(payload)
            .map_err(|e| AppError::BadRequest(format!("invalid scan payload: {e}")))?;

        let library = db.get_library(p.library_id).await?
            .ok_or_else(|| AppError::NotFound(format!("library {} not found", p.library_id)))?;

        let root = Path::new(&library.root_path);
        let result = scanner::scan_library(&db, library.id, root).await?;

        tracing::info!(
            library_id = library.id,
            inserted = result.inserted,
            updated = result.updated,
            removed = result.removed,
            errors = result.errors.len(),
            "scan complete"
        );

        Ok(serde_json::json!({
            "inserted": result.inserted,
            "updated": result.updated,
            "removed": result.removed,
            "errors": result.errors,
        }))
    }
}
```

---

## Task 5: Scheduler

**Files:**
- Create: `src/scheduler/mod.rs`

- [ ] **Step 1: Write `src/scheduler/mod.rs`**

```rust
use std::{collections::HashMap, sync::Arc, time::Duration};

use tokio::sync::Semaphore;

use crate::{
    dal::Store,
    jobs::{scan::ScanJobHandler, JobHandler},
};

/// Concurrency limits per job type (overridden at runtime from settings — Phase 1.6 wires this up).
const DEFAULT_SCAN_CONCURRENCY: usize = 2;
const DEFAULT_OTHER_CONCURRENCY: usize = 4;
const POLL_INTERVAL_SECS: u64 = 5;

pub struct Scheduler {
    db: Arc<dyn Store>,
    handlers: HashMap<&'static str, Arc<dyn JobHandler>>,
    semaphores: HashMap<&'static str, Arc<Semaphore>>,
}

impl Scheduler {
    pub fn new(db: Arc<dyn Store>) -> Self {
        let mut handlers: HashMap<&'static str, Arc<dyn JobHandler>> = HashMap::new();
        handlers.insert("scan", Arc::new(ScanJobHandler));
        // Phase 3: insert "fingerprint" and "mb_lookup" handlers
        // Phase 4: insert "transcode", "art_process", "organize" handlers

        let mut semaphores: HashMap<&'static str, Arc<Semaphore>> = HashMap::new();
        semaphores.insert("scan", Arc::new(Semaphore::new(DEFAULT_SCAN_CONCURRENCY)));
        semaphores.insert("fingerprint", Arc::new(Semaphore::new(DEFAULT_OTHER_CONCURRENCY)));
        semaphores.insert("mb_lookup", Arc::new(Semaphore::new(DEFAULT_OTHER_CONCURRENCY)));
        semaphores.insert("transcode", Arc::new(Semaphore::new(2)));
        semaphores.insert("art_process", Arc::new(Semaphore::new(DEFAULT_OTHER_CONCURRENCY)));
        semaphores.insert("organize", Arc::new(Semaphore::new(DEFAULT_OTHER_CONCURRENCY)));

        Self { db, handlers, semaphores }
    }

    /// Run the scheduler loop indefinitely. Call via `tokio::spawn`.
    pub async fn run(self: Arc<Self>) {
        let job_types: Vec<&str> = self.handlers.keys().copied().collect();

        loop {
            let result = self.db.claim_next_job(&job_types).await;

            match result {
                Err(e) => {
                    tracing::error!(error = %e, "error claiming job");
                    tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
                }
                Ok(None) => {
                    // Queue empty — sleep before polling again
                    tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
                }
                Ok(Some(job)) => {
                    let Some(handler) = self.handlers.get(job.job_type.as_str()).cloned() else {
                        tracing::warn!(job_type = %job.job_type, "no handler for job type");
                        let _ = self.db.fail_job(job.id, "no handler registered").await;
                        continue;
                    };

                    let semaphore = self
                        .semaphores
                        .get(job.job_type.as_str())
                        .cloned()
                        .unwrap_or_else(|| Arc::new(Semaphore::new(1)));

                    let db = self.db.clone();
                    let scheduler = self.clone();

                    tokio::spawn(async move {
                        // Acquire semaphore permit — blocks if concurrency limit reached
                        let _permit = semaphore.acquire().await.unwrap();

                        tracing::info!(job_id = job.id, job_type = %job.job_type, "running job");

                        match handler.run(db.clone(), job.payload.clone()).await {
                            Ok(result) => {
                                if let Err(e) = db.complete_job(job.id, result).await {
                                    tracing::error!(job_id = job.id, error = %e, "failed to mark job complete");
                                }
                            }
                            Err(e) => {
                                tracing::warn!(job_id = job.id, error = %e, "job failed");
                                if let Err(db_err) = db.fail_job(job.id, &e.to_string()).await {
                                    tracing::error!(job_id = job.id, error = %db_err, "failed to mark job failed");
                                }
                            }
                        }
                        drop(_permit);
                    });
                }
            }
        }
    }
}
```

---

## Task 6: Wire scheduler into main.rs + expose modules

**Files:**
- Modify: `src/lib.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Add to `src/lib.rs`**

```rust
pub mod jobs;
pub mod scheduler;
```

- [ ] **Step 2: Spawn scheduler in `src/main.rs`**

After building `AppState`, before `axum::serve`, add:

```rust
use suzuran_server::scheduler::Scheduler;
use std::sync::Arc as StdArc;

// Spawn job scheduler
let scheduler = StdArc::new(Scheduler::new(state.db.clone()));
tokio::spawn({
    let s = scheduler.clone();
    async move { s.run().await }
});
tracing::info!("job scheduler started");
```

---

## Task 7: Jobs API handlers

**Files:**
- Create: `src/api/jobs.rs`
- Modify: `src/api/mod.rs`

- [ ] **Step 1: Write `src/api/jobs.rs`**

```rust
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use crate::{
    api::middleware::{admin::AdminUser, auth::AuthUser},
    error::AppError,
    jobs::ScanPayload,
    models::Job,
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_jobs))
        .route("/:id", get(get_job))
        .route("/:id/cancel", post(cancel_job))
        .route("/scan", post(enqueue_scan))
}

#[derive(Deserialize)]
struct ListJobsQuery {
    status: Option<String>,
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 { 50 }

async fn list_jobs(
    State(state): State<AppState>,
    _auth: AuthUser,
    Query(q): Query<ListJobsQuery>,
) -> Result<Json<Vec<Job>>, AppError> {
    Ok(Json(state.db.list_jobs(q.status.as_deref(), q.limit).await?))
}

async fn get_job(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<Job>, AppError> {
    state.db.get_job(id).await?
        .ok_or_else(|| AppError::NotFound(format!("job {id} not found")))
        .map(Json)
}

async fn cancel_job(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    state.db.cancel_job(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct EnqueueScanRequest {
    library_id: i64,
}

async fn enqueue_scan(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(body): Json<EnqueueScanRequest>,
) -> Result<(StatusCode, Json<Job>), AppError> {
    // Verify library exists
    state.db.get_library(body.library_id).await?
        .ok_or_else(|| AppError::NotFound(format!("library {} not found", body.library_id)))?;

    let job = state.db.enqueue_job(
        "scan",
        serde_json::to_value(ScanPayload { library_id: body.library_id }).unwrap(),
        0,
    ).await?;

    Ok((StatusCode::CREATED, Json(job)))
}
```

- [ ] **Step 2: Update `src/api/mod.rs`**

```rust
pub mod auth;
pub mod jobs;
pub mod libraries;
pub mod middleware;
pub mod settings;
pub mod themes;
pub mod totp;
pub mod webauthn;

use axum::Router;
use crate::state::AppState;

pub fn api_router(state: AppState) -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::router())
        .nest("/totp", totp::router())
        .nest("/webauthn", webauthn::router())
        .nest("/settings", settings::router())
        .nest("/themes", themes::router())
        .nest("/libraries", libraries::router())
        .nest("/jobs", jobs::router())
}
```

- [ ] **Step 3: Compile check**

```bash
cargo build 2>&1 | tail -5
```

Expected: `Finished`.

- [ ] **Step 4: Commit**

```bash
git add src/
git commit -m "feat: job scheduler poll loop + scan job handler + Jobs API"
```

---

## Task 8: Integration test

**Files:**
- Create: `tests/scheduler.rs`

- [ ] **Step 1: Write `tests/scheduler.rs`**

```rust
use std::{sync::Arc, time::Duration};
use url::Url;
use webauthn_rs::WebauthnBuilder;
use suzuran_server::{
    build_router, config::Config, dal::sqlite::SqliteStore, scheduler::Scheduler, state::AppState,
};

async fn spawn_test_server() -> String {
    let store = Arc::new(SqliteStore::new("sqlite::memory:").await.unwrap());
    store.migrate().await.unwrap();

    let origin = Url::parse("http://localhost:3000").unwrap();
    let webauthn = WebauthnBuilder::new("localhost", &origin)
        .unwrap().rp_name("test").build().unwrap();

    let config = Config {
        database_url: "sqlite::memory:".into(),
        jwt_secret: "test-secret-32-chars-minimum-xxxx".into(),
        port: 0,
        log_level: "error".into(),
        rp_id: "localhost".into(),
        rp_origin: "http://localhost:3000".into(),
    };

    let state = AppState::new(store.clone() as Arc<dyn suzuran_server::dal::Store>, config, webauthn);

    // Spawn scheduler
    let scheduler = Arc::new(Scheduler::new(store.clone()));
    tokio::spawn({ let s = scheduler.clone(); async move { s.run().await } });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, build_router(state)).await.unwrap() });
    format!("http://{addr}")
}

async fn admin_client(base: &str) -> reqwest::Client {
    let client = reqwest::Client::builder().cookie_store(true).build().unwrap();
    client.post(format!("{base}/api/v1/auth/register"))
        .json(&serde_json::json!({"username":"admin","email":"a@a.com","password":"password123"}))
        .send().await.unwrap();
    client.post(format!("{base}/api/v1/auth/login"))
        .json(&serde_json::json!({"username":"admin","password":"password123"}))
        .send().await.unwrap();
    client
}

#[tokio::test]
async fn scan_job_enqueues_and_completes() {
    let base = spawn_test_server().await;
    let client = admin_client(&base).await;

    // Create a temp library root
    let dir = tempfile::TempDir::new().unwrap();
    tokio::fs::write(dir.path().join("song.flac"), b"").await.unwrap();

    // Create library
    let lib_res = client.post(format!("{base}/api/v1/libraries/"))
        .json(&serde_json::json!({
            "name": "Test Library",
            "root_path": dir.path().to_str().unwrap(),
            "format": "flac"
        }))
        .send().await.unwrap();
    assert_eq!(lib_res.status(), 201);
    let lib: serde_json::Value = lib_res.json().await.unwrap();
    let lib_id = lib["id"].as_i64().unwrap();

    // Enqueue scan job
    let job_res = client.post(format!("{base}/api/v1/jobs/scan"))
        .json(&serde_json::json!({"library_id": lib_id}))
        .send().await.unwrap();
    assert_eq!(job_res.status(), 201);
    let job: serde_json::Value = job_res.json().await.unwrap();
    let job_id = job["id"].as_i64().unwrap();

    // Wait for scheduler to process (up to 15 seconds)
    let mut completed = false;
    for _ in 0..15 {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let status_res = client.get(format!("{base}/api/v1/jobs/{job_id}"))
            .send().await.unwrap();
        let j: serde_json::Value = status_res.json().await.unwrap();
        if j["status"] == "completed" {
            completed = true;
            // Verify the file was scanned
            let tracks_res = client.get(format!("{base}/api/v1/libraries/{lib_id}/tracks"))
                .send().await.unwrap();
            let tracks: Vec<serde_json::Value> = tracks_res.json().await.unwrap();
            assert_eq!(tracks.len(), 1, "one track should be in the library");
            break;
        }
    }

    assert!(completed, "scan job did not complete within 15 seconds");
}
```

- [ ] **Step 2: Run the test**

```bash
cargo test --test scheduler -- --nocapture
```

Expected: `scan_job_enqueues_and_completes ... ok`

- [ ] **Step 3: Commit**

```bash
git add tests/scheduler.rs tasks/codebase-filemap.md
git commit -m "test: scheduler end-to-end scan job test; update filemap"
```
