# Phase 3 — MusicBrainz Integration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add acoustic fingerprinting, AcoustID/MusicBrainz metadata lookup, gnudb.org (FreeDB) disc-ID fallback, tag suggestion workflow, and an Inbox UI for reviewing and accepting suggested tag changes.

**Architecture:** Three new job types chain together: `fingerprint` (fpcalc subprocess) → `mb_lookup` (AcoustID + MusicBrainz API, enqueues `freedb_lookup` fallback if no matches) → `freedb_lookup` (gnudb.org CDDB disc-ID lookup when DISCID tag present). Suggestions land in a new `tag_suggestions` table. Accept flow merges suggested tags, writes via `lofty`, and updates the `tracks` row. The Inbox UI wires it all together with a diff view, accept/reject actions, cover art, and batch-accept.

**Tech Stack:** Rust/Axum (existing) + `reqwest` (move to main deps) + `lofty` (existing) + React/TanStack Query (existing).

**Branch:**
```bash
git checkout main
git checkout -b 0.3
```

---

## Phase 3 Notes

### FreeDB / gnudb.org scope

FreeDB (now hosted as gnudb.org) is a disc-ID–based CD database — it does not support text search. The integration here is intentionally scoped: if a track's tags contain a `DISCID` field (written by most CD rippers), suzuran queries gnudb.org over the CDDB HTTP protocol and creates a `source = 'freedb'` tag suggestion. If no `DISCID` tag is present, the job skips the track. FreeDB results have lower confidence (0.5) than AcoustID results. The Inbox diff view treats all sources uniformly — the user sees the same accept/reject UI regardless of source.

### Meaningful output checkpoints

| After task | What you can see |
|-----------|-----------------|
| Task 2 | `fingerprint` jobs appear in the Jobs UI after a scan |
| Task 4 | `GET /api/v1/tag-suggestions` returns pending suggestions |
| Task 6 | Inbox page renders in browser with nav badge |
| Task 8 | Accept/reject round-trip works in browser; tags written to file |
| Task 10 | Full automated pipeline: scan → fingerprint → mb_lookup → inbox |

---

## Task 1: DB migration — tag_suggestions table

**Files:**
- Create: `migrations/postgres/0009_tag_suggestions.sql`
- Create: `migrations/sqlite/0009_tag_suggestions.sql`
- Modify: `src/models/mod.rs` — add `TagSuggestion` + `UpsertTagSuggestion`
- Modify: `src/dal/mod.rs` — add 5 Store trait methods
- Modify: `src/dal/postgres.rs` — implement
- Modify: `src/dal/sqlite.rs` — implement
- Create: `tests/tag_suggestions_dal.rs`

**Step 1: Write the failing test**

```rust
// tests/tag_suggestions_dal.rs
use suzuran_server::dal::UpsertTagSuggestion;
mod common;

#[tokio::test]
async fn test_create_and_list_pending() {
    let (store, track_id) = common::setup_with_track().await;

    let dto = UpsertTagSuggestion {
        track_id,
        source: "acoustid".into(),
        suggested_tags: serde_json::json!({"title": "Test Title", "artist": "Test Artist"}),
        confidence: 0.92,
        mb_recording_id: Some("rec-uuid".into()),
        mb_release_id: Some("rel-uuid".into()),
        cover_art_url: None,
    };
    let s = store.create_tag_suggestion(dto).await.unwrap();
    assert_eq!(s.status, "pending");
    assert_eq!(s.source, "acoustid");
    assert!((s.confidence - 0.92).abs() < 0.001);

    let pending = store.list_pending_tag_suggestions(None).await.unwrap();
    assert_eq!(pending.len(), 1);

    store.set_tag_suggestion_status(s.id, "accepted").await.unwrap();
    let pending2 = store.list_pending_tag_suggestions(None).await.unwrap();
    assert_eq!(pending2.len(), 0);

    let count = store.pending_tag_suggestion_count().await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_filter_by_track_id() {
    let (store, track_id) = common::setup_with_track().await;

    store.create_tag_suggestion(UpsertTagSuggestion {
        track_id,
        source: "mb_search".into(),
        suggested_tags: serde_json::json!({}),
        confidence: 0.7,
        mb_recording_id: None,
        mb_release_id: None,
        cover_art_url: None,
    }).await.unwrap();

    let filtered = store.list_pending_tag_suggestions(Some(track_id)).await.unwrap();
    assert_eq!(filtered.len(), 1);

    let wrong_id = store.list_pending_tag_suggestions(Some(track_id + 999)).await.unwrap();
    assert_eq!(wrong_id.len(), 0);
}
```

**Step 2: Run to verify failure**
```bash
docker buildx build --progress=plain -t suzuran:dev . 2>&1 | tail -20
```
Expected: compile error — `UpsertTagSuggestion` not defined.

**Step 3: Write migrations**

`migrations/postgres/0009_tag_suggestions.sql`:
```sql
CREATE TABLE tag_suggestions (
    id              BIGSERIAL PRIMARY KEY,
    track_id        BIGINT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    source          TEXT NOT NULL CHECK (source IN ('acoustid', 'mb_search', 'freedb')),
    suggested_tags  JSONB NOT NULL,
    confidence      REAL NOT NULL DEFAULT 0.0,
    mb_recording_id TEXT,
    mb_release_id   TEXT,
    cover_art_url   TEXT,
    status          TEXT NOT NULL DEFAULT 'pending'
                        CHECK (status IN ('pending', 'accepted', 'rejected')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_tag_suggestions_track_id ON tag_suggestions(track_id);
CREATE INDEX idx_tag_suggestions_status   ON tag_suggestions(status);
```

`migrations/sqlite/0009_tag_suggestions.sql`:
```sql
CREATE TABLE tag_suggestions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    track_id        INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    source          TEXT NOT NULL,
    suggested_tags  TEXT NOT NULL,
    confidence      REAL NOT NULL DEFAULT 0.0,
    mb_recording_id TEXT,
    mb_release_id   TEXT,
    cover_art_url   TEXT,
    status          TEXT NOT NULL DEFAULT 'pending',
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_tag_suggestions_track_id ON tag_suggestions(track_id);
CREATE INDEX idx_tag_suggestions_status   ON tag_suggestions(status);
```

**Step 4: Add model to `src/models/mod.rs`** (after `OrganizationRule`):
```rust
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct TagSuggestion {
    pub id: i64,
    pub track_id: i64,
    pub source: String,
    pub suggested_tags: serde_json::Value,
    pub confidence: f32,
    pub mb_recording_id: Option<String>,
    pub mb_release_id: Option<String>,
    pub cover_art_url: Option<String>,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct UpsertTagSuggestion {
    pub track_id: i64,
    pub source: String,                       // "acoustid" | "mb_search" | "freedb"
    pub suggested_tags: serde_json::Value,
    pub confidence: f32,
    pub mb_recording_id: Option<String>,
    pub mb_release_id: Option<String>,
    pub cover_art_url: Option<String>,
}
```

**Step 5: Add to `Store` trait in `src/dal/mod.rs`**:
```rust
async fn create_tag_suggestion(&self, dto: UpsertTagSuggestion) -> Result<TagSuggestion, AppError>;
async fn list_pending_tag_suggestions(&self, track_id: Option<i64>) -> Result<Vec<TagSuggestion>, AppError>;
async fn get_tag_suggestion(&self, id: i64) -> Result<TagSuggestion, AppError>;
async fn set_tag_suggestion_status(&self, id: i64, status: &str) -> Result<(), AppError>;
async fn pending_tag_suggestion_count(&self) -> Result<i64, AppError>;
```

**Step 6: Implement in `src/dal/postgres.rs`**:
```rust
async fn create_tag_suggestion(&self, dto: UpsertTagSuggestion) -> Result<TagSuggestion, AppError> {
    sqlx::query_as!(
        TagSuggestion,
        r#"INSERT INTO tag_suggestions
           (track_id, source, suggested_tags, confidence, mb_recording_id, mb_release_id, cover_art_url)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING *"#,
        dto.track_id, dto.source, dto.suggested_tags, dto.confidence,
        dto.mb_recording_id, dto.mb_release_id, dto.cover_art_url
    )
    .fetch_one(&self.pool)
    .await
    .map_err(AppError::from)
}

async fn list_pending_tag_suggestions(&self, track_id: Option<i64>) -> Result<Vec<TagSuggestion>, AppError> {
    sqlx::query_as!(
        TagSuggestion,
        r#"SELECT * FROM tag_suggestions
           WHERE status = 'pending'
             AND ($1::bigint IS NULL OR track_id = $1)
           ORDER BY confidence DESC, created_at ASC"#,
        track_id
    )
    .fetch_all(&self.pool)
    .await
    .map_err(AppError::from)
}

async fn get_tag_suggestion(&self, id: i64) -> Result<TagSuggestion, AppError> {
    sqlx::query_as!(TagSuggestion, "SELECT * FROM tag_suggestions WHERE id = $1", id)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)
}

async fn set_tag_suggestion_status(&self, id: i64, status: &str) -> Result<(), AppError> {
    sqlx::query!(
        "UPDATE tag_suggestions SET status = $1 WHERE id = $2",
        status, id
    )
    .execute(&self.pool)
    .await
    .map_err(AppError::from)?;
    Ok(())
}

async fn pending_tag_suggestion_count(&self) -> Result<i64, AppError> {
    let row = sqlx::query!(
        "SELECT COUNT(*) AS count FROM tag_suggestions WHERE status = 'pending'"
    )
    .fetch_one(&self.pool)
    .await
    .map_err(AppError::from)?;
    Ok(row.count.unwrap_or(0))
}
```

