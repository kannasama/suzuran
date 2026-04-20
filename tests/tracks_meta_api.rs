mod common;

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

async fn spawn_test_server_with_store() -> (String, Arc<dyn Store>) {
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
    };
    let mb_service = Arc::new(MusicBrainzService::new(String::new()));
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

/// Register and login the first user (who becomes admin). Returns an authenticated client.
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

/// Create a library and track in the store. Returns `track_id`.
async fn seed_track(store: &Arc<dyn Store>) -> i64 {
    let lib = store
        .create_library("Test", "/music", "flac", None)
        .await
        .unwrap();
    let track = store
        .upsert_track(UpsertTrack {
            library_id: lib.id,
            relative_path: "test.flac".into(),
            file_hash: "abc123".into(),
            title: Some("Test Song".into()),
            artist: Some("Test Artist".into()),
            albumartist: None,
            album: None,
            tracknumber: None,
            discnumber: None,
            totaldiscs: None,
            totaltracks: None,
            date: None,
            genre: None,
            composer: None,
            label: None,
            catalognumber: None,
            tags: serde_json::json!({"title": "Test Song", "artist": "Test Artist"}),
            duration_secs: Some(180.0),
            bitrate: None,
            sample_rate: None,
            channels: None,
            bit_depth: None,
            has_embedded_art: false,
        })
        .await
        .unwrap();
    track.id
}

// ── auth guard ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_track_requires_auth() {
    let (base, _store) = spawn_test_server_with_store().await;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{base}/api/v1/tracks/1"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

// ── known track ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_known_track_returns_200_with_correct_id() {
    let (base, store) = spawn_test_server_with_store().await;
    let client = login_admin(&base).await;

    let track_id = seed_track(&store).await;

    let resp = client
        .get(format!("{base}/api/v1/tracks/{track_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["id"], track_id);
    assert_eq!(body["title"], "Test Song");
    assert_eq!(body["artist"], "Test Artist");
}

// ── unknown track ────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_unknown_track_returns_404() {
    let (base, _store) = spawn_test_server_with_store().await;
    let client = login_admin(&base).await;

    let resp = client
        .get(format!("{base}/api/v1/tracks/99999"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}
