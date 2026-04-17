# Phase 1.6 — Settings + Theming Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the settings key-value service and CRUD API, plus the themes table service and API. No UI yet — this exposes the data layer that the UI will consume in Phase 1.10.

**Architecture:** `SettingsService` wraps the `settings` table with typed get/set helpers. `ThemeService` wraps `themes` with CRUD. Both are thin — no caching, no complex logic. Admin-only write endpoints; read endpoints available to all authenticated users.

**Tech Stack:** sqlx (already present), serde_json for CSS vars JSONB.

---

## File Map

| File | Action | Purpose |
|------|--------|---------|
| `src/dal/mod.rs` | Modify | Add settings + themes Store methods |
| `src/dal/postgres.rs` | Modify | Implement settings + themes queries |
| `src/dal/sqlite.rs` | Modify | Implement settings + themes queries |
| `src/models/mod.rs` | Modify | Add `Setting`, `Theme` structs |
| `src/api/settings.rs` | Create | Settings handlers + routes |
| `src/api/themes.rs` | Create | Themes handlers + routes |
| `src/api/mod.rs` | Modify | Mount settings + themes routes |
| `src/api/middleware/admin.rs` | Create | `AdminUser` extractor (role = admin) |
| `src/api/middleware/mod.rs` | Modify | Add `pub mod admin` |
| `tests/settings.rs` | Create | Settings get/set integration tests |

---

## Task 1: Models

**Files:**
- Modify: `src/models/mod.rs`

- [ ] **Step 1: Append `Setting` and `Theme` to `src/models/mod.rs`**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Setting {
    pub key: String,
    pub value: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Theme {
    pub id: i64,
    pub name: String,
    pub css_vars: serde_json::Value,
    pub accent_color: Option<String>,
    pub background_url: Option<String>,
    pub created_at: DateTime<Utc>,
}
```

> **SQLite note:** `css_vars` is stored as `TEXT` in SQLite. sqlx maps `TEXT` → `serde_json::Value` automatically when the `json` feature is enabled (already in Cargo.toml). Confirm `sqlx = { features = [..., "json"] }` is present.

---

## Task 2: Store trait — settings + themes

**Files:**
- Modify: `src/dal/mod.rs`

- [ ] **Step 1: Add settings and themes methods to the `Store` trait**

```rust
// ── settings ──────────────────────────────────────────────────
async fn get_setting(&self, key: &str) -> Result<Option<Setting>, AppError>;
async fn get_all_settings(&self) -> Result<Vec<Setting>, AppError>;
async fn set_setting(&self, key: &str, value: &str) -> Result<Setting, AppError>;

// ── themes ────────────────────────────────────────────────────
async fn list_themes(&self) -> Result<Vec<Theme>, AppError>;
async fn get_theme(&self, id: i64) -> Result<Option<Theme>, AppError>;
async fn create_theme(
    &self,
    name: &str,
    css_vars: serde_json::Value,
    accent_color: Option<&str>,
    background_url: Option<&str>,
) -> Result<Theme, AppError>;
async fn update_theme(
    &self,
    id: i64,
    name: &str,
    css_vars: serde_json::Value,
    accent_color: Option<&str>,
    background_url: Option<&str>,
) -> Result<Option<Theme>, AppError>;
async fn delete_theme(&self, id: i64) -> Result<(), AppError>;
```

Add `use crate::models::{..., Setting, Theme};` to the import list.

---

## Task 3: Postgres implementations

**Files:**
- Modify: `src/dal/postgres.rs`

- [ ] **Step 1: Append settings implementations to `impl Store for PgStore`**

```rust
async fn get_setting(&self, key: &str) -> Result<Option<Setting>, AppError> {
    sqlx::query_as::<_, Setting>("SELECT * FROM settings WHERE key = $1")
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)
}

async fn get_all_settings(&self) -> Result<Vec<Setting>, AppError> {
    sqlx::query_as::<_, Setting>("SELECT * FROM settings ORDER BY key")
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)
}