SQLite impl: same queries, `?` placeholders, `i64` instead of `$1::bigint IS NULL` (use `WHERE status='pending' AND (? IS NULL OR track_id = ?)`).

**Step 7: Run to verify pass**
```bash
docker buildx build --progress=plain -t suzuran:dev . 2>&1 | tail -20
```
Expected: BUILD SUCCESS, all tests pass.

**Step 8: Update codebase filemap** — add `tests/tag_suggestions_dal.rs` entry; update `migrations/` table.

**Step 9: Commit**
```bash
git add migrations/ src/models/mod.rs src/dal/ tests/tag_suggestions_dal.rs tasks/codebase-filemap.md
git commit -m "feat(3.1): tag_suggestions migration, model, and DAL"
```

---

## Task 2: Fingerprint job (fpcalc subprocess)

**Files:**
- Create: `src/jobs/fingerprint.rs`
- Modify: `src/jobs/mod.rs` — add `FingerprintPayload`; export module
- Modify: `src/jobs/scan.rs` — enqueue `fingerprint` per new track
- Modify: `src/dal/mod.rs` — add `update_track_fingerprint` to Store
- Modify: `src/dal/postgres.rs`, `src/dal/sqlite.rs`
- Create: `tests/fingerprint_job.rs`

**Background:** `fpcalc` (Chromaprint CLI) is installed in the Docker image. It outputs JSON with `fingerprint` (a base64-encoded fingerprint string) and `duration` (float seconds). The fingerprint is stored in `tracks.tags["acoustid_fingerprint"]` so it's queryable by the `mb_lookup` job and returned in the API alongside other tags.

**Step 1: Write the failing test**
```rust
// tests/fingerprint_job.rs
use suzuran_server::jobs::fingerprint::FingerprintJobHandler;
use suzuran_server::jobs::JobHandler;
mod common;

#[tokio::test]
async fn test_fingerprint_stores_in_track_tags() {
    // common::setup_with_audio_track returns (store, track_id, library_root_path)
    let (store, track_id, _root) = common::setup_with_audio_track().await;

    let handler = FingerprintJobHandler::new(store.clone());
    let result = handler.handle(serde_json::json!({"track_id": track_id})).await.unwrap();

    assert!(result.get("fingerprint").is_some(), "result should contain fingerprint");

    let track = store.get_track(track_id).await.unwrap();
    let fp = track.tags
        .get("acoustid_fingerprint")
        .and_then(|v| v.as_str())
        .expect("acoustid_fingerprint should be in track tags");
    assert!(!fp.is_empty());
}
```

**Step 2: Verify fail**
```bash
docker buildx build --progress=plain -t suzuran:dev . 2>&1 | tail -20
```

**Step 3: Add to Store trait and impls**

`src/dal/mod.rs`:
```rust
async fn update_track_fingerprint(&self, track_id: i64, fingerprint: &str, duration_secs: f64) -> Result<(), AppError>;
```

Postgres: merge fingerprint into the `tags` JSONB and update `duration_secs`:
```rust
async fn update_track_fingerprint(&self, track_id: i64, fingerprint: &str, duration_secs: f64) -> Result<(), AppError> {
    sqlx::query!(
        r#"UPDATE tracks
           SET tags = tags || jsonb_build_object('acoustid_fingerprint', $1::text),
               duration_secs = $2
           WHERE id = $3"#,
        fingerprint, duration_secs, track_id
    )
    .execute(&self.pool)
    .await
    .map_err(AppError::from)?;
    Ok(())
}
```

SQLite: deserialize `tags` TEXT → merge → serialize back:
```rust
async fn update_track_fingerprint(&self, track_id: i64, fingerprint: &str, duration_secs: f64) -> Result<(), AppError> {
    let row = sqlx::query!("SELECT tags FROM tracks WHERE id = ?", track_id)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
    let mut tags: serde_json::Value = row.tags
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    tags["acoustid_fingerprint"] = serde_json::Value::String(fingerprint.into());
    let tags_str = serde_json::to_string(&tags).unwrap();
    sqlx::query!(
        "UPDATE tracks SET tags = ?, duration_secs = ? WHERE id = ?",
        tags_str, duration_secs, track_id
    )
    .execute(&self.pool)
    .await
    .map_err(AppError::from)?;
    Ok(())
}
```

**Step 4: Implement `src/jobs/fingerprint.rs`**

```rust
use crate::{dal::Store, error::AppError};
use std::sync::Arc;
use tokio::process::Command;

pub struct FingerprintJobHandler {
    store: Arc<dyn Store>,
}

impl FingerprintJobHandler {
    pub fn new(store: Arc<dyn Store>) -> Self { Self { store } }
}

#[async_trait::async_trait]
impl super::JobHandler for FingerprintJobHandler {
    async fn handle(&self, payload: serde_json::Value) -> Result<serde_json::Value, AppError> {
        let track_id = payload["track_id"].as_i64()
            .ok_or_else(|| AppError::BadRequest("missing track_id".into()))?;

        let track = self.store.get_track(track_id).await?;
        let library = self.store.get_library(track.library_id).await?;
        let full_path = format!(
            "{}/{}",
            library.root_path.trim_end_matches('/'),
            track.relative_path.trim_start_matches('/')
        );

        let out = Command::new("fpcalc")
            .args(["-json", &full_path])
            .output()
            .await
            .map_err(|e| AppError::Internal(format!("fpcalc spawn failed: {e}")))?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(AppError::Internal(format!("fpcalc failed: {stderr}")));
        }

        let json: serde_json::Value = serde_json::from_slice(&out.stdout)
            .map_err(|e| AppError::Internal(format!("fpcalc json parse: {e}")))?;

        let fingerprint = json["fingerprint"].as_str()
            .ok_or_else(|| AppError::Internal("no fingerprint in fpcalc output".into()))?;
        let duration = json["duration"].as_f64().unwrap_or(0.0);

        self.store.update_track_fingerprint(track_id, fingerprint, duration).await?;

        Ok(serde_json::json!({
            "track_id": track_id,
            "fingerprint": fingerprint,
            "duration_secs": duration
        }))
    }
}
```

**Step 5: Modify `src/jobs/scan.rs`** — after each newly inserted track, enqueue a fingerprint job:
```rust
// After confirming a track row was inserted (not just updated):
store.create_job(crate::dal::CreateJob {
    job_type: "fingerprint".into(),
    payload: serde_json::json!({"track_id": new_track.id}),
    priority: 5,
}).await?;
```

Check `src/jobs/mod.rs` for how `CreateJob` is defined. If not present, add it:
```rust
pub struct FingerprintPayload {
    pub track_id: i64,
}
```

Export `pub mod fingerprint;` from `src/jobs/mod.rs`.

**Step 6: Verify pass**
```bash
docker buildx build --progress=plain -t suzuran:dev . 2>&1 | tail -20
```

**Step 7: Update codebase filemap** — add `src/jobs/fingerprint.rs`, `tests/fingerprint_job.rs`.

**Step 8: Commit**
```bash
git add src/jobs/fingerprint.rs src/jobs/mod.rs src/jobs/scan.rs src/dal/ tests/fingerprint_job.rs tasks/codebase-filemap.md
git commit -m "feat(3.2): fingerprint job — fpcalc subprocess, scan auto-enqueue"
```

---

## Task 3: MusicBrainz / AcoustID HTTP service

**Files:**
- Modify: `Cargo.toml` — move `reqwest` from `[dev-dependencies]` to `[dependencies]`; add `wiremock` to `[dev-dependencies]`
- Create: `src/services/musicbrainz.rs`
- Modify: `src/services/mod.rs` — export module

**Note:** `reqwest` is already in `[dev-dependencies]`. It needs to move to `[dependencies]` because `src/services/musicbrainz.rs` is production code.

**Step 1: Write the failing test**
```rust
// tests/musicbrainz_service.rs
use suzuran_server::services::musicbrainz::MusicBrainzService;
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path_regex};

#[tokio::test]
async fn test_acoustid_lookup_returns_scored_results() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex("/v2/lookup"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "ok",
            "results": [
                {
                    "id": "acoustid-abc",
                    "score": 0.96,
                    "recordings": [{"id": "rec-uuid-1"}]
                }
            ]
        })))
        .mount(&server)
        .await;

    let svc = MusicBrainzService::with_base_urls(
        "test-key".into(),
        "https://musicbrainz.org/ws/2".into(), // MB URL not used in this test
        server.uri(),
    );

    let results = svc.acoustid_lookup("AQABz0kkdeRiJI...", 210.0).await.unwrap();
    assert_eq!(results.len(), 1);
    assert!((results[0].score - 0.96).abs() < 0.01);
    assert_eq!(results[0].recordings.as_ref().unwrap()[0].id, "rec-uuid-1");
}

#[tokio::test]
async fn test_get_recording_fetches_metadata() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex("/recording/rec-uuid-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "rec-uuid-1",
            "title": "Comfortably Numb",
            "length": 382000,
            "releases": [
                {
                    "id": "rel-uuid-1",
                    "title": "The Wall",
                    "date": "1979-11-30",
                    "artist-credit": [{"name": "Pink Floyd"}]
                }
            ]
        })))
        .mount(&server)
        .await;

    let svc = MusicBrainzService::with_base_urls(
        "test-key".into(),
        server.uri(),
        "https://api.acoustid.org".into(),
    );
    let rec = svc.get_recording("rec-uuid-1").await.unwrap();
    assert_eq!(rec.title, "Comfortably Numb");
    assert_eq!(rec.releases.unwrap()[0].title, "The Wall");
}
```

