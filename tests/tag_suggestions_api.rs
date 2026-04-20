use std::sync::Arc;
use url::Url;
use webauthn_rs::WebauthnBuilder;

use suzuran_server::{
    build_router,
    config::Config,
    dal::{sqlite::SqliteStore, Store, UpsertTrack, UpsertTagSuggestion},
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

/// Create a library + track in the store, return the track id.
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
            tags: serde_json::json!({}),
            duration_secs: Some(180.0),
            bitrate: None,
            sample_rate: None,
            channels: None,
            has_embedded_art: false,
        })
        .await
        .unwrap();
    track.id
}

/// Create a tag suggestion for the given track_id.
async fn seed_suggestion(store: &Arc<dyn Store>, track_id: i64, confidence: f32) -> i64 {
    let s = store
        .create_tag_suggestion(UpsertTagSuggestion {
            track_id,
            source: "acoustid".into(),
            suggested_tags: serde_json::json!({"title": "Suggested Title"}),
            confidence,
            mb_recording_id: None,
            mb_release_id: None,
            cover_art_url: None,
        })
        .await
        .unwrap();
    s.id
}

// ── auth guard tests ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_list_requires_auth() {
    let (base, _store) = spawn_test_server_with_store().await;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{base}/api/v1/tag-suggestions"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_get_one_requires_auth() {
    let (base, _store) = spawn_test_server_with_store().await;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{base}/api/v1/tag-suggestions/1"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

// ── count is public ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_count_is_public() {
    let (base, _store) = spawn_test_server_with_store().await;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{base}/api/v1/tag-suggestions/count"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["count"], 0);
}

#[tokio::test]
async fn test_count_reflects_pending() {
    let (base, store) = spawn_test_server_with_store().await;
    let track_id = seed_track(&store).await;
    seed_suggestion(&store, track_id, 0.9).await;
    seed_suggestion(&store, track_id, 0.7).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{base}/api/v1/tag-suggestions/count"))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["count"], 2);
}

// ── 404 on missing id ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_unknown_returns_404() {
    let (base, store) = spawn_test_server_with_store().await;
    let client = login_admin(&base).await;
    // Need at least one user registered for auth to work
    let _ = store; // keep alive

    let resp = client
        .get(format!("{base}/api/v1/tag-suggestions/99999"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

// ── list ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_list_returns_pending() {
    let (base, store) = spawn_test_server_with_store().await;
    let client = login_admin(&base).await;

    let track_id = seed_track(&store).await;
    seed_suggestion(&store, track_id, 0.9).await;
    seed_suggestion(&store, track_id, 0.7).await;

    let body: Vec<serde_json::Value> = client
        .get(format!("{base}/api/v1/tag-suggestions"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body.len(), 2);
}

#[tokio::test]
async fn test_list_filtered_by_track_id() {
    let (base, store) = spawn_test_server_with_store().await;
    let client = login_admin(&base).await;

    let track_id = seed_track(&store).await;
    let lib2 = store.create_library("L2", "/music2", "flac", None).await.unwrap();
    let track2 = store
        .upsert_track(UpsertTrack {
            library_id: lib2.id,
            relative_path: "other.flac".into(),
            file_hash: "other_hash".into(),
            title: None,
            artist: None,
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
            tags: serde_json::json!({}),
            duration_secs: None,
            bitrate: None,
            sample_rate: None,
            channels: None,
            has_embedded_art: false,
        })
        .await
        .unwrap();

    seed_suggestion(&store, track_id, 0.9).await;
    seed_suggestion(&store, track2.id, 0.8).await;

    let body: Vec<serde_json::Value> = client
        .get(format!("{base}/api/v1/tag-suggestions?track_id={track_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    // Only the suggestion for track_id, not track2
    assert_eq!(body.len(), 1);
    assert_eq!(body[0]["track_id"], track_id);
}

// ── reject ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_reject_sets_status() {
    let (base, store) = spawn_test_server_with_store().await;
    let client = login_admin(&base).await;

    let track_id = seed_track(&store).await;
    let suggestion_id = seed_suggestion(&store, track_id, 0.9).await;

    let resp = client
        .post(format!("{base}/api/v1/tag-suggestions/{suggestion_id}/reject"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // No longer pending
    let pending = store.list_pending_tag_suggestions(Some(track_id)).await.unwrap();
    assert_eq!(pending.len(), 0);
}

#[tokio::test]
async fn test_reject_unknown_returns_404() {
    let (base, _store) = spawn_test_server_with_store().await;
    let client = login_admin(&base).await;

    let resp = client
        .post(format!("{base}/api/v1/tag-suggestions/99999/reject"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

// ── accept ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_accept_sets_status() {
    let (base, store) = spawn_test_server_with_store().await;
    let client = login_admin(&base).await;

    let track_id = seed_track(&store).await;
    let suggestion_id = seed_suggestion(&store, track_id, 0.9).await;

    let resp = client
        .post(format!("{base}/api/v1/tag-suggestions/{suggestion_id}/accept"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // No longer pending
    let pending = store.list_pending_tag_suggestions(Some(track_id)).await.unwrap();
    assert_eq!(pending.len(), 0);
}

#[tokio::test]
async fn test_accept_unknown_returns_404() {
    let (base, _store) = spawn_test_server_with_store().await;
    let client = login_admin(&base).await;

    let resp = client
        .post(format!("{base}/api/v1/tag-suggestions/99999/accept"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

// ── batch-accept ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_batch_accept_above_threshold() {
    let (base, store) = spawn_test_server_with_store().await;
    let client = login_admin(&base).await;

    let track_id = seed_track(&store).await;
    // Three suggestions at different confidence levels
    seed_suggestion(&store, track_id, 0.9).await;
    seed_suggestion(&store, track_id, 0.7).await;
    seed_suggestion(&store, track_id, 0.5).await;

    let resp = client
        .post(format!("{base}/api/v1/tag-suggestions/batch-accept"))
        .json(&serde_json::json!({"min_confidence": 0.8}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["accepted"], 1);

    // Two remain pending
    let pending = store.list_pending_tag_suggestions(Some(track_id)).await.unwrap();
    assert_eq!(pending.len(), 2);
}

#[tokio::test]
async fn test_batch_accept_all_above_threshold() {
    let (base, store) = spawn_test_server_with_store().await;
    let client = login_admin(&base).await;

    let track_id = seed_track(&store).await;
    seed_suggestion(&store, track_id, 0.9).await;
    seed_suggestion(&store, track_id, 0.85).await;

    let resp = client
        .post(format!("{base}/api/v1/tag-suggestions/batch-accept"))
        .json(&serde_json::json!({"min_confidence": 0.8}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["accepted"], 2);

    let pending = store.list_pending_tag_suggestions(Some(track_id)).await.unwrap();
    assert_eq!(pending.len(), 0);
}

#[tokio::test]
async fn test_batch_accept_requires_auth() {
    let (base, _store) = spawn_test_server_with_store().await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base}/api/v1/tag-suggestions/batch-accept"))
        .json(&serde_json::json!({"min_confidence": 0.8}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

// ── get one ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_one_returns_suggestion() {
    let (base, store) = spawn_test_server_with_store().await;
    let client = login_admin(&base).await;

    let track_id = seed_track(&store).await;
    let suggestion_id = seed_suggestion(&store, track_id, 0.9).await;

    let body: serde_json::Value = client
        .get(format!("{base}/api/v1/tag-suggestions/{suggestion_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["id"], suggestion_id);
    assert_eq!(body["track_id"], track_id);
    assert_eq!(body["status"], "pending");
}
