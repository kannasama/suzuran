# Phase 1.4 — Auth: Password + Sessions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement password-based authentication — register, login, logout, and a protected `me` endpoint — using Argon2 password hashing, JWT-signed session tokens stored in HttpOnly cookies, and server-side session rows for revocation.

**Architecture:** Session tokens are JWTs signed with `JWT_SECRET`. The raw JWT is stored as a SHA-256 hash in the `sessions` table, enabling server-side logout. The `AuthUser` extractor verifies the JWT signature, hashes the token, then confirms the session row exists (not revoked). Models live in `src/models/`. Store trait gains user/session methods. `AuthService` is stateless and takes no DB reference — callers pass a `&dyn Store`.

**Tech Stack:** argon2 0.5, jsonwebtoken 9, chrono (via sqlx feature), sha2 for token hashing.

---

## File Map

| File | Action | Purpose |
|------|--------|---------|
| `src/models/mod.rs` | Create | `User`, `Session` structs |
| `src/dal/mod.rs` | Modify | Add user/session methods to `Store` trait |
| `src/dal/postgres.rs` | Modify | Implement user/session queries (Postgres) |
| `src/dal/sqlite.rs` | Modify | Implement user/session queries (SQLite) |
| `src/services/mod.rs` | Create | `pub mod auth` |
| `src/services/auth.rs` | Create | `AuthService` — hash, verify, JWT, token hash |
| `src/api/mod.rs` | Create | `pub fn api_router(state: AppState) -> Router` |
| `src/api/auth.rs` | Create | register, login, logout, me handlers + route fn |
| `src/api/middleware/mod.rs` | Create | `pub mod auth` |
| `src/api/middleware/auth.rs` | Create | `AuthUser` extractor (`FromRequestParts`) |
| `src/lib.rs` | Modify | Expose `models`, `services`, `api` modules |
| `src/app.rs` | Modify | Mount API router under `/api/v1` |
| `tests/auth.rs` | Create | register, login, logout, auth-required tests |

---

## Task 1: Models

**Files:**
- Create: `src/models/mod.rs`

