use std::sync::Arc;
use url::Url;
use webauthn_rs::WebauthnBuilder;
use suzuran_server::{
    build_router, config::Config,
    dal::{sqlite::SqliteStore, Store, UpsertTrack},
    services::{freedb::FreedBService, musicbrainz::MusicBrainzService},
    state::AppState,
};

async fn spawn_test_server() -> (String, reqwest::Client) {
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

    let mb_service = Arc::new(MusicBrainzService::new(String::new()));
    let freedb_service = Arc::new(FreedBService::new());
    let state = AppState::new(store.clone() as Arc<dyn Store>, config, webauthn, mb_service, freedb_service);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, build_router(state)).await.unwrap() });

    let base = format!("http://{addr}");
    let client = reqwest::Client::builder().cookie_store(true).build().unwrap();

    // Register + login
    client.post(format!("{base}/api/v1/auth/register"))
        .json(&serde_json::json!({"username":"admin","email":"a@a.com","password":"password123"}))
        .send().await.unwrap();
    client.post(format!("{base}/api/v1/auth/login"))
        .json(&serde_json::json!({"username":"admin","password":"password123"}))
        .send().await.unwrap();

    // Seed a real file we can stream
    let dir = tempfile::TempDir::new().unwrap();
    let file_content = b"FAKE_AUDIO_CONTENT_FOR_TESTING_1234567890";
    let file_path = dir.path().join("test.mp3");
    tokio::fs::write(&file_path, file_content).await.unwrap();

    let lib = store.create_library("Test", dir.path().to_str().unwrap(), "mp3", None).await.unwrap();

    store.upsert_track(UpsertTrack {
        library_id: lib.id,
        relative_path: "test.mp3".into(),
        file_hash: "abc123".into(),
        title: Some("Test Track".into()),
        artist: None, albumartist: None, album: None,
        tracknumber: None, discnumber: None, totaldiscs: None, totaltracks: None,
        date: None, genre: None, composer: None, label: None, catalognumber: None,
        tags: serde_json::json!({}),
        duration_secs: Some(3.0),
        bitrate: Some(320),
        sample_rate: Some(44100),
        channels: Some(2),
        has_embedded_art: false,
    }).await.unwrap();

    // Keep dir alive for the duration of the test process
    std::mem::forget(dir);

    (base, client)
}

#[tokio::test]
async fn stream_full_file() {
    let (base, client) = spawn_test_server().await;

    let res = client.get(format!("{base}/api/v1/tracks/1/stream"))
        .send().await.unwrap();

    assert_eq!(res.status(), 200);
    assert!(res.headers().get("accept-ranges").is_some());
    assert!(res.headers().get("content-length").is_some());

    let body = res.bytes().await.unwrap();
    assert_eq!(&body[..], b"FAKE_AUDIO_CONTENT_FOR_TESTING_1234567890");
}

#[tokio::test]
async fn stream_range_request() {
    let (base, client) = spawn_test_server().await;

    let res = client.get(format!("{base}/api/v1/tracks/1/stream"))
        .header("Range", "bytes=0-3")
        .send().await.unwrap();

    assert_eq!(res.status(), 206);
    assert!(res.headers().get("content-range").is_some());

    let body = res.bytes().await.unwrap();
    assert_eq!(&body[..], b"FAKE");
}

#[tokio::test]
async fn stream_head_returns_metadata() {
    let (base, client) = spawn_test_server().await;

    let res = client
        .request(reqwest::Method::HEAD, format!("{base}/api/v1/tracks/1/stream"))
        .send().await.unwrap();

    assert_eq!(res.status(), 200);
    assert!(res.headers().get("accept-ranges").is_some());
    assert_eq!(res.headers().get("x-duration-secs").unwrap(), "3");
    // Axum strips the body and resets Content-Length to 0 on HEAD responses;
    // X-File-Size carries the actual file size independently.
    assert_eq!(res.headers().get("x-file-size").unwrap(), "41");
}

#[tokio::test]
async fn stream_requires_auth() {
    let (base, _) = spawn_test_server().await;
    let anon = reqwest::Client::new();
    let res = anon.get(format!("{base}/api/v1/tracks/1/stream")).send().await.unwrap();
    assert_eq!(res.status(), 401);
}
