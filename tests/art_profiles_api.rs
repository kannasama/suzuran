use std::sync::Arc;
use url::Url;
use webauthn_rs::WebauthnBuilder;

use suzuran_server::{
    build_router,
    config::Config,
    dal::{sqlite::SqliteStore, Store},
    services::auth::AuthService,
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

async fn spawn_test_server() -> String {
    let (base, _store) = spawn_test_server_with_store().await;
    base
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

#[tokio::test]
async fn art_profiles_crud() {
    let base = spawn_test_server().await;
    let client = login_admin(&base).await;

    // Create → 201
    let resp = client
        .post(format!("{base}/api/v1/art-profiles"))
        .json(&serde_json::json!({
            "name": "Cover 600px JPEG",
            "max_width_px": 600,
            "max_height_px": 600,
            "max_size_bytes": 204800,
            "format": "jpeg",
            "quality": 90,
            "apply_to_library_id": null
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201);
    let ap: serde_json::Value = resp.json().await.unwrap();
    let ap_id = ap["id"].as_i64().unwrap();
    assert_eq!(ap["name"], "Cover 600px JPEG");
    assert_eq!(ap["format"], "jpeg");
    assert_eq!(ap["quality"].as_i64().unwrap(), 90);

    // List → 1 item
    let list: Vec<serde_json::Value> = client
        .get(format!("{base}/api/v1/art-profiles"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list.len(), 1);

    // Get by id
    let one: serde_json::Value = client
        .get(format!("{base}/api/v1/art-profiles/{ap_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(one["name"], "Cover 600px JPEG");

    // Update → 200
    let updated: serde_json::Value = client
        .put(format!("{base}/api/v1/art-profiles/{ap_id}"))
        .json(&serde_json::json!({
            "name": "Cover 1200px PNG",
            "max_width_px": 1200,
            "max_height_px": 1200,
            "max_size_bytes": null,
            "format": "png",
            "quality": 95,
            "apply_to_library_id": null
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(updated["name"], "Cover 1200px PNG");
    assert_eq!(updated["format"], "png");
    assert_eq!(updated["max_width_px"].as_i64().unwrap(), 1200);

    // Delete → 204
    let status = client
        .delete(format!("{base}/api/v1/art-profiles/{ap_id}"))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status.as_u16(), 204);

    // List after delete → empty
    let after: Vec<serde_json::Value> = client
        .get(format!("{base}/api/v1/art-profiles"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(after.is_empty());
}

#[tokio::test]
async fn art_profiles_list_requires_auth() {
    let base = spawn_test_server().await;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{base}/api/v1/art-profiles"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 401);
}

#[tokio::test]
async fn art_profiles_create_requires_admin() {
    let (base, store) = spawn_test_server_with_store().await;

    // First user becomes admin
    let _admin_client = login_admin(&base).await;

    // Create a non-admin member via DAL
    let member_password = "memberpass123";
    let member_hash = AuthService::hash_password(member_password).unwrap();
    store
        .create_user("member", "member@test.com", &member_hash, "user")
        .await
        .unwrap();

    let member_client = reqwest::Client::builder().cookie_store(true).build().unwrap();
    let login_resp = member_client
        .post(format!("{base}/api/v1/auth/login"))
        .json(&serde_json::json!({
            "username": "member",
            "password": member_password
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(login_resp.status().as_u16(), 204);

    // Non-admin POST → 403
    let resp = member_client
        .post(format!("{base}/api/v1/art-profiles"))
        .json(&serde_json::json!({
            "name": "Small Thumb",
            "max_width_px": 200,
            "max_height_px": 200,
            "max_size_bytes": null,
            "format": "jpeg",
            "quality": 75,
            "apply_to_library_id": null
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 403);
}