- [ ] **Step 1: Write `src/models/mod.rs`**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: String,
    pub force_password_change: bool,
    pub totp_required: bool,
    pub webauthn_required: bool,
    pub accent_color: Option<String>,
    pub base_theme: String,
    pub theme_id: Option<i64>,
    pub display_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Session {
    pub id: i64,
    pub user_id: i64,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
```

> **SQLite note:** sqlx maps SQLite `INTEGER` columns to `bool` automatically when the Rust field is `bool`. Timestamps stored as `TEXT` in ISO 8601 format map to `DateTime<Utc>` via the `chrono` feature.

---

## Task 2: Extend Store trait with user/session methods

**Files:**
- Modify: `src/dal/mod.rs`

- [ ] **Step 1: Add user and session methods to the Store trait**

```rust
pub mod postgres;
pub mod sqlite;

use chrono::{DateTime, Utc};

use crate::{error::AppError, models::{Session, User}};

#[async_trait::async_trait]
pub trait Store: Send + Sync {
    // ── connectivity ──────────────────────────────────────────────
    async fn health_check(&self) -> Result<(), AppError>;

    // ── users ─────────────────────────────────────────────────────
    async fn count_users(&self) -> Result<i64, AppError>;
    async fn create_user(
        &self,
        username: &str,
        email: &str,
        password_hash: &str,
        role: &str,
    ) -> Result<User, AppError>;
    async fn find_user_by_username(&self, username: &str) -> Result<Option<User>, AppError>;
    async fn find_user_by_id(&self, id: i64) -> Result<Option<User>, AppError>;

    // ── sessions ──────────────────────────────────────────────────
    async fn create_session(
        &self,
        user_id: i64,
        token_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<Session, AppError>;
    async fn find_session_by_token_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<Session>, AppError>;
    async fn delete_session(&self, id: i64) -> Result<(), AppError>;
}
```

---

## Task 3: Implement Store methods — Postgres

**Files:**
- Modify: `src/dal/postgres.rs`

- [ ] **Step 1: Add imports and user/session implementations**

Append to `src/dal/postgres.rs` (after the existing `impl Store for PgStore` block — replace the whole `impl` block):

```rust
use chrono::{DateTime, Utc};

use crate::{dal::Store, error::AppError, models::{Session, User}};
use sqlx::PgPool;

pub struct PgStore {
    pool: PgPool,
}

impl PgStore {
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        let pool = PgPool::connect(database_url).await?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> anyhow::Result<()> {
        sqlx::migrate!("migrations/postgres").run(&self.pool).await?;
        Ok(())
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait::async_trait]
impl Store for PgStore {
    async fn health_check(&self) -> Result<(), AppError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }

    async fn count_users(&self) -> Result<i64, AppError> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    async fn create_user(
        &self,
        username: &str,
        email: &str,
        password_hash: &str,
        role: &str,
    ) -> Result<User, AppError> {
        sqlx::query_as::<_, User>(
            "INSERT INTO users (username, email, password_hash, role)
             VALUES ($1, $2, $3, $4)
             RETURNING *",
        )
        .bind(username)
        .bind(email)
        .bind(password_hash)
        .bind(role)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(ref db) if db.constraint() == Some("users_username_key") => {
                AppError::BadRequest("username already taken".into())
            }
            sqlx::Error::Database(ref db) if db.constraint() == Some("users_email_key") => {
                AppError::BadRequest("email already registered".into())
            }
            other => AppError::Database(other),
        })
    }

    async fn find_user_by_username(&self, username: &str) -> Result<Option<User>, AppError> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = $1")
            .bind(username)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_user_by_id(&self, id: i64) -> Result<Option<User>, AppError> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn create_session(
        &self,
        user_id: i64,
        token_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<Session, AppError> {
        sqlx::query_as::<_, Session>(
            "INSERT INTO sessions (user_id, token_hash, expires_at)
             VALUES ($1, $2, $3)
             RETURNING *",
        )
        .bind(user_id)
        .bind(token_hash)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn find_session_by_token_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<Session>, AppError> {
        sqlx::query_as::<_, Session>(
            "SELECT * FROM sessions WHERE token_hash = $1 AND expires_at > NOW()",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn delete_session(&self, id: i64) -> Result<(), AppError> {
        sqlx::query("DELETE FROM sessions WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }
}
```

---

## Task 4: Implement Store methods — SQLite

**Files:**
- Modify: `src/dal/sqlite.rs`

- [ ] **Step 1: Replace `src/dal/sqlite.rs` with full implementation**

```rust
use chrono::{DateTime, Utc};

use crate::{dal::Store, error::AppError, models::{Session, User}};
use sqlx::SqlitePool;

pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        let pool = SqlitePool::connect(database_url).await?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> anyhow::Result<()> {
        sqlx::migrate!("migrations/sqlite").run(&self.pool).await?;
        Ok(())
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[async_trait::async_trait]
impl Store for SqliteStore {
    async fn health_check(&self) -> Result<(), AppError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }

    async fn count_users(&self) -> Result<i64, AppError> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    async fn create_user(
        &self,
        username: &str,
        email: &str,
        password_hash: &str,
        role: &str,
    ) -> Result<User, AppError> {
        sqlx::query_as::<_, User>(
            "INSERT INTO users (username, email, password_hash, role)
             VALUES (?1, ?2, ?3, ?4)
             RETURNING *",
        )
        .bind(username)
        .bind(email)
        .bind(password_hash)
        .bind(role)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(ref db) if db.message().contains("UNIQUE constraint failed: users.username") => {
                AppError::BadRequest("username already taken".into())
            }
            sqlx::Error::Database(ref db) if db.message().contains("UNIQUE constraint failed: users.email") => {
                AppError::BadRequest("email already registered".into())
            }
            other => AppError::Database(other),
        })
    }

    async fn find_user_by_username(&self, username: &str) -> Result<Option<User>, AppError> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?1")
            .bind(username)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_user_by_id(&self, id: i64) -> Result<Option<User>, AppError> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn create_session(
        &self,
        user_id: i64,
        token_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<Session, AppError> {
        sqlx::query_as::<_, Session>(
            "INSERT INTO sessions (user_id, token_hash, expires_at)
             VALUES (?1, ?2, ?3)
             RETURNING *",
        )
        .bind(user_id)
        .bind(token_hash)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn find_session_by_token_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<Session>, AppError> {
        sqlx::query_as::<_, Session>(
            "SELECT * FROM sessions WHERE token_hash = ?1 AND expires_at > datetime('now')",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)
    }

    async fn delete_session(&self, id: i64) -> Result<(), AppError> {
        sqlx::query("DELETE FROM sessions WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }
}
```

- [ ] **Step 2: Compile check**

```bash
cargo build 2>&1 | tail -10
```

Expected: `Finished` — no errors.

- [ ] **Step 3: Commit**

```bash
git add src/models/ src/dal/ src/services/ Cargo.toml
git commit -m "feat: User/Session models, Store trait user/session methods, Postgres+SQLite impls"
```

---

## Task 5: AuthService

**Files:**
- Create: `src/services/mod.rs`
- Create: `src/services/auth.rs`

- [ ] **Step 1: Write `src/services/mod.rs`**

```rust
pub mod auth;
```

- [ ] **Step 2: Write `src/services/auth.rs`**

```rust
use std::sync::Arc;

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{dal::Store, error::AppError, models::{Session, User}};

/// Session duration: 30 days.
const SESSION_DURATION_DAYS: i64 = 30;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: i64,         // user_id
    pub sid: i64,         // session_id (set after session row created)
    pub exp: i64,         // unix timestamp
}

pub struct AuthService;

impl AuthService {
    pub fn hash_password(password: &str) -> anyhow::Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        Ok(argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow::anyhow!("password hashing failed: {e}"))?
            .to_string())
    }

    pub fn verify_password(password: &str, hash: &str) -> bool {
        let Ok(parsed) = PasswordHash::new(hash) else {
            return false;
        };
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    }

    pub fn create_token(
        user_id: i64,
        session_id: i64,
        jwt_secret: &str,
    ) -> anyhow::Result<String> {
        let exp = (Utc::now() + Duration::days(SESSION_DURATION_DAYS)).timestamp();
        let claims = Claims {
            sub: user_id,
            sid: session_id,
            exp,
        };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(jwt_secret.as_bytes()),
        )
        .map_err(|e| anyhow::anyhow!("JWT encoding failed: {e}"))
    }

    pub fn decode_token(token: &str, jwt_secret: &str) -> Result<Claims, AppError> {
        decode::<Claims>(
            token,
            &DecodingKey::from_secret(jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map(|td| td.claims)
        .map_err(|_| AppError::Unauthorized)
    }

    /// SHA-256 hex digest of a raw token — used as the DB lookup key.
    pub fn hash_token(token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub fn session_expires_at() -> chrono::DateTime<chrono::Utc> {
        Utc::now() + Duration::days(SESSION_DURATION_DAYS)
    }

    /// Full login flow: verify password, create session, return signed JWT.
    pub async fn login(
        db: &Arc<dyn Store>,
        username: &str,
        password: &str,
        jwt_secret: &str,
    ) -> Result<String, AppError> {
        let user = db
            .find_user_by_username(username)
            .await?
            .ok_or(AppError::Unauthorized)?;

        if !Self::verify_password(password, &user.password_hash) {
            return Err(AppError::Unauthorized);
        }

        // Create session row with a placeholder token_hash; we need session_id for the JWT first.
        // Two-step: insert with temp hash, get id, re-sign with session_id, update hash.
        let expires_at = Self::session_expires_at();
        let temp_hash = Self::hash_token(&format!("temp-{}-{}", user.id, Utc::now().timestamp()));
        let session = db.create_session(user.id, &temp_hash, expires_at).await?;

        let token = Self::create_token(user.id, session.id, jwt_secret)
            .map_err(|e| AppError::Internal(e))?;
        let final_hash = Self::hash_token(&token);

        // Update the session row with the real token hash.
        db.update_session_token_hash(session.id, &final_hash).await?;

        Ok(token)
    }
}
```

> **Note:** The two-step token creation requires `update_session_token_hash` on the Store trait. Add this method in the next step.

- [ ] **Step 3: Add `update_session_token_hash` to the Store trait and both backends**

In `src/dal/mod.rs`, add to the trait:

```rust
async fn update_session_token_hash(
    &self,
    session_id: i64,
    token_hash: &str,
) -> Result<(), AppError>;
```

In `src/dal/postgres.rs`, add to `impl Store for PgStore`:

```rust
async fn update_session_token_hash(
    &self,
    session_id: i64,
    token_hash: &str,
) -> Result<(), AppError> {
    sqlx::query("UPDATE sessions SET token_hash = $1 WHERE id = $2")
        .bind(token_hash)
        .bind(session_id)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(AppError::Database)
}
```

In `src/dal/sqlite.rs`, add to `impl Store for SqliteStore`:

```rust
async fn update_session_token_hash(
    &self,
    session_id: i64,
    token_hash: &str,
) -> Result<(), AppError> {
    sqlx::query("UPDATE sessions SET token_hash = ?1 WHERE id = ?2")
        .bind(token_hash)
        .bind(session_id)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(AppError::Database)
}
```

- [ ] **Step 4: Add `sha2` to Cargo.toml**

```toml
sha2 = "0.10"
```

- [ ] **Step 5: Compile check**

```bash
cargo build 2>&1 | tail -5
```

Expected: `Finished`.

- [ ] **Step 6: Commit**

```bash
git add src/services/ src/dal/ Cargo.toml
git commit -m "feat: AuthService (argon2, JWT, session token, login flow)"
```

---

## Task 6: AuthUser middleware extractor

**Files:**
- Create: `src/api/middleware/mod.rs`
- Create: `src/api/middleware/auth.rs`

- [ ] **Step 1: Write `src/api/middleware/mod.rs`**

```rust
pub mod auth;
```

- [ ] **Step 2: Write `src/api/middleware/auth.rs`**

```rust
use std::sync::Arc;

use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, header},
};
use axum_extra::extract::CookieJar;

use crate::{
    dal::Store,
    error::AppError,
    models::User,
    services::auth::AuthService,
    state::AppState,
};

/// Authenticated user extracted from the session cookie.
/// Use as a handler parameter to require authentication.
pub struct AuthUser(pub User);

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Extract token from HttpOnly cookie named "session"
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar
            .get("session")
            .map(|c| c.value().to_string())
            .ok_or(AppError::Unauthorized)?;

        let claims = AuthService::decode_token(&token, &state.config.jwt_secret)?;
        let token_hash = AuthService::hash_token(&token);

        let session = state
            .db
            .find_session_by_token_hash(&token_hash)
            .await?
            .ok_or(AppError::Unauthorized)?;

        // Confirm JWT session_id matches DB row (extra safety check)
        if session.id != claims.sid {
            return Err(AppError::Unauthorized);
        }

        let user = state
            .db
            .find_user_by_id(claims.sub)
            .await?
            .ok_or(AppError::Unauthorized)?;

        Ok(AuthUser(user))
    }
}
```

- [ ] **Step 3: Add `axum-extra` to Cargo.toml**

```toml
axum-extra = { version = "0.9", features = ["cookie"] }
```

---

## Task 7: Auth handlers

**Files:**
- Create: `src/api/mod.rs`
- Create: `src/api/auth.rs`

- [ ] **Step 1: Write `src/api/auth.rs`**

```rust
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::{Deserialize, Serialize};
use time::Duration;

use crate::{
    dal::Store,
    error::AppError,
    models::User,
    services::auth::AuthService,
    state::AppState,
    api::middleware::auth::AuthUser,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/me", get(me))
}

#[derive(Deserialize)]
struct RegisterRequest {
    username: String,
    email: String,
    password: String,
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct UserResponse {
    id: i64,
    username: String,
    email: String,
    role: String,
    display_name: Option<String>,
}

impl From<User> for UserResponse {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            username: u.username,
            email: u.email,
            role: u.role,
            display_name: u.display_name,
        }
    }
}

async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<impl IntoResponse, AppError> {
    // First user becomes admin; all subsequent users are role "user"
    let role = if state.db.count_users().await? == 0 {
        "admin"
    } else {
        "user"
    };

    if body.password.len() < 8 {
        return Err(AppError::BadRequest("password must be at least 8 characters".into()));
    }

    let hash = AuthService::hash_password(&body.password)
        .map_err(|e| AppError::Internal(e))?;

    let user = state
        .db
        .create_user(&body.username, &body.email, &hash, role)
        .await?;

    Ok((StatusCode::CREATED, Json(UserResponse::from(user))))
}

async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let token = AuthService::login(
        &state.db,
        &body.username,
        &body.password,
        &state.config.jwt_secret,
    )
    .await?;

    let cookie = Cookie::build(("session", token))
        .http_only(true)
        .same_site(SameSite::Strict)
        .max_age(Duration::days(30))
        .path("/")
        .build();

    Ok((jar.add(cookie), StatusCode::NO_CONTENT))
}

async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
    auth: AuthUser,
) -> Result<impl IntoResponse, AppError> {
    // Find and delete the session
    let token = jar.get("session").map(|c| c.value().to_string()).unwrap_or_default();
    let token_hash = AuthService::hash_token(&token);
    if let Some(session) = state.db.find_session_by_token_hash(&token_hash).await? {
        state.db.delete_session(session.id).await?;
    }

    let removed = jar.remove(Cookie::from("session"));
    Ok((removed, StatusCode::NO_CONTENT))
}

