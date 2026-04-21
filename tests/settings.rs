use std::sync::Arc;
use url::Url;
use webauthn_rs::WebauthnBuilder;

use suzuran_server::{build_router, config::Config, dal::sqlite::SqliteStore, services::{freedb::FreedBService, musicbrainz::MusicBrainzService}, state::AppState};

async fn spawn_test_server() -> String {
    let store = SqliteStore::new("sqlite::memory:").await.unwrap();
    store.migrate().await.unwrap();

    let origin = Url::parse("http://localhost:3000").unwrap();
    let webauthn = WebauthnBuilder::new("localhost", &origin)
        .unwrap()
        .rp_name("test")
        .build()
        .unwrap();

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
    let state = AppState::new(Arc::new(store), config, webauthn, mb_service, freedb_service);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, build_router(state)).await.unwrap() });
    format!("http://{addr}")
}

async fn admin_client(base: &str) -> reqwest::Client {
    let client = reqwest::Client::builder().cookie_store(true).build().unwrap();
    client
        .post(format!("{base}/api/v1/auth/register"))
        .json(&serde_json::json!({"username": "admin", "email": "a@a.com", "password": "password123"}))
        .send()
        .await
        .unwrap();
    client
        .post(format!("{base}/api/v1/auth/login"))
        .json(&serde_json::json!({"username": "admin", "password": "password123"}))
        .send()
        .await
        .unwrap();
    client
}

#[tokio::test]
async fn settings_list_requires_auth() {
    let base = spawn_test_server().await;
    let res = reqwest::get(format!("{base}/api/v1/settings")).await.unwrap();
    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn settings_list_returns_defaults() {
    let base = spawn_test_server().await;
    let client = admin_client(&base).await;
    let res = client.get(format!("{base}/api/v1/settings")).send().await.unwrap();
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
        .send()
        .await
        .unwrap();
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
        .post(format!("{base}/api/v1/themes"))
        .json(&serde_json::json!({
            "name": "Midnight",
            "css_vars": {"--bg": "#0a0a0f"},
            "accent_color": "#4f8ef7"
        }))
        .send()
        .await
        .unwrap();
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