**Step 2: Verify fail**
```bash
docker buildx build --progress=plain -t suzuran:dev . 2>&1 | tail -20
```

**Step 3: Update `Cargo.toml`**

Move `reqwest` from `[dev-dependencies]` to `[dependencies]`:
```toml
[dependencies]
# ... existing deps ...
reqwest = { version = "0.12", features = ["json"] }

[dev-dependencies]
reqwest = { version = "0.12", features = ["json", "cookies"] }  # keep cookies for integration tests
wiremock = "0.6"
tokio = { version = "1", features = ["full"] }
tempfile = "3"
```

**Step 4: Implement `src/services/musicbrainz.rs`**

```rust
use reqwest::Client;
use std::time::Duration;
use tokio::time::sleep;

const MB_RATE_LIMIT_MS: u64 = 1100; // MusicBrainz: max 1 req/sec

#[derive(Clone)]
pub struct MusicBrainzService {
    client: Client,
    acoustid_key: String,
    mb_base: String,
    acoustid_base: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct AcoustIdResult {
    pub id: String,
    pub score: f32,
    pub recordings: Option<Vec<AcoustIdRecording>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct AcoustIdRecording {
    pub id: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct MbRecording {
    pub id: String,
    pub title: String,
    pub length: Option<u64>,       // milliseconds
    pub releases: Option<Vec<MbRelease>>,
    #[serde(rename = "artist-credit")]
    pub artist_credit: Option<Vec<MbArtistCredit>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct MbRelease {
    pub id: String,
    pub title: String,
    pub date: Option<String>,
    #[serde(rename = "artist-credit")]
    pub artist_credit: Option<Vec<MbArtistCredit>>,
    #[serde(rename = "label-info")]
    pub label_info: Option<Vec<MbLabelInfo>>,
    #[serde(rename = "release-group")]
    pub release_group: Option<MbReleaseGroup>,
    pub media: Option<Vec<MbMedia>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct MbArtistCredit {
    pub name: Option<String>,
    pub artist: Option<MbArtist>,
}

#[derive(Debug, serde::Deserialize)]
pub struct MbArtist {
    pub id: String,
    pub name: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct MbLabelInfo {
    pub label: Option<MbLabel>,
    #[serde(rename = "catalog-number")]
    pub catalog_number: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct MbLabel {
    pub name: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct MbReleaseGroup {
    #[serde(rename = "primary-type")]
    pub primary_type: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct MbMedia {
    pub position: Option<u32>,
    #[serde(rename = "track-count")]
    pub track_count: Option<u32>,
    pub tracks: Option<Vec<MbTrack>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct MbTrack {
    pub number: Option<String>,
    pub title: Option<String>,
}

impl MusicBrainzService {
    pub fn new(acoustid_key: String) -> Self {
        Self::with_base_urls(
            acoustid_key,
            "https://musicbrainz.org/ws/2".into(),
            "https://api.acoustid.org".into(),
        )
    }

    pub fn with_base_urls(acoustid_key: String, mb_base: String, acoustid_base: String) -> Self {
        let client = Client::builder()
            .user_agent("suzuran/0.3 ( https://github.com/user/suzuran )")
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client build");
        Self { client, acoustid_key, mb_base, acoustid_base }
    }

    pub async fn acoustid_lookup(
        &self,
        fingerprint: &str,
        duration: f64,
    ) -> anyhow::Result<Vec<AcoustIdResult>> {
        let url = format!("{}/v2/lookup", self.acoustid_base);
        let resp: serde_json::Value = self.client
            .get(&url)
            .query(&[
                ("client", self.acoustid_key.as_str()),
                ("fingerprint", fingerprint),
                ("duration", &duration.round().to_string()),
                ("meta", "recordings"),
            ])
            .send().await?
            .error_for_status()?
            .json().await?;

        let results = resp["results"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect();
        Ok(results)
    }

    pub async fn get_recording(&self, recording_id: &str) -> anyhow::Result<MbRecording> {
        sleep(Duration::from_millis(MB_RATE_LIMIT_MS)).await;
        let url = format!("{}/recording/{}", self.mb_base, recording_id);
        let rec = self.client
            .get(&url)
            .query(&[("inc", "releases+artist-credits+labels+release-groups+media"), ("fmt", "json")])
            .send().await?
            .error_for_status()?
            .json::<MbRecording>().await?;
        Ok(rec)
    }

    /// Build a MusicBrainz-keyed tag map from a recording + chosen release.
    pub fn to_tag_map(
        rec: &MbRecording,
        release: &MbRelease,
    ) -> std::collections::HashMap<String, String> {
        let mut tags = std::collections::HashMap::new();

        tags.insert("title".into(), rec.title.clone());
        tags.insert("musicbrainz_recordingid".into(), rec.id.clone());
        tags.insert("musicbrainz_releaseid".into(), release.id.clone());
        tags.insert("album".into(), release.title.clone());

        if let Some(date) = &release.date {
            tags.insert("date".into(), date.clone());
        }

        // Artist from recording-level artist-credit (primary artist)
        let artist_name = rec.artist_credit.as_ref()
            .and_then(|ac| ac.first())
            .and_then(|a| a.name.as_ref().or(a.artist.as_ref().map(|ar| &ar.name)))
            .cloned()
            .unwrap_or_default();
        if !artist_name.is_empty() {
            tags.insert("artist".into(), artist_name.clone());
            tags.insert("albumartist".into(), artist_name);
        }

        // Label + catalog number
        if let Some(label_info) = release.label_info.as_ref().and_then(|li| li.first()) {
            if let Some(label) = &label_info.label {
                tags.insert("label".into(), label.name.clone());
            }
            if let Some(cat) = &label_info.catalog_number {
                tags.insert("catalognumber".into(), cat.clone());
            }
        }

        // Disc count
        if let Some(media) = &release.media {
            let disc_count = media.len();
            if disc_count > 1 {
                tags.insert("totaldiscs".into(), disc_count.to_string());
            }
        }

        // Release group type
        if let Some(rg) = &release.release_group {
            if let Some(pt) = &rg.primary_type {
                tags.insert("releasetype".into(), pt.to_lowercase());
            }
        }

        tags
    }

    /// Cover Art Archive URL for a release (front image, 500px).
    pub fn caa_url(release_id: &str) -> String {
        format!("https://coverartarchive.org/release/{}/front-500", release_id)
    }
}
```

**Step 5: Export from `src/services/mod.rs`**:
```rust
pub mod musicbrainz;
```

**Step 6: Verify pass**
```bash
docker buildx build --progress=plain -t suzuran:dev . 2>&1 | tail -20
```

**Step 7: Update codebase filemap** — add `src/services/musicbrainz.rs`.

**Step 8: Commit**
```bash
git add src/services/musicbrainz.rs src/services/mod.rs Cargo.toml Cargo.lock tests/musicbrainz_service.rs tasks/codebase-filemap.md
git commit -m "feat(3.3): MusicBrainz/AcoustID HTTP service with wiremock tests"
```

---

## Task 4: MB lookup job

**Files:**
- Create: `src/jobs/mb_lookup.rs`
- Modify: `src/jobs/mod.rs` — export module
- Modify: `src/state.rs` — add `Arc<MusicBrainzService>` field + construction in `main.rs`
- Modify: `src/main.rs` — build `MusicBrainzService` from settings
- Create: `tests/mb_lookup_job.rs`