async fn me(auth: AuthUser) -> Json<UserResponse> {
    Json(UserResponse::from(auth.0))
}
```

- [ ] **Step 2: Write `src/api/mod.rs`**

```rust
pub mod auth;
pub mod middleware;

use axum::Router;
use crate::state::AppState;

pub fn api_router(state: AppState) -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::router())
}
```

- [ ] **Step 3: Add `time` crate to Cargo.toml (required by axum-extra cookies)**

```toml
time = "0.3"
```

---

## Task 8: Wire API router into app.rs and lib.rs

**Files:**
- Modify: `src/app.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Update `src/app.rs` to mount the API router**

```rust
use axum::{extract::State, routing::get, Json, Router};
use serde_json::{json, Value};

use crate::{api::api_router, error::AppError, state::AppState};

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .nest("/api/v1", api_router(state.clone()))
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    state.db.health_check().await?;
    Ok(Json(json!({ "status": "ok" })))
}
```

- [ ] **Step 2: Update `src/lib.rs`**

```rust
pub mod api;
pub mod config;
pub mod dal;
pub mod error;
pub mod models;
pub mod services;
pub mod state;

mod app;
pub use app::build_router;
```

- [ ] **Step 3: Compile check**

```bash
cargo build 2>&1 | tail -10
```

