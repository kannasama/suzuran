use std::sync::Arc;
use url::Url;
use webauthn_rs::WebauthnBuilder;

use suzuran_server::{
    build_router,
    config::Config,
    dal::{sqlite::SqliteStore, Store, UpsertEncodingProfile, UpsertTrack},
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

async fn spawn_test_server() -> (String, Arc<dyn Store>) {
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

#[tokio::test]
async fn transcode_track_requires_auth() {
    let (base, _store) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base}/api/v1/tracks/1/transcode"))
        .json(&serde_json::json!({ "target_library_id": 1 }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 401);
}

#[tokio::test]
async fn transcode_library_requires_auth() {
    let (base, _store) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base}/api/v1/libraries/1/transcode"))
        .json(&serde_json::json!({ "target_library_id": 1 }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 401);
}

#[tokio::test]
async fn transcode_library_sync_requires_auth() {
    let (base, _store) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base}/api/v1/libraries/1/transcode-sync"))
        .json(&serde_json::json!({ "target_library_id": 1 }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 401);
}

#[tokio::test]
async fn transcode_track_not_found() {
    let (base, _store) = spawn_test_server().await;
    let client = login_admin(&base).await;
    let resp = client
        .post(format!("{base}/api/v1/tracks/9999/transcode"))
        .json(&serde_json::json!({ "target_library_id": 1 }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 404);
}

#[tokio::test]
async fn transcode_track_enqueues_job() {
    let (base, store) = spawn_test_server().await;
    let client = login_admin(&base).await;

    // Create source and target libraries
    let src_lib = store
        .create_library("Source", "/music/src", "flac", None)
        .await
        .unwrap();
    let tgt_lib = store
        .create_library("Target", "/music/tgt", "aac", None)
        .await
        .unwrap();

    // Create an encoding profile and attach to target library
    let ep = store
        .create_encoding_profile(UpsertEncodingProfile {
            name: "AAC 256k".into(),
            codec: "aac".into(),
            bitrate: Some("256k".into()),
            sample_rate: None,
            channels: None,
            bit_depth: None,
            advanced_args: None,
        })
        .await
        .unwrap();
    store
        .set_library_encoding_profile(tgt_lib.id, Some(ep.id))
        .await
        .unwrap();

    // Insert a track into source library
    let track = store
        .upsert_track(UpsertTrack {
            library_id: src_lib.id,
            relative_path: "01 - Song.flac".into(),
            file_hash: "abc123".into(),
            title: Some("Song".into()),
            tags: serde_json::json!({}),
            ..UpsertTrack::default()
        })
        .await
        .unwrap();

    // POST /api/v1/tracks/:id/transcode
    let resp = client
        .post(format!("{base}/api/v1/tracks/{}/transcode", track.id))
        .json(&serde_json::json!({ "target_library_id": tgt_lib.id }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 202);

    // Verify a job was enqueued
    let jobs = store.list_jobs(Some("pending"), 10).await.unwrap();
    let transcode_jobs: Vec<_> = jobs.iter().filter(|j| j.job_type == "transcode").collect();
    assert_eq!(transcode_jobs.len(), 1);
    assert_eq!(
        transcode_jobs[0].payload["source_track_id"].as_i64().unwrap(),
        track.id
    );
    assert_eq!(
        transcode_jobs[0].payload["target_library_id"].as_i64().unwrap(),
        tgt_lib.id
    );
}

#[tokio::test]
async fn transcode_library_enqueues_all_tracks() {
    let (base, store) = spawn_test_server().await;
    let client = login_admin(&base).await;

    let src_lib = store
        .create_library("Source", "/music/src2", "flac", None)
        .await
        .unwrap();
    let tgt_lib = store
        .create_library("Target2", "/music/tgt2", "aac", None)
        .await
        .unwrap();

    // Insert two tracks
    for i in 1..=2_u32 {
        store
            .upsert_track(UpsertTrack {
                library_id: src_lib.id,
                relative_path: format!("0{i} - Song.flac"),
                file_hash: format!("hash{i}"),
                tags: serde_json::json!({}),
                ..UpsertTrack::default()
            })
            .await
            .unwrap();
    }

    let resp = client
        .post(format!("{base}/api/v1/libraries/{}/transcode", src_lib.id))
        .json(&serde_json::json!({ "target_library_id": tgt_lib.id }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 202);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["enqueued"].as_i64().unwrap(), 2);
}

#[tokio::test]
async fn transcode_library_sync_skips_already_linked() {
    let (base, store) = spawn_test_server().await;
    let client = login_admin(&base).await;

    let src_lib = store
        .create_library("SrcSync", "/music/srcsync", "flac", None)
        .await
        .unwrap();
    let tgt_lib = store
        .create_library("TgtSync", "/music/tgtsync", "aac", None)
        .await
        .unwrap();

    // Insert two source tracks
    let t1 = store
        .upsert_track(UpsertTrack {
            library_id: src_lib.id,
            relative_path: "01 - A.flac".into(),
            file_hash: "hash_a".into(),
            tags: serde_json::json!({}),
            ..UpsertTrack::default()
        })
        .await
        .unwrap();
    let t2 = store
        .upsert_track(UpsertTrack {
            library_id: src_lib.id,
            relative_path: "02 - B.flac".into(),
            file_hash: "hash_b".into(),
            tags: serde_json::json!({}),
            ..UpsertTrack::default()
        })
        .await
        .unwrap();

    // Insert a derived track for t1 in the target library + link it
    let derived = store
        .upsert_track(UpsertTrack {
            library_id: tgt_lib.id,
            relative_path: "01 - A.aac".into(),
            file_hash: "hash_a_derived".into(),
            tags: serde_json::json!({}),
            ..UpsertTrack::default()
        })
        .await
        .unwrap();
    store
        .create_track_link(t1.id, derived.id, None)
        .await
        .unwrap();

    // transcode-sync should only enqueue for t2 (t1 already linked)
    let resp = client
        .post(format!("{base}/api/v1/libraries/{}/transcode-sync", src_lib.id))
        .json(&serde_json::json!({ "target_library_id": tgt_lib.id }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 202);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["enqueued"].as_i64().unwrap(), 1);

    // Verify the enqueued job is for t2
    let jobs = store.list_jobs(Some("pending"), 10).await.unwrap();
    let transcode_jobs: Vec<_> = jobs.iter().filter(|j| j.job_type == "transcode").collect();
    assert_eq!(transcode_jobs.len(), 1);
    assert_eq!(
        transcode_jobs[0].payload["source_track_id"].as_i64().unwrap(),
        t2.id
    );
}