**Step 1: Write the failing test** (uses wiremock to mock both AcoustID and MB)
```rust
// tests/mb_lookup_job.rs
use suzuran_server::jobs::mb_lookup::MbLookupJobHandler;
use suzuran_server::jobs::JobHandler;
use suzuran_server::services::musicbrainz::MusicBrainzService;
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path_regex};
use std::sync::Arc;
mod common;

#[tokio::test]
async fn test_mb_lookup_creates_suggestion() {
    let acoustid_server = MockServer::start().await;
    let mb_server = MockServer::start().await;

    // Seed AcoustID response
    Mock::given(method("GET")).and(path_regex("/v2/lookup"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "ok",
            "results": [{"id": "aid-1", "score": 0.95, "recordings": [{"id": "rec-1"}]}]
        })))
        .mount(&acoustid_server).await;

    // Seed MB recording response
    Mock::given(method("GET")).and(path_regex("/recording/rec-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "rec-1", "title": "Test Song",
            "releases": [{"id": "rel-1", "title": "Test Album", "date": "2000"}]
        })))
        .mount(&mb_server).await;

    let (store, track_id) = common::setup_with_fingerprinted_track().await;
    let mb_svc = Arc::new(MusicBrainzService::with_base_urls(
        "test-key".into(), mb_server.uri(), acoustid_server.uri(),
    ));

    let handler = MbLookupJobHandler::new(store.clone(), mb_svc);
    let result = handler.handle(serde_json::json!({"track_id": track_id})).await.unwrap();

    assert_eq!(result["suggestions_created"].as_i64(), Some(1));
    let suggestions = store.list_pending_tag_suggestions(Some(track_id)).await.unwrap();
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].source, "acoustid");
    assert!((suggestions[0].confidence - 0.95).abs() < 0.01);
}

#[tokio::test]
async fn test_mb_lookup_enqueues_freedb_when_no_matches() {
    let acoustid_server = MockServer::start().await;
    Mock::given(method("GET")).and(path_regex("/v2/lookup"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "ok",
            "results": []
        })))
        .mount(&acoustid_server).await;

    let (store, track_id) = common::setup_with_fingerprinted_track().await;
    let mb_svc = Arc::new(MusicBrainzService::with_base_urls(
        "test-key".into(), "http://unused".into(), acoustid_server.uri(),
    ));

    let handler = MbLookupJobHandler::new(store.clone(), mb_svc);
    handler.handle(serde_json::json!({"track_id": track_id})).await.unwrap();

    // A freedb_lookup job should be in the queue
    let jobs = store.list_jobs(None, Some("pending")).await.unwrap();
    let freedb_job = jobs.iter().find(|j| j.job_type == "freedb_lookup");
    assert!(freedb_job.is_some(), "freedb_lookup job should be enqueued as fallback");
}
```

**Step 2: Verify fail**
```bash
docker buildx build --progress=plain -t suzuran:dev . 2>&1 | tail -20
```

**Step 3: Add `MusicBrainzService` to `AppState`** (`src/state.rs`):
```rust
pub struct AppState {
    pub store: Arc<dyn Store>,
    pub config: Arc<Config>,
    pub webauthn: Arc<Webauthn>,
    pub mb_service: Arc<crate::services::musicbrainz::MusicBrainzService>,
}
```

In `src/main.rs`, build the service using the AcoustID API key from settings (or from env as bootstrap):
```rust
// Read ACOUSTID_KEY from env at startup (settings table populated after first boot,
// so env var is the bootstrap path for the service to start):
let acoustid_key = std::env::var("ACOUSTID_KEY").unwrap_or_default();
let mb_service = Arc::new(MusicBrainzService::new(acoustid_key));
```

**Step 4: Implement `src/jobs/mb_lookup.rs`**

```rust
use crate::{
    dal::{Store, UpsertTagSuggestion, CreateJob},
    error::AppError,
    services::musicbrainz::MusicBrainzService,
};
use std::sync::Arc;

pub struct MbLookupJobHandler {
    store: Arc<dyn Store>,
    mb_service: Arc<MusicBrainzService>,
}

impl MbLookupJobHandler {
    pub fn new(store: Arc<dyn Store>, mb_service: Arc<MusicBrainzService>) -> Self {
        Self { store, mb_service }
    }
}

const ACOUSTID_THRESHOLD: f32 = 0.8;

#[async_trait::async_trait]
impl super::JobHandler for MbLookupJobHandler {
    async fn handle(&self, payload: serde_json::Value) -> Result<serde_json::Value, AppError> {
        let track_id = payload["track_id"].as_i64()
            .ok_or_else(|| AppError::BadRequest("missing track_id".into()))?;

        let track = self.store.get_track(track_id).await?;

        let fingerprint = track.tags
            .get("acoustid_fingerprint")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Internal("track has no acoustid_fingerprint".into()))?
            .to_string();

        let duration = track.duration_secs.unwrap_or(0.0);

        let results = self.mb_service
            .acoustid_lookup(&fingerprint, duration)
            .await
            .map_err(|e| AppError::Internal(format!("AcoustID: {e}")))?;

        let mut suggestions_created: usize = 0;

        for result in results.iter().filter(|r| r.score >= ACOUSTID_THRESHOLD) {
            let Some(recordings) = &result.recordings else { continue };
            for rec_stub in recordings {
                let rec = match self.mb_service.get_recording(&rec_stub.id).await {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!("MB recording fetch failed for {}: {e}", rec_stub.id);
                        continue;
                    }
                };

                let releases = rec.releases.clone().unwrap_or_default();
                for release in &releases {
                    let tag_map = MusicBrainzService::to_tag_map(&rec, release);
                    let cover_art_url = Some(MusicBrainzService::caa_url(&release.id));

                    self.store.create_tag_suggestion(UpsertTagSuggestion {
                        track_id,
                        source: "acoustid".into(),
                        suggested_tags: serde_json::to_value(&tag_map)
                            .map_err(|e| AppError::Internal(e.to_string()))?,
                        confidence: result.score,
                        mb_recording_id: Some(rec.id.clone()),
                        mb_release_id: Some(release.id.clone()),
                        cover_art_url,
                    }).await?;

                    suggestions_created += 1;
                }
            }
        }

        // No AcoustID matches above threshold → enqueue FreeDB fallback
        if suggestions_created == 0 {
            self.store.create_job(CreateJob {
                job_type: "freedb_lookup".into(),
                payload: serde_json::json!({"track_id": track_id}),
                priority: 4,
            }).await?;
        }

        Ok(serde_json::json!({
            "track_id": track_id,
            "suggestions_created": suggestions_created,
        }))
    }
}
```

Export: `pub mod mb_lookup;` in `src/jobs/mod.rs`.

**Step 5: Verify pass**
```bash
docker buildx build --progress=plain -t suzuran:dev . 2>&1 | tail -20
```

**Step 6: Update codebase filemap** — add `src/jobs/mb_lookup.rs`; update `src/state.rs` description.

**Step 7: Commit**
```bash
git add src/jobs/mb_lookup.rs src/jobs/mod.rs src/state.rs src/main.rs tests/mb_lookup_job.rs tasks/codebase-filemap.md
git commit -m "feat(3.4): mb_lookup job — AcoustID + MusicBrainz, freedb fallback enqueue"
```

---

## Task 5: gnudb.org (FreeDB) lookup job

**Files:**
- Create: `src/services/freedb.rs`
- Create: `src/jobs/freedb_lookup.rs`
- Modify: `src/services/mod.rs`, `src/jobs/mod.rs`
- Create: `tests/freedb_lookup_job.rs`

**Background:** FreeDB/gnudb uses the CDDB protocol over HTTP. The protocol is disc-ID–based; lookup is only possible when a `DISCID` tag is present in the file (e.g., `DISCID=a50e1d13`). Most CD rippers (EAC, whipper, dBpoweramp) write this tag. If `DISCID` is absent, the job records a skip and exits cleanly. The CDDB flow: (1) query by disc ID, (2) if multiple matches, read the first match, (3) parse the XMCD format response into a tag map.

**Step 1: Write the failing test**
```rust
// tests/freedb_lookup_job.rs
use suzuran_server::services::freedb::FreedBService;
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, query_param};
mod common;

#[tokio::test]
async fn test_freedb_disc_lookup_creates_suggestion() {
    let server = MockServer::start().await;

    // Mock CDDB query response (211 = inexact match)
    Mock::given(method("GET"))
        .and(query_param("cmd", "cddb query a50e1d13 12 150 23115 41765 54723 68158 80500 98520 112670 131715 148020 163517 2750"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_string("211 Found inexact matches\nrock a50e1d13 Artist / Album Title\n.\n"))
        .mount(&server).await;

    // Mock CDDB read response (XMCD format)
    Mock::given(method("GET"))
        .and(query_param("cmd", "cddb read rock a50e1d13"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_string(
                "200 rock a50e1d13\n\
                 DISCID=a50e1d13\n\
                 DTITLE=Test Artist / Test Album\n\
                 DYEAR=1999\n\
                 DGENRE=Rock\n\
                 TTITLE0=Track One\n\
                 TTITLE1=Track Two\n\
                 .\n"
            ))
        .mount(&server).await;

    let svc = FreedBService::with_base_url(server.uri());
    let result = svc.disc_lookup("a50e1d13").await.unwrap();
    assert!(result.is_some());
    let candidate = result.unwrap();
    assert_eq!(candidate.artist, "Test Artist");
    assert_eq!(candidate.album, "Test Album");
    assert_eq!(candidate.tracks[0], "Track One");
}

#[tokio::test]
async fn test_freedb_job_skips_track_without_discid() {
    let (store, track_id) = common::setup_with_track().await; // track has no DISCID tag
    let svc = Arc::new(FreedBService::with_base_url("http://unused".into()));

    let handler = suzuran_server::jobs::freedb_lookup::FreedBLookupJobHandler::new(
        store.clone(), svc
    );
    let result = handler.handle(serde_json::json!({"track_id": track_id})).await.unwrap();
    assert_eq!(result["skipped"].as_bool(), Some(true));

    let suggestions = store.list_pending_tag_suggestions(Some(track_id)).await.unwrap();
    assert_eq!(suggestions.len(), 0);
}
```

**Step 2: Verify fail**
```bash
docker buildx build --progress=plain -t suzuran:dev . 2>&1 | tail -20
```

**Step 3: Implement `src/services/freedb.rs`**