Expected: `Finished`.

- [ ] **Step 4: Commit**

```bash
git add src/
git commit -m "feat: auth handlers (register, login, logout, me) + AuthUser extractor"
```

---

## Task 9: Integration tests

**Files:**
- Create: `tests/auth.rs`

- [ ] **Step 1: Write `tests/auth.rs`**

```rust
use std::sync::Arc;
use suzuran_server::{
    build_router,
    config::Config,
    dal::sqlite::SqliteStore,
    state::AppState,
};

async fn test_app() -> (axum::Router, String) {
    let store = SqliteStore::new("sqlite::memory:")
        .await
        .expect("SQLite failed");
    store.migrate().await.expect("migrations failed");

    let config = Config {
        database_url: "sqlite::memory:".into(),
        jwt_secret: "test-secret-32-chars-minimum-xxxx".into(),
        port: 0,
        log_level: "error".into(),
    };
    let base_url = format!("http://127.0.0.1"); // filled in per-test
    let state = AppState::new(Arc::new(store), config);
    (build_router(state), base_url)
}

async fn spawn_test_server() -> String {
    let (app, _) = test_app().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn register_first_user_becomes_admin() {
    let base = spawn_test_server().await;
    let client = reqwest::Client::new();

    let res = client
        .post(format!("{base}/api/v1/auth/register"))
        .json(&serde_json::json!({
            "username": "alice",
            "email": "alice@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 201);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["role"], "admin");
    assert_eq!(body["username"], "alice");
    assert!(body.get("password_hash").is_none(), "password_hash must not be serialized");
}

#[tokio::test]
async fn login_sets_session_cookie() {
    let base = spawn_test_server().await;
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();

    // Register first
    client
        .post(format!("{base}/api/v1/auth/register"))
        .json(&serde_json::json!({
            "username": "bob",
            "email": "bob@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();

    // Login
    let res = client
        .post(format!("{base}/api/v1/auth/login"))
        .json(&serde_json::json!({
            "username": "bob",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 204);
    assert!(
        res.headers().get("set-cookie").is_some(),
        "set-cookie header must be present"
    );
}

#[tokio::test]
async fn me_requires_authentication() {
    let base = spawn_test_server().await;
    let client = reqwest::Client::new();

    let res = client
        .get(format!("{base}/api/v1/auth/me"))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn me_returns_user_after_login() {
    let base = spawn_test_server().await;
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();

    client
        .post(format!("{base}/api/v1/auth/register"))
        .json(&serde_json::json!({
            "username": "carol",
            "email": "carol@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{base}/api/v1/auth/login"))
        .json(&serde_json::json!({
            "username": "carol",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();

    let res = client
        .get(format!("{base}/api/v1/auth/me"))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["username"], "carol");
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test --test auth -- --nocapture
```

