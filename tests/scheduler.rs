use std::{sync::Arc, time::Duration};
use url::Url;
use webauthn_rs::WebauthnBuilder;
use suzuran_server::{
    build_router, config::Config, dal::{sqlite::SqliteStore, Store}, scheduler::Scheduler,
    services::musicbrainz::MusicBrainzService,
    state::AppState,
};

async fn spawn_test_server() -> String {
    let store = Arc::new(SqliteStore::new("sqlite::memory:").await.unwrap());
    store.migrate().await.unwrap();

    let origin = Url::parse("http://localhost:3000").unwrap();
    let webauthn = WebauthnBuilder::new("localhost", &origin)
        .unwrap().rp_name("test").build().unwrap();

    let config = Config {
        database_url: "sqlite::memory:".into(),
        jwt_secret: "test-secret-32-chars-minimum-xxxx".into(),
        port: 0,
        log_level: "error".into(),
        rp_id: "localhost".into(),
        rp_origin: "http://localhost:3000".into(),
    };

    let db: Arc<dyn Store> = store.clone();
    let mb_service = Arc::new(MusicBrainzService::new(String::new()));
    let state = AppState::new(db.clone(), config, webauthn, mb_service.clone());

    // Spawn scheduler against the same DB
    let scheduler = Arc::new(Scheduler::new(db, mb_service));
    tokio::spawn({ let s = scheduler.clone(); async move { s.run().await } });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, build_router(state)).await.unwrap() });
    format!("http://{addr}")
}

async fn admin_client(base: &str) -> reqwest::Client {
    let client = reqwest::Client::builder().cookie_store(true).build().unwrap();
    client.post(format!("{base}/api/v1/auth/register"))
        .json(&serde_json::json!({"username":"admin","email":"a@a.com","password":"password123"}))
        .send().await.unwrap();
    client.post(format!("{base}/api/v1/auth/login"))
        .json(&serde_json::json!({"username":"admin","password":"password123"}))
        .send().await.unwrap();
    client
}

#[tokio::test]
async fn scan_job_enqueues_and_completes() {
    let base = spawn_test_server().await;
    let client = admin_client(&base).await;

    // Create a temp library root with one fake audio file
    let dir = tempfile::TempDir::new().unwrap();
    tokio::fs::write(dir.path().join("song.flac"), b"").await.unwrap();

    // Create library (no trailing slash)
    let lib_res = client.post(format!("{base}/api/v1/libraries"))
        .json(&serde_json::json!({
            "name": "Test Library",
            "root_path": dir.path().to_str().unwrap(),
            "format": "flac"
        }))
        .send().await.unwrap();
    assert_eq!(lib_res.status(), 201);
    let lib: serde_json::Value = lib_res.json().await.unwrap();
    let lib_id = lib["id"].as_i64().unwrap();

    // Enqueue scan job
    let job_res = client.post(format!("{base}/api/v1/jobs/scan"))
        .json(&serde_json::json!({"library_id": lib_id}))
        .send().await.unwrap();
    assert_eq!(job_res.status(), 201);
    let job: serde_json::Value = job_res.json().await.unwrap();
    let job_id = job["id"].as_i64().unwrap();

    // Wait for scheduler to process (up to 15 seconds)
    let mut completed = false;
    for _ in 0..15 {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let status_res = client.get(format!("{base}/api/v1/jobs/{job_id}"))
            .send().await.unwrap();
        let j: serde_json::Value = status_res.json().await.unwrap();
        if j["status"] == "completed" {
            completed = true;
            // Verify the file was scanned
            let tracks_res = client.get(format!("{base}/api/v1/libraries/{lib_id}/tracks"))
                .send().await.unwrap();
            let tracks: Vec<serde_json::Value> = tracks_res.json().await.unwrap();
            assert_eq!(tracks.len(), 1, "one track should be in the library");
            break;
        }
    }

    assert!(completed, "scan job did not complete within 15 seconds");
}
