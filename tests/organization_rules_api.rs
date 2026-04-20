use std::sync::Arc;
use url::Url;
use webauthn_rs::WebauthnBuilder;

use suzuran_server::{
    build_router,
    config::Config,
    dal::{sqlite::SqliteStore, Store},
    services::auth::AuthService,
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
    };
    let mb_service = Arc::new(MusicBrainzService::new(String::new()));
    let state = AppState::new(Arc::clone(&store), config, test_webauthn(), mb_service);
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
    // Register first user (becomes admin)
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
    // Login uses username, not email
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
async fn org_rules_crud_via_api() {
    let base = spawn_test_server().await;
    let client = login_admin(&base).await;

    // Create a library
    let lib: serde_json::Value = client
        .post(format!("{base}/api/v1/libraries"))
        .json(&serde_json::json!({"name": "FLAC", "root_path": "/tmp/flac", "format": "flac"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let lib_id = lib["id"].as_i64().unwrap();

    // Create rule → 201
    let resp = client
        .post(format!("{base}/api/v1/organization-rules"))
        .json(&serde_json::json!({
            "name": "Default",
            "library_id": null,
            "priority": 0,
            "conditions": null,
            "path_template": "{albumartist}/{date} - {album}/{tracknumber:02} - {title}",
            "enabled": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let rule: serde_json::Value = resp.json().await.unwrap();
    let rule_id = rule["id"].as_i64().unwrap();
    assert_eq!(rule["name"], "Default");

    // List all → 1 rule
    let all: Vec<serde_json::Value> = client
        .get(format!("{base}/api/v1/organization-rules"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(all.len(), 1);

    // List filtered by library → includes global rule
    let filtered: Vec<serde_json::Value> = client
        .get(format!("{base}/api/v1/organization-rules?library_id={lib_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(filtered.len(), 1);

    // Get one
    let one: serde_json::Value = client
        .get(format!("{base}/api/v1/organization-rules/{rule_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(one["name"], "Default");

    // Update → 200
    let updated: serde_json::Value = client
        .put(format!("{base}/api/v1/organization-rules/{rule_id}"))
        .json(&serde_json::json!({
            "name": "Renamed",
            "priority": 5,
            "conditions": null,
            "path_template": "{title}",
            "enabled": false
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(updated["name"], "Renamed");
    assert!(!updated["enabled"].as_bool().unwrap());

    // Delete → 204
    let status = client
        .delete(format!("{base}/api/v1/organization-rules/{rule_id}"))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status.as_u16(), 204);

    // List after delete → empty
    let after: Vec<serde_json::Value> = client
        .get(format!("{base}/api/v1/organization-rules"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(after.is_empty());
}

#[tokio::test]
async fn create_rule_requires_admin() {
    let (base, store) = spawn_test_server_with_store().await;

    // Register admin via HTTP (first user gets admin role automatically)
    let _admin_client = login_admin(&base).await;

    // Create a non-admin "user" role account directly via DAL (HTTP register is blocked after first user)
    let member_password = "memberpass123";
    let member_hash = AuthService::hash_password(member_password)
        .expect("argon2 hashing failed");
    store
        .create_user("member", "member@test.com", &member_hash, "user")
        .await
        .expect("create member user failed");

    // Log in as the member user via HTTP
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
    assert_eq!(login_resp.status().as_u16(), 204, "member login should succeed");

    // Attempt to create an organization rule as a non-admin member → expect 403
    let resp = member_client
        .post(format!("{base}/api/v1/organization-rules"))
        .json(&serde_json::json!({
            "name": "Unauthorized Rule",
            "library_id": null,
            "priority": 0,
            "conditions": null,
            "path_template": "{title}",
            "enabled": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status().as_u16(),
        403,
        "non-admin member should be forbidden from creating organization rules"
    );
}