Expected: all 4 tests pass.

- [ ] **Step 3: Commit**

```bash
git add tests/auth.rs
git commit -m "test: auth integration tests (register, login, me, 401 guard)"
```

---

## Task 10: Full Docker smoke test

- [ ] **Step 1: Build and start**

```bash
docker compose up --build -d
sleep 5
```

- [ ] **Step 2: Register a user**

```bash
curl -sf -X POST http://localhost:3000/api/v1/auth/register \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","email":"admin@example.com","password":"password123"}' \
  | python3 -m json.tool
```

Expected: `{"id": 1, "username": "admin", "role": "admin", ...}`

- [ ] **Step 3: Login and capture cookie**

```bash
curl -sf -c /tmp/cookies.txt -X POST http://localhost:3000/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"password123"}'
echo "Status: $?"
```

Expected: exit 0 (204 response).

- [ ] **Step 4: Call /me with cookie**

```bash
curl -sf -b /tmp/cookies.txt http://localhost:3000/api/v1/auth/me \
  | python3 -m json.tool
```

Expected: `{"username": "admin", "role": "admin", ...}`

- [ ] **Step 5: Tear down and commit filemap**

```bash
docker compose down -v
git add tasks/codebase-filemap.md
git commit -m "docs: update filemap for Phase 1.4 auth"
```
