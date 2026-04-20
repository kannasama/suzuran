use std::sync::Arc;
use tempfile::TempDir;
use url::Url;
use webauthn_rs::WebauthnBuilder;

use suzuran_server::{
    build_router,
    config::Config,
    dal::sqlite::SqliteStore,
    services::{freedb::FreedBService, musicbrainz::MusicBrainzService},
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

/// Spawn a test server that uses a temporary uploads directory.
/// Returns `(base_url, temp_dir)` — keep `temp_dir` alive for the duration of the test.
async fn spawn_test_server() -> (String, TempDir) {
    let uploads_tmp = TempDir::new().unwrap();
    let uploads_path = uploads_tmp.path().to_path_buf();

    let store = SqliteStore::new("sqlite::memory:")
        .await
        .expect("SQLite failed");
    store.migrate().await.expect("migrations failed");

    let config = Config {
        database_url: "sqlite::memory:".into(),
        jwt_secret: "test-secret-32-chars-minimum-xxxx".into(),
        port: 0,
        log_level: "error".into(),
        rp_id: "localhost".into(),
        rp_origin: "http://localhost:3000".into(),
        uploads_dir: uploads_path,
    };
    let mb_service = Arc::new(MusicBrainzService::new(String::new()));
    let freedb_service = Arc::new(FreedBService::new());
    let state = AppState::new(
        Arc::new(store),
        config,
        test_webauthn(),
        mb_service,
        freedb_service,
    );
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), uploads_tmp)
}

async fn login_admin(base: &str) -> reqwest::Client {
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();
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

#[tokio::test]
async fn test_image_upload_and_serve() {
    let (base, _tmp) = spawn_test_server().await;
    let client = login_admin(&base).await;

    // Upload the 1×1 PNG fixture
    let png = include_bytes!("fixtures/1x1.png");
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(png.to_vec())
            .file_name("bg.png")
            .mime_str("image/png")
            .unwrap(),
    );

    let resp = client
        .post(format!("{base}/api/v1/uploads/images"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201);

    let body: serde_json::Value = resp.json().await.unwrap();
    let url = body["url"].as_str().unwrap();
    assert!(
        url.starts_with("/uploads/"),
        "URL must be a local path, got: {url}"
    );
    assert!(url.ends_with(".png"), "URL must end with .png, got: {url}");

    // File must be serveable via GET
    let serve_resp = client
        .get(format!("{base}{url}"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        serve_resp.status().as_u16(),
        200,
        "expected 200 serving uploaded file at {url}"
    );
    assert_eq!(
        serve_resp
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "image/png"
    );
}

#[tokio::test]
async fn test_upload_rejects_non_image_mime() {
    let (base, _tmp) = spawn_test_server().await;
    let client = login_admin(&base).await;

    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(b"not an image".to_vec())
            .file_name("evil.exe")
            .mime_str("application/octet-stream")
            .unwrap(),
    );

    let resp = client
        .post(format!("{base}/api/v1/uploads/images"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status().as_u16(),
        400,
        "expected 400 for non-image MIME type"
    );
}

#[tokio::test]
async fn test_upload_requires_auth() {
    let (base, _tmp) = spawn_test_server().await;
    let client = reqwest::Client::new();

    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(b"data".to_vec())
            .file_name("bg.png")
            .mime_str("image/png")
            .unwrap(),
    );

    let resp = client
        .post(format!("{base}/api/v1/uploads/images"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status().as_u16(),
        401,
        "expected 401 for unauthenticated upload"
    );
}