async fn set_setting(&self, key: &str, value: &str) -> Result<Setting, AppError> {
    sqlx::query_as::<_, Setting>(
        "INSERT INTO settings (key, value, updated_at) VALUES ($1, $2, NOW())
         ON CONFLICT (key) DO UPDATE SET value = $2, updated_at = NOW()
         RETURNING *",
    )
    .bind(key)
    .bind(value)
    .fetch_one(&self.pool)
    .await
    .map_err(AppError::Database)
}
```

- [ ] **Step 2: Append themes implementations**

```rust
async fn list_themes(&self) -> Result<Vec<Theme>, AppError> {
    sqlx::query_as::<_, Theme>("SELECT * FROM themes ORDER BY name")
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)
}

async fn get_theme(&self, id: i64) -> Result<Option<Theme>, AppError> {
    sqlx::query_as::<_, Theme>("SELECT * FROM themes WHERE id = $1")
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)
}

async fn create_theme(
    &self,
    name: &str,
    css_vars: serde_json::Value,
    accent_color: Option<&str>,
    background_url: Option<&str>,
) -> Result<Theme, AppError> {
    sqlx::query_as::<_, Theme>(
        "INSERT INTO themes (name, css_vars, accent_color, background_url)
         VALUES ($1, $2, $3, $4)
         RETURNING *",
    )
    .bind(name)
    .bind(css_vars)
    .bind(accent_color)
    .bind(background_url)
    .fetch_one(&self.pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db) if db.constraint() == Some("themes_name_key") => {
            AppError::BadRequest("theme name already exists".into())
        }
        other => AppError::Database(other),
    })
}

async fn update_theme(
    &self,
    id: i64,
    name: &str,
    css_vars: serde_json::Value,
    accent_color: Option<&str>,
    background_url: Option<&str>,
) -> Result<Option<Theme>, AppError> {
    sqlx::query_as::<_, Theme>(
        "UPDATE themes SET name=$1, css_vars=$2, accent_color=$3, background_url=$4
         WHERE id=$5
         RETURNING *",
    )
    .bind(name)
    .bind(css_vars)
    .bind(accent_color)
    .bind(background_url)
    .bind(id)
    .fetch_optional(&self.pool)
    .await
    .map_err(AppError::Database)
}

async fn delete_theme(&self, id: i64) -> Result<(), AppError> {
    sqlx::query("DELETE FROM themes WHERE id = $1")
        .bind(id)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(AppError::Database)
}
```

---

## Task 4: SQLite implementations

**Files:**
- Modify: `src/dal/sqlite.rs`

- [ ] **Step 1: Append settings implementations to `impl Store for SqliteStore`**

```rust
async fn get_setting(&self, key: &str) -> Result<Option<Setting>, AppError> {
    sqlx::query_as::<_, Setting>("SELECT * FROM settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)
}

async fn get_all_settings(&self) -> Result<Vec<Setting>, AppError> {
    sqlx::query_as::<_, Setting>("SELECT * FROM settings ORDER BY key")
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)
}

async fn set_setting(&self, key: &str, value: &str) -> Result<Setting, AppError> {
    sqlx::query_as::<_, Setting>(
        "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, datetime('now'))
         ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = datetime('now')
         RETURNING *",
    )
    .bind(key)
    .bind(value)
    .fetch_one(&self.pool)
    .await
    .map_err(AppError::Database)
}
```

- [ ] **Step 2: Append themes implementations**

```rust
async fn list_themes(&self) -> Result<Vec<Theme>, AppError> {
    sqlx::query_as::<_, Theme>("SELECT * FROM themes ORDER BY name")
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)
}

async fn get_theme(&self, id: i64) -> Result<Option<Theme>, AppError> {
    sqlx::query_as::<_, Theme>("SELECT * FROM themes WHERE id = ?1")
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)
}

