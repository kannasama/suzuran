# Phase 1.9 — Streaming Endpoint Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement `GET /api/v1/tracks/:id/stream` with full HTTP range request support, `Accept-Ranges`, `Content-Length`, and correct `Content-Type` per format. Also `HEAD` support so clients can query duration and file size without downloading.

**Architecture:** The handler resolves the track's absolute path from library root + relative_path, opens the file, reads the `Range` header if present, and streams the byte range. Uses `tokio::fs::File` + `axum::body::Body::from_stream`. No web player in v1.0 — this is the passive groundwork for v1.1 streaming.

**Tech Stack:** tokio::fs, axum Body streaming, tower-http (already present), mime_guess for Content-Type.

---

## File Map

| File | Action | Purpose |
|------|--------|---------|
| `Cargo.toml` | Modify | Add `mime_guess` |
| `src/api/tracks.rs` | Create | Stream handler + HEAD handler + route |
| `src/api/mod.rs` | Modify | Mount tracks routes |
| `tests/streaming.rs` | Create | Range request and HEAD tests |

---

## Task 1: Dependencies

- [ ] **Step 1: Add `mime_guess` to `Cargo.toml`**

```toml
mime_guess = "2"
```

---

## Task 2: Stream handler

**Files:**
- Create: `src/api/tracks.rs`

- [ ] **Step 1: Write `src/api/tracks.rs`**

```rust
use std::path::PathBuf;

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::Response,
    routing::get,
    Router,
};
use tokio::io::AsyncSeekExt;

use crate::{api::middleware::auth::AuthUser, error::AppError, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/:id/stream", get(stream).head(stream_head))
}

/// Resolve a track's absolute path from its library root + relative_path.
async fn resolve_track_path(
    state: &AppState,
    track_id: i64,
) -> Result<(PathBuf, crate::models::Track), AppError> {
    let track = state.db.get_track(track_id).await?
        .ok_or_else(|| AppError::NotFound(format!("track {track_id} not found")))?;

    let library = state.db.get_library(track.library_id).await?
        .ok_or_else(|| AppError::NotFound(format!("library {} not found", track.library_id)))?;

    let abs_path = PathBuf::from(&library.root_path).join(&track.relative_path);
    Ok((abs_path, track))
}

fn content_type_for(path: &std::path::Path) -> String {
    mime_guess::from_path(path)
        .first_or_octet_stream()
        .to_string()
}

/// Parse a Range header value like "bytes=0-1023" or "bytes=512-".
fn parse_range(range_header: &str, file_size: u64) -> Option<(u64, u64)> {
    let s = range_header.strip_prefix("bytes=")?;
    let (start_str, end_str) = s.split_once('-')?;
    let start: u64 = start_str.parse().ok()?;
    let end: u64 = if end_str.is_empty() {
        file_size.saturating_sub(1)
    } else {
        end_str.parse().ok()?
    };
    if start > end || end >= file_size {
        return None;
    }
    Some((start, end))
}

/// GET /api/v1/tracks/:id/stream
/// Supports: full file, byte-range requests (Range header), correct Content-Type.
pub async fn stream(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(track_id): Path<i64>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let (abs_path, _track) = resolve_track_path(&state, track_id).await?;

    let metadata = tokio::fs::metadata(&abs_path).await.map_err(|e| {
        AppError::Internal(anyhow::anyhow!("file metadata error for {:?}: {e}", abs_path))
    })?;
    let file_size = metadata.len();
    let content_type = content_type_for(&abs_path);

    // Parse Range header
    let range = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| parse_range(s, file_size));

    let (start, end, status) = match range {
        Some((s, e)) => (s, e, StatusCode::PARTIAL_CONTENT),
        None => (0, file_size.saturating_sub(1), StatusCode::OK),
    };

    let length = end - start + 1;

    let mut file = tokio::fs::File::open(&abs_path).await.map_err(|e| {
        AppError::Internal(anyhow::anyhow!("file open error: {e}"))
    })?;

    if start > 0 {
        file.seek(std::io::SeekFrom::Start(start)).await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!("seek error: {e}"))
        })?;
    }

    // Stream only `length` bytes
    let limited = tokio::io::AsyncReadExt::take(file, length);
    let stream = tokio_util::io::ReaderStream::new(limited);
    let body = Body::from_stream(stream);

    let mut builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, length.to_string())
        .header(header::ACCEPT_RANGES, "bytes");

    if status == StatusCode::PARTIAL_CONTENT {
        builder = builder.header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{file_size}"),
        );
    }

    builder.body(body).map_err(|e| AppError::Internal(anyhow::anyhow!("response build error: {e}")))
}

/// HEAD /api/v1/tracks/:id/stream
/// Returns headers without body — clients use this to get Content-Length, Content-Type, duration.
pub async fn stream_head(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(track_id): Path<i64>,
) -> Result<Response, AppError> {
    let (abs_path, track) = resolve_track_path(&state, track_id).await?;

    let metadata = tokio::fs::metadata(&abs_path).await.map_err(|e| {
        AppError::Internal(anyhow::anyhow!("file metadata error: {e}"))
    })?;

    let content_type = content_type_for(&abs_path);

    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, metadata.len().to_string())
        .header(header::ACCEPT_RANGES, "bytes");

    if let Some(dur) = track.duration_secs {
        builder = builder.header("X-Duration-Secs", dur.to_string());
    }
    if let Some(br) = track.bitrate {
        builder = builder.header("X-Bitrate", br.to_string());
    }
    if let Some(sr) = track.sample_rate {
        builder = builder.header("X-Sample-Rate", sr.to_string());
    }

    builder
        .body(Body::empty())
        .map_err(|e| AppError::Internal(anyhow::anyhow!("response build error: {e}")))
}
```