```rust
use reqwest::Client;
use std::time::Duration;

pub struct FreedBService {
    client: Client,
    base_url: String,
}

#[derive(Debug)]
pub struct FreedBCandidate {
    pub artist: String,
    pub album: String,
    pub year: Option<String>,
    pub genre: Option<String>,
    pub tracks: Vec<String>,      // indexed 0..N-1
}

impl FreedBService {
    pub fn new() -> Self {
        Self::with_base_url("http://gnudb.org/~cddb/cddb.cgi".into())
    }

    pub fn with_base_url(base_url: String) -> Self {
        let client = Client::builder()
            .user_agent("suzuran/0.3")
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap();
        Self { client, base_url }
    }

    /// Look up a disc by its CDDB disc ID.
    /// Returns the first matching candidate, or None if not found.
    pub async fn disc_lookup(&self, disc_id: &str) -> anyhow::Result<Option<FreedBCandidate>> {
        // Step 1: query by disc ID (we don't have offset data, so we pass a minimal query)
        // The server returns matching entries with category + disc ID
        let query_cmd = format!("cddb query {} 1 0 60", disc_id); // minimal valid query
        let query_resp = self.cddb_request(&query_cmd).await?;

        let status_code = query_resp.lines().next()
            .and_then(|l| l.split_whitespace().next())
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(0);

        // 200 = exact match, 211 = inexact matches, 202 = no match
        if status_code == 202 || status_code == 0 {
            return Ok(None);
        }

        // Extract category and disc ID from the response
        let (category, found_id) = parse_cddb_query_first_result(&query_resp)?;

        // Step 2: read the full entry
        let read_cmd = format!("cddb read {} {}", category, found_id);
        let read_resp = self.cddb_request(&read_cmd).await?;

        if !read_resp.starts_with("200") {
            return Ok(None);
        }

        Ok(Some(parse_xmcd(&read_resp)))
    }

    async fn cddb_request(&self, cmd: &str) -> anyhow::Result<String> {
        let text = self.client.get(&self.base_url)
            .query(&[
                ("cmd", cmd),
                ("hello", "user localhost suzuran 0.3"),
                ("proto", "6"),
            ])
            .send().await?
            .text().await?;
        Ok(text)
    }

    /// Convert a FreedBCandidate to a MusicBrainz-compatible tag map.
    pub fn to_tag_map(
        candidate: &FreedBCandidate,
        zero_based_track_index: usize,
    ) -> std::collections::HashMap<String, String> {
        let mut tags = std::collections::HashMap::new();
        tags.insert("artist".into(), candidate.artist.clone());
        tags.insert("albumartist".into(), candidate.artist.clone());
        tags.insert("album".into(), candidate.album.clone());
        if let Some(year) = &candidate.year {
            tags.insert("date".into(), year.clone());
        }
        if let Some(genre) = &candidate.genre {
            tags.insert("genre".into(), genre.clone());
        }
        if let Some(title) = candidate.tracks.get(zero_based_track_index) {
            tags.insert("title".into(), title.clone());
        }
        tags.insert("totaltracks".into(), candidate.tracks.len().to_string());
        tags
    }
}

fn parse_cddb_query_first_result(text: &str) -> anyhow::Result<(String, String)> {
    // After the status line, each result line is: "category discid Artist / Album"
    let result_line = text.lines()
        .skip(1)
        .find(|l| !l.starts_with('.') && !l.is_empty())
        .ok_or_else(|| anyhow::anyhow!("no result line in CDDB query response"))?;

    let mut parts = result_line.splitn(3, ' ');
    let category = parts.next().unwrap_or("misc").to_string();
    let disc_id  = parts.next().unwrap_or("").to_string();
    Ok((category, disc_id))
}

/// Parse an XMCD-format CDDB record (lines after the 200 status).
fn parse_xmcd(text: &str) -> FreedBCandidate {
    let mut artist = String::new();
    let mut album  = String::new();
    let mut year   = None;
    let mut genre  = None;
    let mut tracks: std::collections::BTreeMap<usize, String> = Default::default();

    for line in text.lines().skip(1) {
        let line = line.trim();
        if line.starts_with('#') || line == "." { continue; }

        if let Some(val) = line.strip_prefix("DTITLE=") {
            // "Artist / Album Title"
            if let Some((a, b)) = val.split_once(" / ") {
                artist = a.trim().into();
                album  = b.trim().into();
            } else {
                album = val.trim().into();
            }
        } else if let Some(val) = line.strip_prefix("DYEAR=") {
            year = Some(val.trim().into());
        } else if let Some(val) = line.strip_prefix("DGENRE=") {
            genre = Some(val.trim().into());
        } else if line.starts_with("TTITLE") {
            // TTITLEn=Track Name
            if let Some(eq) = line.find('=') {
                let idx_str = &line[6..eq];
                if let Ok(idx) = idx_str.parse::<usize>() {
                    tracks.insert(idx, line[eq+1..].trim().into());
                }
            }
        }
    }

    FreedBCandidate {
        artist,
        album,
        year,
        genre,
        tracks: tracks.into_values().collect(),
    }
}
```

**Step 4: Implement `src/jobs/freedb_lookup.rs`**

```rust
use crate::{dal::{Store, UpsertTagSuggestion}, error::AppError, services::freedb::FreedBService};
use std::sync::Arc;

pub struct FreedBLookupJobHandler {
    store: Arc<dyn Store>,
    freedb: Arc<FreedBService>,
}

impl FreedBLookupJobHandler {
    pub fn new(store: Arc<dyn Store>, freedb: Arc<FreedBService>) -> Self {
        Self { store, freedb }
    }
}

#[async_trait::async_trait]
impl super::JobHandler for FreedBLookupJobHandler {
    async fn handle(&self, payload: serde_json::Value) -> Result<serde_json::Value, AppError> {
        let track_id = payload["track_id"].as_i64()
            .ok_or_else(|| AppError::BadRequest("missing track_id".into()))?;

        let track = self.store.get_track(track_id).await?;

        // FreeDB requires a DISCID tag — skip if absent
        let disc_id = match track.tags.get("DISCID").or_else(|| track.tags.get("discid"))
            .and_then(|v| v.as_str()).map(str::to_string)
        {
            Some(id) if !id.is_empty() => id,
            _ => {
                return Ok(serde_json::json!({"track_id": track_id, "skipped": true, "reason": "no DISCID tag"}));
            }
        };

        let candidate = match self.freedb.disc_lookup(&disc_id).await
            .map_err(|e| AppError::Internal(format!("gnudb.org: {e}")))? 
        {
            Some(c) => c,
            None => {
                return Ok(serde_json::json!({"track_id": track_id, "suggestions_created": 0}));
            }
        };

        // Derive zero-based track index from tracknumber tag
        let track_index = track.tags
            .get("tracknumber")
            .and_then(|v| v.as_str())
            .and_then(|s| s.split('/').next())    // handle "3/12" format
            .and_then(|s| s.trim().parse::<usize>().ok())
            .map(|n| n.saturating_sub(1))
            .unwrap_or(0);

        let tags = FreedBService::to_tag_map(&candidate, track_index);

        self.store.create_tag_suggestion(UpsertTagSuggestion {
            track_id,
            source: "freedb".into(),
            suggested_tags: serde_json::to_value(&tags)
                .map_err(|e| AppError::Internal(e.to_string()))?,
            confidence: 0.5,
            mb_recording_id: None,
            mb_release_id: None,
            cover_art_url: None,
        }).await?;

        Ok(serde_json::json!({"track_id": track_id, "suggestions_created": 1}))
    }
}
```

Export both modules and update `src/state.rs` with `Arc<FreedBService>`.

**Step 5: Verify pass**
```bash
docker buildx build --progress=plain -t suzuran:dev . 2>&1 | tail -20
```

**Step 6: Update codebase filemap** — add `src/services/freedb.rs`, `src/jobs/freedb_lookup.rs`.

**Step 7: Commit**
```bash
git add src/services/freedb.rs src/jobs/freedb_lookup.rs src/services/mod.rs src/jobs/mod.rs src/state.rs tests/freedb_lookup_job.rs tasks/codebase-filemap.md
git commit -m "feat(3.5): gnudb.org (FreeDB) CDDB disc-ID lookup job"
```

---

## Task 6: Tag suggestions REST API

**Files:**
- Create: `src/api/tag_suggestions.rs`
- Modify: `src/api/mod.rs` — mount `/tag-suggestions`
- Create: `tests/tag_suggestions_api.rs`

**Endpoints:**

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/tag-suggestions` | required | List pending (optional `?track_id=N`) |
| `GET` | `/tag-suggestions/count` | none | Badge count (public for nav polling) |
| `GET` | `/tag-suggestions/:id` | required | Single suggestion |
| `POST` | `/tag-suggestions/:id/accept` | required | Accept → writes tags (Task 7) |
| `POST` | `/tag-suggestions/:id/reject` | required | Mark rejected |
| `POST` | `/tag-suggestions/batch-accept` | required | Accept all above `min_confidence` |

**Step 1: Write the failing test**
```rust
// tests/tag_suggestions_api.rs
mod common;
use common::TestApp;

#[tokio::test]
async fn test_list_requires_auth() {
    let app = TestApp::spawn().await;
    let resp = app.client.get(&app.url("/api/v1/tag-suggestions")).send().await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_count_is_public() {
    let app = TestApp::spawn().await;
    let resp = app.client.get(&app.url("/api/v1/tag-suggestions/count")).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["count"], 0);
}