async fn create_theme(
    &self,
    name: &str,
    css_vars: serde_json::Value,
    accent_color: Option<&str>,
    background_url: Option<&str>,
) -> Result<Theme, AppError> {
    sqlx::query_as::<_, Theme>(
        "INSERT INTO themes (name, css_vars, accent_color, background_url)
         VALUES (?1, ?2, ?3, ?4)
         RETURNING *",
    )
    .bind(name)
    .bind(css_vars)
    .bind(accent_color)
    .bind(background_url)
    .fetch_one(&self.pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db) if db.message().contains("UNIQUE constraint failed: themes.name") => {
            AppError::BadRequest("theme name already exists".into())
        }
        other => AppError::Database(other),
    })
}

async fn update_theme(
    &self,
    id: i64,
    name: &str,
    css_vars: serde_json::Value,
    accent_color: Option<&str>,
    background_url: Option<&str>,
) -> Result<Option<Theme>, AppError> {
    sqlx::query_as::<_, Theme>(
        "UPDATE themes SET name=?1, css_vars=?2, accent_color=?3, background_url=?4
         WHERE id=?5
         RETURNING *",
    )
    .bind(name)
    .bind(css_vars)
    .bind(accent_color)
    .bind(background_url)
    .bind(id)
    .fetch_optional(&self.pool)
    .await
    .map_err(AppError::Database)
}

async fn delete_theme(&self, id: i64) -> Result<(), AppError> {
    sqlx::query("DELETE FROM themes WHERE id = ?1")
        .bind(id)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(AppError::Database)
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
git commit -m "feat: settings + themes Store trait methods and Postgres/SQLite impls"
```

---

## Task 5: AdminUser extractor

**Files:**
- Create: `src/api/middleware/admin.rs`
- Modify: `src/api/middleware/mod.rs`

- [ ] **Step 1: Add `pub mod admin;` to `src/api/middleware/mod.rs`**

- [ ] **Step 2: Write `src/api/middleware/admin.rs`**

```rust
use axum::{async_trait, extract::FromRequestParts, http::request::Parts};

use crate::{
    api::middleware::auth::AuthUser,
    error::AppError,
    models::User,
    state::AppState,
};

/// Requires authentication AND admin role.
pub struct AdminUser(pub User);

#[async_trait]
impl FromRequestParts<AppState> for AdminUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let AuthUser(user) = AuthUser::from_request_parts(parts, state).await?;
        if user.role != "admin" {
            return Err(AppError::Forbidden);
        }
        Ok(AdminUser(user))
    }
}
```

---

## Task 6: Settings API handlers

**Files:**
- Create: `src/api/settings.rs`

- [ ] **Step 1: Write `src/api/settings.rs`**

```rust
use axum::{
    extract::{Path, State},
    routing::{get, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::{
    api::middleware::{admin::AdminUser, auth::AuthUser},
    error::AppError,
    models::Setting,
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_settings))
        .route("/:key", get(get_setting).put(set_setting))
}

async fn list_settings(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Result<Json<Vec<Setting>>, AppError> {
    let settings = state.db.get_all_settings().await?;
    Ok(Json(settings))
}

async fn get_setting(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(key): Path<String>,
) -> Result<Json<Setting>, AppError> {
    state
        .db
        .get_setting(&key)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("setting '{key}' not found")))
        .map(Json)
}

#[derive(Deserialize)]
struct SetSettingRequest {
    value: String,
}