- [ ] **Step 2: Add `tokio-util` to `Cargo.toml`**

```toml
tokio-util = { version = "0.7", features = ["io"] }
```

- [ ] **Step 3: Update `src/api/mod.rs`**

```rust
pub mod auth;
pub mod jobs;
pub mod libraries;
pub mod middleware;
pub mod settings;
pub mod themes;
pub mod totp;
pub mod tracks;
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
        .nest("/tracks", tracks::router())
}
```

- [ ] **Step 4: Add `pub mod api;` already exists in `src/lib.rs` — no change needed.**

- [ ] **Step 5: Compile check**

```bash
cargo build 2>&1 | tail -5
```

Expected: `Finished`.

- [ ] **Step 6: Commit**

```bash
git add src/ Cargo.toml
git commit -m "feat: streaming endpoint GET+HEAD /api/v1/tracks/:id/stream with range request support"
```

---

## Task 3: Integration tests

**Files:**
- Create: `tests/streaming.rs`

- [ ] **Step 1: Write `tests/streaming.rs`**

```rust
use std::{sync::Arc, path::PathBuf};
use url::Url;
use webauthn_rs::WebauthnBuilder;
use suzuran_server::{
    build_router, config::Config,
    dal::{sqlite::SqliteStore, Store, UpsertTrack},
    state::AppState,
};

async fn spawn_test_server() -> (String, reqwest::Client) {
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

    let state = AppState::new(store.clone() as Arc<dyn Store>, config, webauthn);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, build_router(state)).await.unwrap() });

    let base = format!("http://{addr}");
    let client = reqwest::Client::builder().cookie_store(true).build().unwrap();

    // Register + login
    client.post(format!("{base}/api/v1/auth/register"))
        .json(&serde_json::json!({"username":"admin","email":"a@a.com","password":"password123"}))
        .send().await.unwrap();
    client.post(format!("{base}/api/v1/auth/login"))
        .json(&serde_json::json!({"username":"admin","password":"password123"}))
        .send().await.unwrap();

    // Seed a real file we can stream
    let dir = tempfile::TempDir::new().unwrap();
    let file_content = b"FAKE_AUDIO_CONTENT_FOR_TESTING_1234567890";
    let file_path = dir.path().join("test.mp3");
    tokio::fs::write(&file_path, file_content).await.unwrap();

    let lib = store.create_library("Test", dir.path().to_str().unwrap(), "mp3", None).await.unwrap();

    store.upsert_track(UpsertTrack {
        library_id: lib.id,
        relative_path: "test.mp3".into(),
        file_hash: "abc123".into(),
        title: Some("Test Track".into()),
        artist: None, albumartist: None, album: None,
        tracknumber: None, discnumber: None, totaldiscs: None, totaltracks: None,
        date: None, genre: None, composer: None, label: None, catalognumber: None,
        tags: serde_json::json!({}),
        duration_secs: Some(3.0),
        bitrate: Some(320),
        sample_rate: Some(44100),
        channels: Some(2),
        has_embedded_art: false,
    }).await.unwrap();

    // Keep dir alive via leak (test-only)
    std::mem::forget(dir);

    (base, client)
}

#[tokio::test]
async fn stream_full_file() {
    let (base, client) = spawn_test_server().await;

    let res = client.get(format!("{base}/api/v1/tracks/1/stream"))
        .send().await.unwrap();

    assert_eq!(res.status(), 200);
    assert!(res.headers().get("accept-ranges").is_some());
    assert!(res.headers().get("content-length").is_some());

    let body = res.bytes().await.unwrap();
    assert_eq!(&body[..], b"FAKE_AUDIO_CONTENT_FOR_TESTING_1234567890");
}

#[tokio::test]
async fn stream_range_request() {
    let (base, client) = spawn_test_server().await;

    let res = client.get(format!("{base}/api/v1/tracks/1/stream"))
        .header("Range", "bytes=0-3")
        .send().await.unwrap();

    assert_eq!(res.status(), 206);
    assert!(res.headers().get("content-range").is_some());

    let body = res.bytes().await.unwrap();
    assert_eq!(&body[..], b"FAKE");
}

#[tokio::test]
async fn stream_head_returns_metadata() {
    let (base, client) = spawn_test_server().await;

    let res = client
        .request(reqwest::Method::HEAD, format!("{base}/api/v1/tracks/1/stream"))
        .send().await.unwrap();

    assert_eq!(res.status(), 200);
    assert!(res.headers().get("content-length").is_some());
    assert!(res.headers().get("accept-ranges").is_some());
    assert_eq!(res.headers().get("x-duration-secs").unwrap(), "3");
    assert_eq!(res.content_length().unwrap(), 40); // len of FAKE_AUDIO_CONTENT...
}

#[tokio::test]
async fn stream_requires_auth() {
    let (base, _) = spawn_test_server().await;
    let anon = reqwest::Client::new();
    let res = anon.get(format!("{base}/api/v1/tracks/1/stream")).send().await.unwrap();
    assert_eq!(res.status(), 401);
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test --test streaming -- --nocapture
```

Expected: all 4 tests pass.

- [ ] **Step 3: Commit**

```bash
git add tests/streaming.rs tasks/codebase-filemap.md
git commit -m "test: streaming endpoint — full file, range request, HEAD, auth guard"
```