#[tokio::test]
async fn test_accept_reject_crud() {
    let app = TestApp::spawn().await;
    let (token, track_id) = app.seed_user_with_track().await;

    // Create a suggestion directly in the DB
    let s = app.store.create_tag_suggestion(UpsertTagSuggestion {
        track_id,
        source: "acoustid".into(),
        suggested_tags: serde_json::json!({"title": "Accepted Title"}),
        confidence: 0.9,
        mb_recording_id: None,
        mb_release_id: None,
        cover_art_url: None,
    }).await.unwrap();

    // Reject it
    let resp = app.client
        .post(&app.url(&format!("/api/v1/tag-suggestions/{}/reject", s.id)))
        .header("Cookie", format!("session={}", token))
        .send().await.unwrap();
    assert_eq!(resp.status(), 200);

    // Verify it no longer appears in pending list
    let resp = app.client
        .get(&app.url("/api/v1/tag-suggestions"))
        .header("Cookie", format!("session={}", token))
        .send().await.unwrap();
    let list: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(list.len(), 0);
}
```

**Step 2: Verify fail**
```bash
docker buildx build --progress=plain -t suzuran:dev . 2>&1 | tail -20
```

**Step 3: Implement `src/api/tag_suggestions.rs`**

```rust
use axum::{
    Router,
    routing::{get, post},
    extract::{Path, Query, State},
    Json, http::StatusCode,
};
use std::collections::HashMap;
use crate::{
    state::AppState,
    api::middleware::auth::AuthUser,
    error::AppError,
    models::TagSuggestion,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/",              get(list))
        .route("/count",         get(count))
        .route("/:id",           get(get_one))
        .route("/:id/accept",    post(accept))
        .route("/:id/reject",    post(reject))
        .route("/batch-accept",  post(batch_accept))
}

