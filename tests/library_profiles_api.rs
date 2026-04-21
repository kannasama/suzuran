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

/// Creates a library and encoding profile, then returns (library_id, encoding_profile_id).
async fn create_prereqs(base: &str, client: &reqwest::Client) -> (i64, i64) {
    let lib: serde_json::Value = client
        .post(format!("{base}/api/v1/libraries"))
        .json(&serde_json::json!({
            "name": "Test Library",
            "root_path": "/music",
            "format": "flac"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let library_id = lib["id"].as_i64().unwrap();

    let ep: serde_json::Value = client
        .post(format!("{base}/api/v1/encoding-profiles"))
        .json(&serde_json::json!({
            "name": "AAC 256k",
            "codec": "aac",
            "bitrate": "256k",
            "sample_rate": null,
            "channels": null,
            "bit_depth": null,
            "advanced_args": null
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let encoding_profile_id = ep["id"].as_i64().unwrap();

    (library_id, encoding_profile_id)
}

#[tokio::test]
async fn test_create_library_profile() {
    let base = spawn_test_server().await;
    let client = login_admin(&base).await;
    let (library_id, encoding_profile_id) = create_prereqs(&base, &client).await;

    let resp = client
        .post(format!("{base}/api/v1/library-profiles"))
        .json(&serde_json::json!({
            "library_id": library_id,
            "encoding_profile_id": encoding_profile_id,
            "derived_dir_name": "lossy",
            "include_on_submit": true,
            "auto_include_above_hz": null
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201);
    let profile: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(profile["library_id"], library_id);
    assert_eq!(profile["encoding_profile_id"], encoding_profile_id);
    assert_eq!(profile["derived_dir_name"], "lossy");
    assert_eq!(profile["include_on_submit"], true);
}

#[tokio::test]
async fn test_list_library_profiles() {
    let base = spawn_test_server().await;
    let client = login_admin(&base).await;
    let (library_id, encoding_profile_id) = create_prereqs(&base, &client).await;

    // Create one profile
    client
        .post(format!("{base}/api/v1/library-profiles"))
        .json(&serde_json::json!({
            "library_id": library_id,
            "encoding_profile_id": encoding_profile_id,
            "derived_dir_name": "lossy",
            "include_on_submit": true,
            "auto_include_above_hz": null
        }))
        .send()
        .await
        .unwrap();

    // List by library_id
    let list: Vec<serde_json::Value> = client
        .get(format!("{base}/api/v1/library-profiles?library_id={library_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(!list.is_empty());
    assert!(list.iter().any(|p| p["derived_dir_name"] == "lossy"));
}

#[tokio::test]
async fn test_get_library_profile() {
    let base = spawn_test_server().await;
    let client = login_admin(&base).await;
    let (library_id, encoding_profile_id) = create_prereqs(&base, &client).await;

    let created: serde_json::Value = client
        .post(format!("{base}/api/v1/library-profiles"))
        .json(&serde_json::json!({
            "library_id": library_id,
            "encoding_profile_id": encoding_profile_id,
            "derived_dir_name": "lossy",
            "include_on_submit": false,
            "auto_include_above_hz": 48000
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = created["id"].as_i64().unwrap();

    let fetched: serde_json::Value = client
        .get(format!("{base}/api/v1/library-profiles/{id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(fetched["id"], id);
    assert_eq!(fetched["derived_dir_name"], "lossy");
    assert_eq!(fetched["auto_include_above_hz"], 48000);
}

#[tokio::test]
async fn test_update_library_profile() {
    let base = spawn_test_server().await;
    let client = login_admin(&base).await;
    let (library_id, encoding_profile_id) = create_prereqs(&base, &client).await;

    let created: serde_json::Value = client
        .post(format!("{base}/api/v1/library-profiles"))
        .json(&serde_json::json!({
            "library_id": library_id,
            "encoding_profile_id": encoding_profile_id,
            "derived_dir_name": "lossy",
            "include_on_submit": false,
            "auto_include_above_hz": null
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = created["id"].as_i64().unwrap();

    let updated: serde_json::Value = client
        .put(format!("{base}/api/v1/library-profiles/{id}"))
        .json(&serde_json::json!({
            "library_id": library_id,
            "encoding_profile_id": encoding_profile_id,
            "derived_dir_name": "hi-res-lossy",
            "include_on_submit": true,
            "auto_include_above_hz": 96000
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(updated["derived_dir_name"], "hi-res-lossy");
    assert_eq!(updated["include_on_submit"], true);
    assert_eq!(updated["auto_include_above_hz"], 96000);
}

#[tokio::test]
async fn test_delete_library_profile() {
    let base = spawn_test_server().await;
    let client = login_admin(&base).await;
    let (library_id, encoding_profile_id) = create_prereqs(&base, &client).await;

    let created: serde_json::Value = client
        .post(format!("{base}/api/v1/library-profiles"))
        .json(&serde_json::json!({
            "library_id": library_id,
            "encoding_profile_id": encoding_profile_id,
            "derived_dir_name": "lossy",
            "include_on_submit": true,
            "auto_include_above_hz": null
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = created["id"].as_i64().unwrap();

    let status = client
        .delete(format!("{base}/api/v1/library-profiles/{id}"))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status.as_u16(), 204);
}

#[tokio::test]
async fn test_auth_guard_list() {
    let base = spawn_test_server().await;
    let client = reqwest::Client::new();
    // Missing library_id param — but auth check comes first
    let resp = client
        .get(format!("{base}/api/v1/library-profiles?library_id=1"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 401);
}

#[tokio::test]
async fn test_auth_guard_create() {
    let base = spawn_test_server().await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base}/api/v1/library-profiles"))
        .json(&serde_json::json!({
            "library_id": 1,
            "encoding_profile_id": 1,
            "derived_dir_name": "lossy",
            "include_on_submit": true,
            "auto_include_above_hz": null
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 401);
}

#[tokio::test]
async fn test_admin_guard_create() {
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
    member_client
        .post(format!("{base}/api/v1/auth/login"))
        .json(&serde_json::json!({
            "username": "member",
            "password": member_password
        }))
        .send()
        .await
        .unwrap();

    let resp = member_client
        .post(format!("{base}/api/v1/library-profiles"))
        .json(&serde_json::json!({
            "library_id": 1,
            "encoding_profile_id": 1,
            "derived_dir_name": "lossy",
            "include_on_submit": true,
            "auto_include_above_hz": null
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 403);
}
