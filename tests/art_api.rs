use std::sync::Arc;
use url::Url;
use webauthn_rs::WebauthnBuilder;

use suzuran_server::{
    build_router,
    config::Config,
    dal::{sqlite::SqliteStore, Store, UpsertTrack},
    services::freedb::FreedBService,
    services::musicbrainz::MusicBrainzService,
    state::AppState,
};

fn test_webauthn() -> webauthn_rs::Webauthn {
    let origin = Url::parse("http://localhost:3000").unwrap();
    WebauthnBuilder::new("localhost", &origin)
        .unwrap()
        .rp_name("suzuran-test")
        .build()
        .unwrap()
}

async fn spawn_test_server() -> (String, Arc<dyn Store>) {
    let store = SqliteStore::new("sqlite::memory:")
        .await
        .expect("SQLite failed");
    store.migrate().await.expect("migrations failed");
    let store: Arc<dyn Store> = Arc::new(store);

    let config = Config {
        database_url: "sqlite::memory:".into(),
        jwt_secret: "test-secret-32-chars-minimum-xxxx".into(),
        port: 0,
        log_level: "error".into(),
        rp_id: "localhost".into(),
        rp_origin: "http://localhost:3000".into(),
        uploads_dir: std::path::PathBuf::from("/tmp/suzuran-test-uploads"),
    };
    let mb_service = Arc::new(MusicBrainzService::new());
    let freedb_service = Arc::new(FreedBService::new());
    let state = AppState::new(Arc::clone(&store), config, test_webauthn(), mb_service, freedb_service);
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), store)
}

async fn login_admin(base: &str) -> reqwest::Client {
    let client = reqwest::Client::builder().cookie_store(true).build().unwrap();
    client
        .post(format!("{base}/api/v1/auth/register"))
        .json(&serde_json::json!({
            "username": "admin",
            "email": "admin@test.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();
    client
        .post(format!("{base}/api/v1/auth/login"))
        .json(&serde_json::json!({
            "username": "admin",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();
    client
}

// ── auth guard tests ──────────────────────────────────────────────────────────

#[tokio::test]
async fn art_embed_requires_auth() {
    let (base, _store) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base}/api/v1/tracks/1/art/embed"))
        .json(&serde_json::json!({ "source_url": "http://example.com/cover.jpg" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 401);
}

#[tokio::test]
async fn art_extract_requires_auth() {
    let (base, _store) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base}/api/v1/tracks/1/art/extract"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 401);
}

#[tokio::test]
async fn art_standardize_requires_auth() {
    let (base, _store) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base}/api/v1/tracks/1/art/standardize"))
        .json(&serde_json::json!({ "art_profile_id": 1 }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 401);
}

#[tokio::test]
async fn art_standardize_library_requires_auth() {
    let (base, _store) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base}/api/v1/libraries/1/art/standardize"))
        .json(&serde_json::json!({ "art_profile_id": 1 }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 401);
}

// ── 404 guard tests ───────────────────────────────────────────────────────────

#[tokio::test]
async fn art_embed_track_not_found() {
    let (base, _store) = spawn_test_server().await;
    let client = login_admin(&base).await;
    let resp = client
        .post(format!("{base}/api/v1/tracks/9999/art/embed"))
        .json(&serde_json::json!({ "source_url": "http://example.com/cover.jpg" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 404);
}

#[tokio::test]
async fn art_extract_track_not_found() {
    let (base, _store) = spawn_test_server().await;
    let client = login_admin(&base).await;
    let resp = client
        .post(format!("{base}/api/v1/tracks/9999/art/extract"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 404);
}

// ── job enqueue tests ─────────────────────────────────────────────────────────

#[tokio::test]
async fn art_embed_enqueues_job() {
    let (base, store) = spawn_test_server().await;
    let client = login_admin(&base).await;

    let lib = store
        .create_library("Test", "/music", "flac")
        .await
        .unwrap();
    let track = store
        .upsert_track(UpsertTrack {
            library_id: lib.id,
            relative_path: "song.flac".into(),
            file_hash: "hash_embed".into(),
            tags: serde_json::json!({}),
            ..UpsertTrack::default()
        })
        .await
        .unwrap();

    let resp = client
        .post(format!("{base}/api/v1/tracks/{}/art/embed", track.id))
        .json(&serde_json::json!({ "source_url": "http://example.com/cover.jpg" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 202);

    let jobs = store.list_jobs(Some("pending"), 10).await.unwrap();
    let art_jobs: Vec<_> = jobs.iter().filter(|j| j.job_type == "art_process").collect();
    assert_eq!(art_jobs.len(), 1);
    assert_eq!(art_jobs[0].payload["action"], "embed");
    assert_eq!(art_jobs[0].payload["source_url"], "http://example.com/cover.jpg");
    assert_eq!(art_jobs[0].payload["track_id"].as_i64().unwrap(), track.id);
}

#[tokio::test]
async fn art_extract_enqueues_job() {
    let (base, store) = spawn_test_server().await;
    let client = login_admin(&base).await;

    let lib = store
        .create_library("Test2", "/music2", "flac")
        .await
        .unwrap();
    let track = store
        .upsert_track(UpsertTrack {
            library_id: lib.id,
            relative_path: "song2.flac".into(),
            file_hash: "hash_extract".into(),
            has_embedded_art: true,
            tags: serde_json::json!({}),
            ..UpsertTrack::default()
        })
        .await
        .unwrap();

    let resp = client
        .post(format!("{base}/api/v1/tracks/{}/art/extract", track.id))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 202);

    let jobs = store.list_jobs(Some("pending"), 10).await.unwrap();
    let art_jobs: Vec<_> = jobs.iter().filter(|j| j.job_type == "art_process").collect();
    assert_eq!(art_jobs.len(), 1);
    assert_eq!(art_jobs[0].payload["action"], "extract");
    assert_eq!(art_jobs[0].payload["track_id"].as_i64().unwrap(), track.id);
}

#[tokio::test]
async fn art_standardize_library_enqueues_for_tracks_with_art() {
    let (base, store) = spawn_test_server().await;
    let client = login_admin(&base).await;

    let lib = store
        .create_library("Test3", "/music3", "flac")
        .await
        .unwrap();

    // Track with art
    store
        .upsert_track(UpsertTrack {
            library_id: lib.id,
            relative_path: "with_art.flac".into(),
            file_hash: "hash_with_art".into(),
            has_embedded_art: true,
            tags: serde_json::json!({}),
            ..UpsertTrack::default()
        })
        .await
        .unwrap();

    // Track without art — should be skipped
    store
        .upsert_track(UpsertTrack {
            library_id: lib.id,
            relative_path: "no_art.flac".into(),
            file_hash: "hash_no_art".into(),
            has_embedded_art: false,
            tags: serde_json::json!({}),
            ..UpsertTrack::default()
        })
        .await
        .unwrap();

    let resp = client
        .post(format!("{base}/api/v1/libraries/{}/art/standardize", lib.id))
        .json(&serde_json::json!({ "art_profile_id": 1 }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 202);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["enqueued"].as_i64().unwrap(), 1);

    let jobs = store.list_jobs(Some("pending"), 10).await.unwrap();
    let art_jobs: Vec<_> = jobs.iter().filter(|j| j.job_type == "art_process").collect();
    assert_eq!(art_jobs.len(), 1);
    assert_eq!(art_jobs[0].payload["action"], "standardize");
    assert_eq!(art_jobs[0].payload["art_profile_id"].as_i64().unwrap(), 1);
}
