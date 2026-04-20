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

// ── CRUD test ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn virtual_libraries_crud() {
    let base = spawn_test_server().await;
    let client = login_admin(&base).await;

    // Create → 201
    let resp = client
        .post(format!("{base}/api/v1/virtual-libraries"))
        .json(&serde_json::json!({
            "name": "Symlink Library",
            "root_path": "/srv/vlib/symlinks",
            "link_type": "symlink"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201);
    let vlib: serde_json::Value = resp.json().await.unwrap();
    let vlib_id = vlib["id"].as_i64().unwrap();
    assert_eq!(vlib["name"], "Symlink Library");
    assert_eq!(vlib["link_type"], "symlink");

    // List → 1 item
    let list: Vec<serde_json::Value> = client
        .get(format!("{base}/api/v1/virtual-libraries"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list.len(), 1);

    // Get by id
    let one: serde_json::Value = client
        .get(format!("{base}/api/v1/virtual-libraries/{vlib_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(one["name"], "Symlink Library");

    // Update → 200
    let updated: serde_json::Value = client
        .put(format!("{base}/api/v1/virtual-libraries/{vlib_id}"))
        .json(&serde_json::json!({
            "name": "Hardlink Library",
            "root_path": "/srv/vlib/hardlinks",
            "link_type": "hardlink"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(updated["name"], "Hardlink Library");
    assert_eq!(updated["link_type"], "hardlink");

    // Delete → 204
    let status = client
        .delete(format!("{base}/api/v1/virtual-libraries/{vlib_id}"))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status.as_u16(), 204);

    // List after delete → empty
    let after: Vec<serde_json::Value> = client
        .get(format!("{base}/api/v1/virtual-libraries"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(after.is_empty());
}

// ── Auth guard test ───────────────────────────────────────────────────────────

#[tokio::test]
async fn virtual_libraries_list_requires_auth() {
    let base = spawn_test_server().await;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{base}/api/v1/virtual-libraries"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 401);
}

#[tokio::test]
async fn virtual_libraries_create_requires_admin() {
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
        .post(format!("{base}/api/v1/virtual-libraries"))
        .json(&serde_json::json!({
            "name": "Test",
            "root_path": "/tmp/test",
            "link_type": "symlink"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 403);
}

// ── Sources test ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn virtual_libraries_sources() {
    let (base, store) = spawn_test_server_with_store().await;
    let client = login_admin(&base).await;

    // Create two regular libraries in the DB directly
    let lib1 = store
        .create_library("Lib 1", "/music/lib1", "flac", None)
        .await
        .unwrap();
    let lib2 = store
        .create_library("Lib 2", "/music/lib2", "mp3", None)
        .await
        .unwrap();

    // Create a virtual library
    let resp = client
        .post(format!("{base}/api/v1/virtual-libraries"))
        .json(&serde_json::json!({
            "name": "Multi-source VLib",
            "root_path": "/srv/vlib/multi",
            "link_type": "symlink"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201);
    let vlib: serde_json::Value = resp.json().await.unwrap();
    let vlib_id = vlib["id"].as_i64().unwrap();

    // Set sources
    let set_resp = client
        .put(format!("{base}/api/v1/virtual-libraries/{vlib_id}/sources"))
        .json(&serde_json::json!([
            { "library_id": lib1.id, "priority": 1 },
            { "library_id": lib2.id, "priority": 2 }
        ]))
        .send()
        .await
        .unwrap();
    assert_eq!(set_resp.status().as_u16(), 204);

    // Get sources → ordered by priority
    let sources: Vec<serde_json::Value> = client
        .get(format!("{base}/api/v1/virtual-libraries/{vlib_id}/sources"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(sources.len(), 2);
    assert_eq!(sources[0]["library_id"], lib1.id);
    assert_eq!(sources[0]["priority"], 1);
    assert_eq!(sources[1]["library_id"], lib2.id);
    assert_eq!(sources[1]["priority"], 2);

    // Replace sources atomically (swap order)
    let replace_resp = client
        .put(format!("{base}/api/v1/virtual-libraries/{vlib_id}/sources"))
        .json(&serde_json::json!([
            { "library_id": lib2.id, "priority": 1 },
            { "library_id": lib1.id, "priority": 2 }
        ]))
        .send()
        .await
        .unwrap();
    assert_eq!(replace_resp.status().as_u16(), 204);

    let updated_sources: Vec<serde_json::Value> = client
        .get(format!("{base}/api/v1/virtual-libraries/{vlib_id}/sources"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(updated_sources.len(), 2);
    assert_eq!(updated_sources[0]["library_id"], lib2.id);
    assert_eq!(updated_sources[0]["priority"], 1);
}

// ── Sync enqueue test ─────────────────────────────────────────────────────────

#[tokio::test]
async fn virtual_libraries_sync_enqueue() {
    let (base, store) = spawn_test_server_with_store().await;
    let client = login_admin(&base).await;

    // Create a virtual library
    let resp = client
        .post(format!("{base}/api/v1/virtual-libraries"))
        .json(&serde_json::json!({
            "name": "Sync Test VLib",
            "root_path": "/srv/vlib/sync",
            "link_type": "symlink"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201);
    let vlib: serde_json::Value = resp.json().await.unwrap();
    let vlib_id = vlib["id"].as_i64().unwrap();

    // POST /:id/sync → 202
    let sync_resp = client
        .post(format!("{base}/api/v1/virtual-libraries/{vlib_id}/sync"))
        .send()
        .await
        .unwrap();
    assert_eq!(sync_resp.status().as_u16(), 202);

    // Verify a virtual_sync job was enqueued as pending
    let jobs = store.list_jobs(Some("pending"), 50).await.unwrap();
    let sync_jobs: Vec<_> = jobs
        .iter()
        .filter(|j| {
            j.job_type == "virtual_sync"
                && j.payload
                    .get("virtual_library_id")
                    .and_then(|v| v.as_i64())
                    == Some(vlib_id)
        })
        .collect();
    assert!(!sync_jobs.is_empty(), "expected a pending virtual_sync job");
}