async fn set_setting(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(key): Path<String>,
    Json(body): Json<SetSettingRequest>,
) -> Result<Json<Setting>, AppError> {
    let setting = state.db.set_setting(&key, &body.value).await?;
    Ok(Json(setting))
}
```

---

## Task 7: Themes API handlers

**Files:**
- Create: `src/api/themes.rs`

- [ ] **Step 1: Write `src/api/themes.rs`**

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::Deserialize;

use crate::{
    api::middleware::{admin::AdminUser, auth::AuthUser},
    error::AppError,
    models::Theme,
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_themes).post(create_theme))
        .route("/:id", get(get_theme).put(update_theme).delete(delete_theme))
}

async fn list_themes(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Result<Json<Vec<Theme>>, AppError> {
    Ok(Json(state.db.list_themes().await?))
}

async fn get_theme(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<Theme>, AppError> {
    state
        .db
        .get_theme(id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("theme {id} not found")))
        .map(Json)
}

#[derive(Deserialize)]
struct ThemeRequest {
    name: String,
    css_vars: serde_json::Value,
    accent_color: Option<String>,
    background_url: Option<String>,
}

async fn create_theme(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(body): Json<ThemeRequest>,
) -> Result<(StatusCode, Json<Theme>), AppError> {
    let theme = state
        .db
        .create_theme(
            &body.name,
            body.css_vars,
            body.accent_color.as_deref(),
            body.background_url.as_deref(),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(theme)))
}

async fn update_theme(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
    Json(body): Json<ThemeRequest>,
) -> Result<Json<Theme>, AppError> {
    state
        .db
        .update_theme(
            id,
            &body.name,
            body.css_vars,
            body.accent_color.as_deref(),
            body.background_url.as_deref(),
        )
        .await?
        .ok_or_else(|| AppError::NotFound(format!("theme {id} not found")))
        .map(Json)
}

async fn delete_theme(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    state.db.delete_theme(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
```

---

## Task 8: Mount routes

**Files:**
- Modify: `src/api/mod.rs`

- [ ] **Step 1: Update `src/api/mod.rs`**

```rust
pub mod auth;
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
git commit -m "feat: settings and themes API (admin-gated writes, auth-gated reads)"
```

---

## Task 9: Integration tests

**Files:**
- Create: `tests/settings.rs`

- [ ] **Step 1: Write `tests/settings.rs`**

```rust
use std::sync::Arc;
use url::Url;
use webauthn_rs::WebauthnBuilder;
use suzuran_server::{build_router, config::Config, dal::sqlite::SqliteStore, state::AppState};

async fn spawn_test_server() -> String {
    let store = SqliteStore::new("sqlite::memory:").await.unwrap();
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

    let state = AppState::new(Arc::new(store), config, webauthn);
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
async fn settings_list_requires_auth() {
    let base = spawn_test_server().await;
    let res = reqwest::get(format!("{base}/api/v1/settings/")).await.unwrap();
    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn settings_list_returns_defaults() {
    let base = spawn_test_server().await;
    let client = admin_client(&base).await;
    let res = client.get(format!("{base}/api/v1/settings/")).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: Vec<serde_json::Value> = res.json().await.unwrap();
    assert!(!body.is_empty());
    let keys: Vec<&str> = body.iter().filter_map(|s| s["key"].as_str()).collect();
    assert!(keys.contains(&"mb_rate_limit_ms"));
}

#[tokio::test]
async fn admin_can_update_setting() {
    let base = spawn_test_server().await;
    let client = admin_client(&base).await;

    let res = client
        .put(format!("{base}/api/v1/settings/mb_rate_limit_ms"))
        .json(&serde_json::json!({"value": "2000"}))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["value"], "2000");
}

#[tokio::test]
async fn themes_crud() {
    let base = spawn_test_server().await;
    let client = admin_client(&base).await;

    // Create
    let res = client
        .post(format!("{base}/api/v1/themes/"))
        .json(&serde_json::json!({
            "name": "Midnight",
            "css_vars": {"--bg": "#0a0a0f"},
            "accent_color": "#4f8ef7"
        }))
        .send().await.unwrap();
    assert_eq!(res.status(), 201);
    let theme: serde_json::Value = res.json().await.unwrap();
    let id = theme["id"].as_i64().unwrap();

    // Get
    let res = client.get(format!("{base}/api/v1/themes/{id}")).send().await.unwrap();
    assert_eq!(res.status(), 200);

    // Delete
    let res = client.delete(format!("{base}/api/v1/themes/{id}")).send().await.unwrap();
    assert_eq!(res.status(), 204);

    // Confirm gone
    let res = client.get(format!("{base}/api/v1/themes/{id}")).send().await.unwrap();
    assert_eq!(res.status(), 404);
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test --test settings -- --nocapture
```

Expected: all 4 tests pass.

- [ ] **Step 3: Commit**

```bash
git add tests/settings.rs tasks/codebase-filemap.md
git commit -m "test: settings and themes integration tests; update filemap"
```
