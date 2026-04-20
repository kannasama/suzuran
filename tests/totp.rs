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
    };

    let mb_service = Arc::new(MusicBrainzService::new(String::new()));
    let freedb_service = Arc::new(FreedBService::new());
    let state = AppState::new(Arc::new(store), config, webauthn, mb_service, freedb_service);
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}")
}

async fn register_and_login(base: &str) -> reqwest::Client {
    let client = reqwest::Client::builder().cookie_store(true).build().unwrap();

    client
        .post(format!("{base}/api/v1/auth/register"))
        .json(&serde_json::json!({
            "username": "alice",
            "email": "alice@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{base}/api/v1/auth/login"))
        .json(&serde_json::json!({"username": "alice", "password": "password123"}))
        .send()
        .await
        .unwrap();

    client
}

#[tokio::test]
async fn totp_enroll_returns_otpauth_uri() {
    let base = spawn_test_server().await;
    let client = register_and_login(&base).await;

    let res = client
        .post(format!("{base}/api/v1/totp/enroll"))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(
        body["otpauth_uri"].as_str().unwrap().starts_with("otpauth://totp/"),
        "expected otpauth://totp/ prefix, got: {}",
        body["otpauth_uri"]
    );
}

#[tokio::test]
async fn totp_enroll_then_disenroll() {
    let base = spawn_test_server().await;
    let client = register_and_login(&base).await;

    client
        .post(format!("{base}/api/v1/totp/enroll"))
        .send()
        .await
        .unwrap();

    let res = client
        .delete(format!("{base}/api/v1/totp/disenroll"))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 204);
}