async fn list(
    _user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<TagSuggestion>>, AppError> {
    let track_id = params.get("track_id").and_then(|s| s.parse().ok());
    Ok(Json(state.store.list_pending_tag_suggestions(track_id).await?))
}

async fn count(State(state): State<AppState>) -> Result<Json<serde_json::Value>, AppError> {
    let n = state.store.pending_tag_suggestion_count().await?;
    Ok(Json(serde_json::json!({"count": n})))
}

async fn get_one(
    _user: AuthUser,
    Path(id): Path<i64>,
    State(state): State<AppState>,
) -> Result<Json<TagSuggestion>, AppError> {
    Ok(Json(state.store.get_tag_suggestion(id).await?))
}

async fn accept(
    _user: AuthUser,
    Path(id): Path<i64>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    let suggestion = state.store.get_tag_suggestion(id).await?;
    crate::services::tagging::apply_suggestion(&state.store, &suggestion).await?;
    state.store.set_tag_suggestion_status(id, "accepted").await?;
    Ok(StatusCode::OK)
}

async fn reject(
    _user: AuthUser,
    Path(id): Path<i64>,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    state.store.set_tag_suggestion_status(id, "rejected").await?;
    Ok(StatusCode::OK)
}

#[derive(serde::Deserialize)]
struct BatchAcceptBody {
    min_confidence: f32,
}

async fn batch_accept(
    _user: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<BatchAcceptBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    let suggestions = state.store.list_pending_tag_suggestions(None).await?;
    let mut accepted = 0usize;
    for s in suggestions.iter().filter(|s| s.confidence >= body.min_confidence) {
        crate::services::tagging::apply_suggestion(&state.store, s).await?;
        state.store.set_tag_suggestion_status(s.id, "accepted").await?;
        accepted += 1;
    }
    Ok(Json(serde_json::json!({"accepted": accepted})))
}
```

Mount in `src/api/mod.rs`:
```rust
.nest("/tag-suggestions", tag_suggestions::router())
```

**Step 4: Verify pass**
```bash
docker buildx build --progress=plain -t suzuran:dev . 2>&1 | tail -20
```

**Step 5: Update codebase filemap** — add `src/api/tag_suggestions.rs` and `tests/tag_suggestions_api.rs`.

**Step 6: Commit**
```bash
git add src/api/tag_suggestions.rs src/api/mod.rs tests/tag_suggestions_api.rs tasks/codebase-filemap.md
git commit -m "feat(3.6): tag suggestions REST API — list, count, accept, reject, batch-accept"
```

---

## Task 7: Accept flow — write tags to file and update DB

**Files:**
- Create: `src/services/tagging.rs`
- Modify: `src/services/mod.rs`
- Modify: `src/dal/mod.rs` — add `update_track_tags` to Store
- Modify: `src/dal/postgres.rs`, `src/dal/sqlite.rs`
- Create: `tests/tagging_service.rs`

**Step 1: Write the failing test**
```rust
// tests/tagging_service.rs
use suzuran_server::services::tagging::apply_suggestion;
use suzuran_server::dal::UpsertTagSuggestion;
mod common;

#[tokio::test]
async fn test_accept_writes_tags_to_file_and_db() {
    // setup_with_audio_track writes a real small FLAC/mp3 fixture to a temp dir
    let (store, track_id, audio_path) = common::setup_with_audio_track().await;

    let suggestion = store.create_tag_suggestion(UpsertTagSuggestion {
        track_id,
        source: "acoustid".into(),
        suggested_tags: serde_json::json!({
            "title": "Accepted Title",
            "artist": "Accepted Artist",
            "album":  "Accepted Album"
        }),
        confidence: 0.9,
        mb_recording_id: Some("rec-1".into()),
        mb_release_id: Some("rel-1".into()),
        cover_art_url: None,
    }).await.unwrap();

    apply_suggestion(&store, &suggestion).await.unwrap();

    // DB updated
    let track = store.get_track(track_id).await.unwrap();
    assert_eq!(track.tags["title"].as_str(), Some("Accepted Title"));
    assert_eq!(track.artist.as_deref(), Some("Accepted Artist"));

    // File updated (read tags back from disk)
    let on_disk = suzuran_server::tagger::read_tags(&audio_path).unwrap();
    assert_eq!(on_disk.get("title").map(String::as_str), Some("Accepted Title"));
}
```

**Step 2: Verify fail**
```bash
docker buildx build --progress=plain -t suzuran:dev . 2>&1 | tail -20
```

**Step 3: Add `update_track_tags` to Store**

`src/dal/mod.rs`:
```rust
async fn update_track_tags(&self, track_id: i64, tags: serde_json::Value) -> Result<(), AppError>;
```

Postgres impl — update `tags` JSONB and sync indexed columns:
```rust
async fn update_track_tags(&self, track_id: i64, tags: serde_json::Value) -> Result<(), AppError> {
    sqlx::query!(
        r#"UPDATE tracks SET
             tags         = $1,
             title        = ($1 ->> 'title'),
             artist       = ($1 ->> 'artist'),
             albumartist  = ($1 ->> 'albumartist'),
             album        = ($1 ->> 'album'),
             date         = ($1 ->> 'date'),
             genre        = ($1 ->> 'genre'),
             tracknumber  = ($1 ->> 'tracknumber'),
             discnumber   = ($1 ->> 'discnumber'),
             label        = ($1 ->> 'label'),
             catalognumber= ($1 ->> 'catalognumber')
           WHERE id = $2"#,
        tags, track_id
    )
    .execute(&self.pool)
    .await
    .map_err(AppError::from)?;
    Ok(())
}
```

SQLite impl: deserialize existing tags, merge, re-serialize, then UPDATE individual columns:
```rust
async fn update_track_tags(&self, track_id: i64, tags: serde_json::Value) -> Result<(), AppError> {
    let tags_str = serde_json::to_string(&tags).unwrap();
    let t = tags.as_object().cloned().unwrap_or_default();
    let get = |k: &str| t.get(k).and_then(|v| v.as_str()).map(str::to_string);
    sqlx::query!(
        r#"UPDATE tracks SET
             tags = ?, title = ?, artist = ?, albumartist = ?,
             album = ?, date = ?, genre = ?, tracknumber = ?,
             discnumber = ?, label = ?, catalognumber = ?
           WHERE id = ?"#,
        tags_str,
        get("title"), get("artist"), get("albumartist"),
        get("album"), get("date"), get("genre"), get("tracknumber"),
        get("discnumber"), get("label"), get("catalognumber"),
        track_id
    )
    .execute(&self.pool)
    .await
    .map_err(AppError::from)?;
    Ok(())
}
```

**Step 4: Implement `src/services/tagging.rs`**

```rust
use crate::{dal::Store, error::AppError, models::TagSuggestion, tagger};
use std::sync::Arc;

/// Apply an accepted tag suggestion: merge tags, write to audio file via lofty, update DB.
pub async fn apply_suggestion(
    store: &Arc<dyn Store>,
    suggestion: &TagSuggestion,
) -> Result<(), AppError> {
    let track   = store.get_track(suggestion.track_id).await?;
    let library = store.get_library(track.library_id).await?;

    let full_path = format!(
        "{}/{}",
        library.root_path.trim_end_matches('/'),
        track.relative_path.trim_start_matches('/')
    );

    // Merge: start with existing tags, overlay suggestion tags
    let mut merged = track.tags
        .as_object()
        .cloned()
        .unwrap_or_default();

    if let Some(suggested_obj) = suggestion.suggested_tags.as_object() {
        for (k, v) in suggested_obj {
            merged.insert(k.clone(), v.clone());
        }
    }

    // Write to audio file
    let string_map: std::collections::HashMap<String, String> = merged.iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
        .collect();

    tagger::write_tags(&full_path, &string_map)
        .map_err(|e| AppError::Internal(format!("lofty write failed: {e}")))?;

    // Update DB
    store.update_track_tags(suggestion.track_id, serde_json::Value::Object(merged)).await?;

    Ok(())
}
```

Export: `pub mod tagging;` in `src/services/mod.rs`.

**Step 5: Verify pass**
```bash
docker buildx build --progress=plain -t suzuran:dev . 2>&1 | tail -20
```

**Step 6: Update codebase filemap** — add `src/services/tagging.rs`, `tests/tagging_service.rs`.

**Step 7: Commit**
```bash
git add src/services/tagging.rs src/services/mod.rs src/dal/ tests/tagging_service.rs tasks/codebase-filemap.md
git commit -m "feat(3.7): tagging service — apply_suggestion writes tags to file and DB"
```

---

## Task 8: Inbox UI — shell, API client, nav badge

**Files:**
- Create: `ui/src/api/tagSuggestions.ts`
- Create: `ui/src/types/tagSuggestion.ts`
- Create: `ui/src/pages/InboxPage.tsx`
- Modify: `ui/src/components/TopNav.tsx` — Inbox link + badge
- Modify: `ui/src/App.tsx` — add `/inbox` route

**Step 1: Add type**

`ui/src/types/tagSuggestion.ts`:
```typescript
export interface TagSuggestion {
  id: number;
  track_id: number;
  source: 'acoustid' | 'mb_search' | 'freedb';
  suggested_tags: Record<string, string>;
  confidence: number;
  mb_recording_id?: string;
  mb_release_id?: string;
  cover_art_url?: string;
  status: 'pending' | 'accepted' | 'rejected';
  created_at: string;
}
```

**Step 2: Add API client**

`ui/src/api/tagSuggestions.ts`:
```typescript
import { client } from './client';
import type { TagSuggestion } from '../types/tagSuggestion';

export const tagSuggestionsApi = {
  listPending(trackId?: number) {
    return client
      .get<TagSuggestion[]>('/tag-suggestions', {
        params: trackId != null ? { track_id: trackId } : {},
      })
      .then(r => r.data);
  },

  count(): Promise<number> {
    return client
      .get<{ count: number }>('/tag-suggestions/count')
      .then(r => r.data.count);
  },

  accept(id: number) {
    return client.post(`/tag-suggestions/${id}/accept`);
  },

  reject(id: number) {
    return client.post(`/tag-suggestions/${id}/reject`);
  },

  batchAccept(minConfidence: number) {
    return client
      .post<{ accepted: number }>('/tag-suggestions/batch-accept', {
        min_confidence: minConfidence,
      })
      .then(r => r.data);
  },
};
```

**Step 3: Add Inbox page**

`ui/src/pages/InboxPage.tsx`:
```tsx
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { tagSuggestionsApi } from '../api/tagSuggestions';
import type { TagSuggestion } from '../types/tagSuggestion';

export default function InboxPage() {
  const qc = useQueryClient();

  const { data: suggestions = [], isLoading } = useQuery({
    queryKey: ['tag-suggestions'],
    queryFn: () => tagSuggestionsApi.listPending(),
  });

  const accept = useMutation({
    mutationFn: (id: number) => tagSuggestionsApi.accept(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['tag-suggestions'] });
      qc.invalidateQueries({ queryKey: ['inbox-count'] });
    },
  });

  const reject = useMutation({
    mutationFn: (id: number) => tagSuggestionsApi.reject(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['tag-suggestions'] });
      qc.invalidateQueries({ queryKey: ['inbox-count'] });
    },
  });

  const batchAccept = useMutation({
    mutationFn: () => tagSuggestionsApi.batchAccept(0.8),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['tag-suggestions'] });
      qc.invalidateQueries({ queryKey: ['inbox-count'] });
    },
  });

  if (isLoading) {
    return <div className="p-4 text-muted-foreground text-sm">Loading…</div>;
  }

  return (
    <div className="flex flex-col h-full">
      {/* Toolbar */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-border">
        <span className="text-sm text-muted-foreground">
          {suggestions.length === 0
            ? 'No pending suggestions'
            : `${suggestions.length} pending suggestion${suggestions.length !== 1 ? 's' : ''}`}
        </span>
        {suggestions.length > 0 && (
          <button
            onClick={() => batchAccept.mutate()}
            disabled={batchAccept.isPending}
            className="px-3 py-1 text-xs bg-primary text-primary-foreground rounded
                       hover:bg-primary/90 disabled:opacity-50"
          >
            Accept all ≥ 80%
          </button>
        )}
      </div>

      {/* Suggestion list */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {suggestions.length === 0 ? (
          <p className="text-center text-muted-foreground text-sm pt-12">Inbox is empty</p>
        ) : (
          suggestions.map(s => (
            <SuggestionCard
              key={s.id}
              suggestion={s}
              onAccept={() => accept.mutate(s.id)}
              onReject={() => reject.mutate(s.id)}
              isPending={accept.isPending || reject.isPending}
            />
          ))
        )}
      </div>
    </div>
  );
}

function SuggestionCard({
  suggestion,
  onAccept,
  onReject,
  isPending,
}: {
  suggestion: TagSuggestion;
  onAccept: () => void;
  onReject: () => void;
  isPending: boolean;
}) {
  const pct = Math.round(suggestion.confidence * 100);

  return (
    <div className="border border-border rounded bg-card">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-border">
        <div className="flex items-center gap-2">
          <span className="text-xs uppercase tracking-wide text-muted-foreground font-mono">
            {suggestion.source}
          </span>
          <span
            className={`text-xs font-mono ${pct >= 80 ? 'text-green-400' : 'text-yellow-400'}`}
          >
            {pct}%
          </span>
        </div>
        <span className="text-xs text-muted-foreground font-mono">
          track #{suggestion.track_id}
        </span>
      </div>

      {/* Cover art + tag table */}
      <div className="flex gap-4 p-4">
        {suggestion.cover_art_url && (
          <img
            src={suggestion.cover_art_url}
            alt="cover"
            className="w-20 h-20 object-cover rounded border border-border flex-shrink-0"
            onError={e => { (e.currentTarget as HTMLImageElement).style.display = 'none'; }}
          />
        )}
        <div className="flex-1 min-w-0">
          {/* Tag list (no diff yet — diff added in Task 9) */}
          <dl className="grid grid-cols-[8rem_1fr] gap-x-2 gap-y-0.5 text-sm">
            {Object.entries(suggestion.suggested_tags).map(([k, v]) => (
              <React.Fragment key={k}>
                <dt className="text-muted-foreground font-mono text-xs truncate">{k}</dt>
                <dd className="truncate">{v}</dd>
              </React.Fragment>
            ))}
          </dl>
        </div>
      </div>

      {/* Actions */}
      <div className="flex gap-2 px-4 py-2 border-t border-border">
        <button
          onClick={onAccept}
          disabled={isPending}
          className="px-3 py-1 text-sm bg-primary text-primary-foreground rounded
                     hover:bg-primary/90 disabled:opacity-50"
        >
          Accept
        </button>
        <button
          onClick={onReject}
          disabled={isPending}
          className="px-3 py-1 text-sm border border-border rounded
                     hover:bg-muted disabled:opacity-50"
        >
          Reject
        </button>
      </div>
    </div>
  );
}
```

Add `import React from 'react';` at the top if not using JSX transform.

**Step 4: Add nav badge to `ui/src/components/TopNav.tsx`**

Add this query near the top of the component:
```tsx
const { data: inboxCount = 0 } = useQuery({
  queryKey: ['inbox-count'],
  queryFn: () => tagSuggestionsApi.count(),
  refetchInterval: 30_000,
});
```

Add Inbox link to nav items (alongside Library, Jobs, Settings):
```tsx
<NavLink to="/inbox" className={navLinkClass}>
  Inbox
  {inboxCount > 0 && (
    <span className="ml-1.5 inline-flex items-center justify-center
                     h-4 min-w-[1rem] px-1 rounded-full
                     text-[10px] font-bold
                     bg-primary text-primary-foreground">
      {inboxCount > 99 ? '99+' : inboxCount}
    </span>
  )}
</NavLink>
```

**Step 5: Add route in `ui/src/App.tsx`**:
```tsx
import InboxPage from './pages/InboxPage';
// ...
<Route path="/inbox" element={<InboxPage />} />
```

**Step 6: Build and verify in browser**
```bash
docker compose up --build -d
# Open http://localhost:3000/inbox
# Should show "Inbox is empty" with no errors
# Nav should show "Inbox" link with no badge (count=0)
```

**Step 7: Update codebase filemap** — add new UI files.

**Step 8: Commit**
```bash
git add ui/src/api/tagSuggestions.ts ui/src/types/tagSuggestion.ts ui/src/pages/InboxPage.tsx ui/src/components/TopNav.tsx ui/src/App.tsx tasks/codebase-filemap.md
git commit -m "feat(3.8): Inbox UI shell — suggestion list, accept/reject, nav badge"
```

---

## Task 9: Inbox — tag diff view

**Files:**
- Create: `ui/src/api/tracks.ts` (or extend existing) — add `getTrack(id)` if not present
- Create: `ui/src/components/TagDiffTable.tsx`
- Modify: `ui/src/pages/InboxPage.tsx` — replace flat tag list with `TagDiffTable`

**Background:** The diff view shows current file tags vs. suggested tags side-by-side, with changed fields highlighted. It needs the track's current `tags` field. Add a `GET /api/v1/tracks/:id` endpoint if not already present (check `src/api/tracks.rs` — the existing endpoint is `GET /:id/stream`; a metadata GET is needed). Or we can include tags in the suggestion list response by joining in the backend.

**Simpler approach:** Add `GET /tracks/:id` to `src/api/tracks.rs` returning the `Track` model; the frontend fetches it per suggestion.

**Step 1: Add `GET /tracks/:id` endpoint**

In `src/api/tracks.rs`, add:
```rust
async fn get_track(
    _user: AuthUser,
    Path(id): Path<i64>,
    State(state): State<AppState>,
) -> Result<Json<Track>, AppError> {
    Ok(Json(state.store.get_track(id).await?))
}
```

Register: `.route("/:id", get(get_track))` alongside the existing stream routes.

**Step 2: Add `getTrack` to UI API client**

`ui/src/api/tracks.ts`:
```typescript
import { client } from './client';
import type { Track } from '../types/track';

export const tracksApi = {
  getTrack(id: number): Promise<Track> {
    return client.get<Track>(`/tracks/${id}`).then(r => r.data);
  },
};
```

Add `Track` type to `ui/src/types/track.ts`:
```typescript
export interface Track {
  id: number;
  library_id: number;
  relative_path: string;
  title?: string;
  artist?: string;
  albumartist?: string;
  album?: string;
  tracknumber?: string;
  date?: string;
  genre?: string;
  tags: Record<string, unknown>;
}
```

**Step 3: Implement `TagDiffTable`**

`ui/src/components/TagDiffTable.tsx`:
```tsx
import { useQuery } from '@tanstack/react-query';
import { tracksApi } from '../api/tracks';

const ORDERED_KEYS = [
  'title', 'artist', 'albumartist', 'album', 'date', 'genre',
  'tracknumber', 'discnumber', 'totaltracks', 'totaldiscs',
  'label', 'catalognumber', 'composer',
  'musicbrainz_recordingid', 'musicbrainz_releaseid',
];

interface Props {
  trackId: number;
  suggestedTags: Record<string, string>;
}

export function TagDiffTable({ trackId, suggestedTags }: Props) {
  const { data: track } = useQuery({
    queryKey: ['track', trackId],
    queryFn: () => tracksApi.getTrack(trackId),
  });

  const current: Record<string, string> = Object.fromEntries(
    Object.entries(track?.tags ?? {}).filter(([, v]) => typeof v === 'string') as [string, string][]
  );

  // Show ordered keys first, then any extra keys from suggested
  const extraKeys = Object.keys(suggestedTags).filter(k => !ORDERED_KEYS.includes(k));
  const keys = [
    ...ORDERED_KEYS.filter(k => suggestedTags[k] || current[k]),
    ...extraKeys,
  ];

  if (keys.length === 0) return null;

  return (
    <table className="w-full text-xs border-collapse">
      <thead>
        <tr className="text-muted-foreground">
          <th className="text-left pb-1 pr-3 w-36 font-normal">Field</th>
          <th className="text-left pb-1 pr-3 font-normal">Current</th>
          <th className="text-left pb-1 font-normal">Suggested</th>
        </tr>
      </thead>
      <tbody>
        {keys.map(key => {
          const cur = current[key] ?? '';
          const sug = suggestedTags[key] ?? '';
          const changed = cur !== sug;
          return (
            <tr key={key} className={changed ? 'bg-yellow-500/5' : ''}>
              <td className="py-px pr-3 font-mono text-muted-foreground">{key}</td>
              <td className={`py-px pr-3 ${changed ? 'text-muted-foreground line-through' : ''}`}>
                {cur || <span className="italic text-muted-foreground/40">—</span>}
              </td>
              <td className={`py-px ${changed ? 'text-green-400' : ''}`}>
                {sug || <span className="italic text-muted-foreground/40">—</span>}
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}
```

**Step 4: Replace flat tag list in InboxPage**

In `SuggestionCard`, replace the `<dl>` block with:
```tsx
import { TagDiffTable } from '../components/TagDiffTable';
// ...
<TagDiffTable
  trackId={suggestion.track_id}
  suggestedTags={suggestion.suggested_tags}
/>
```

**Step 5: Build and verify in browser**
```bash
docker compose up --build -d
# Seed a suggestion via curl or the Jobs UI
# Navigate to /inbox
# The suggestion card should show the two-column diff table
# Changed fields should be highlighted yellow/strikethrough + green
```

**Step 6: Update codebase filemap** — add `ui/src/components/TagDiffTable.tsx`.

**Step 7: Commit**
```bash
git add src/api/tracks.rs ui/src/api/tracks.ts ui/src/types/track.ts ui/src/components/TagDiffTable.tsx ui/src/pages/InboxPage.tsx tasks/codebase-filemap.md
git commit -m "feat(3.9): Inbox tag diff view — current vs suggested tags, cover art"
```

---

## Task 10: Scheduler wiring + phase complete

**Files:**
- Modify: `src/scheduler/mod.rs` — register `fingerprint`, `mb_lookup`, `freedb_lookup` handlers
- Modify: `src/state.rs` — add `Arc<FreedBService>` if not already there
- Modify: `src/main.rs` — construct `FreedBService`, pass to scheduler

**Step 1: Wire new job handlers into the scheduler**

In `src/scheduler/mod.rs`, the dispatch match (or if/else chain) that routes `job.job_type` to a handler needs three new arms:

```rust
"fingerprint" => {
    FingerprintJobHandler::new(state.store.clone())
        .handle(payload).await
}
"mb_lookup" => {
    MbLookupJobHandler::new(state.store.clone(), state.mb_service.clone())
        .handle(payload).await
}
"freedb_lookup" => {
    FreedBLookupJobHandler::new(state.store.clone(), state.freedb_service.clone())
        .handle(payload).await
}
```

**Step 2: Add `Arc<FreedBService>` to AppState** (if not added in Task 5):
```rust
pub struct AppState {
    pub store: Arc<dyn Store>,
    pub config: Arc<Config>,
    pub webauthn: Arc<Webauthn>,
    pub mb_service: Arc<MusicBrainzService>,
    pub freedb_service: Arc<FreedBService>,
}
```

In `src/main.rs`:
```rust
let freedb_service = Arc::new(FreedBService::new());
```

**Step 3: Verify pass**
```bash
docker buildx build --progress=plain -t suzuran:dev . 2>&1 | tail -20
```
Expected: BUILD SUCCESS, all tests pass.

**Step 4: End-to-end smoke test**
```bash
docker compose up --build -d
docker compose logs -f app
```
1. Trigger a library scan from the UI → Jobs page shows `scan` job completing
2. After scan: `fingerprint` jobs appear (one per new track)
3. After fingerprint: `mb_lookup` jobs appear
4. If AcoustID key is configured in env and tracks matched: nav badge shows pending count
5. Navigate to `/inbox` → suggestion cards visible with diff tables
6. Accept one → badge decrements, card disappears

**Step 5: Update CHANGELOG.md** with v0.3.0 entry:
```markdown
## [v0.3.0] — 2026-04-19

### Added
- Acoustic fingerprinting via fpcalc (Chromaprint) — runs automatically after scan for new tracks
- AcoustID + MusicBrainz metadata lookup job chain — suggestions written to `tag_suggestions` table
- gnudb.org (FreeDB) disc-ID lookup fallback — activates when DISCID tag present, mb_lookup finds no matches
- Tag suggestions REST API (`/api/v1/tag-suggestions`) — list, accept, reject, batch-accept
- Tagging service — apply_suggestion merges and writes tags to audio file via lofty, syncs DB
- Inbox UI — nav badge with live count, suggestion cards with tag diff view and cover art
- Batch accept action (≥ 80% confidence default)
```

**Step 6: Commit**
```bash
git add src/scheduler/mod.rs src/state.rs src/main.rs CHANGELOG.md tasks/codebase-filemap.md
git commit -m "feat(3.10): scheduler wiring — fingerprint/mb_lookup/freedb_lookup handlers + CHANGELOG"
```

**Step 7: Tag the release**
```bash
git tag v0.3.0
```

---

## Summary

| Task | Output | Commit message |
|------|--------|----------------|
| 1 | tag_suggestions migration + DAL | `feat(3.1)` |
| 2 | fingerprint job + scan auto-enqueue | `feat(3.2)` |
| 3 | MusicBrainz/AcoustID HTTP service | `feat(3.3)` |
| 4 | MB lookup job (AcoustID + MB API) | `feat(3.4)` |
| 5 | gnudb.org FreeDB disc-ID fallback | `feat(3.5)` |
| 6 | Tag suggestions REST API | `feat(3.6)` |
| 7 | Accept flow — lofty write + DB sync | `feat(3.7)` |
| 8 | Inbox UI shell + nav badge | `feat(3.8)` |
| 9 | Tag diff view + cover art | `feat(3.9)` |
| 10 | Scheduler wiring + release | `feat(3.10)` |
