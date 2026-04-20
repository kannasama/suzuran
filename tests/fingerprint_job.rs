use std::sync::Arc;
use tempfile::TempDir;
use suzuran_server::{
    dal::{sqlite::SqliteStore, Store, UpsertTrack},
    jobs::{fingerprint::FingerprintJobHandler, JobHandler},
};

async fn make_db() -> Arc<dyn Store> {
    let store = SqliteStore::new("sqlite::memory:").await.unwrap();
    store.migrate().await.unwrap();
    Arc::new(store)
}

/// Returns true if `fpcalc` is present on the PATH.
fn fpcalc_available() -> bool {
    std::process::Command::new("fpcalc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Minimal valid FLAC file: 1 second of silence at 44100 Hz, mono, 16-bit.
/// Generated with: ffmpeg -f lavfi -i aevalsrc=0 -t 1 -ar 44100 -ac 1 -c:a flac out.flac
/// then `xxd -i out.flac` truncated to a valid parseable FLAC.
///
/// This is the raw bytes of a minimal fpcalc-processable FLAC file (silence.flac).
/// The file is 2828 bytes.
const SILENCE_FLAC: &[u8] = &[
    0x66, 0x4c, 0x61, 0x43, // "fLaC" marker
    0x80, 0x00, 0x00, 0x22, // STREAMINFO block (last-metadata=1), length=34
    0x00, 0x12, 0x00, 0x12, // min/max blocksize = 18
    0x00, 0x00, 0x0e, 0x00, 0x00, 0x0e, // min/max framesize = 14
    0x0a, 0xc4, 0x42, 0xf0, 0x00, 0x00, 0xac, 0x44, // samplerate=44100, ch=1, bps=16, samples=44100
    // MD5 signature (16 bytes, all zero for silence)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// Set up an in-memory DB with a track pointing to a real audio file.
/// Returns (store, track_id, TempDir) — keep TempDir alive to prevent cleanup.
async fn setup_with_audio_track() -> (Arc<dyn Store>, i64, TempDir) {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    let track_file = root.join("test_track.flac");
    tokio::fs::write(&track_file, SILENCE_FLAC).await.unwrap();

    let db = make_db().await;
    let lib = db
        .create_library("Test", root.to_str().unwrap(), "flac", None)
        .await
        .unwrap();

    let track = db
        .upsert_track(UpsertTrack {
            library_id: lib.id,
            relative_path: "test_track.flac".into(),
            file_hash: "abc123".into(),
            title: Some("Silence".into()),
            artist: Some("Test Artist".into()),
            albumartist: None,
            album: None,
            tracknumber: None,
            discnumber: None,
            totaldiscs: None,
            totaltracks: None,
            date: None,
            genre: None,
            composer: None,
            label: None,
            catalognumber: None,
            tags: serde_json::json!({}),
            duration_secs: None,
            bitrate: None,
            sample_rate: None,
            channels: None,
            bit_depth: None,
            has_embedded_art: false,
        })
        .await
        .unwrap();

    (db, track.id, dir)
}

#[tokio::test]
async fn test_fingerprint_stores_in_track_tags() {
    if !fpcalc_available() {
        eprintln!("SKIP: fpcalc not found on PATH — skipping fingerprint test");
        return;
    }

    let (store, track_id, _dir) = setup_with_audio_track().await;

    let handler = FingerprintJobHandler;
    let result = handler
        .run(store.clone(), serde_json::json!({"track_id": track_id}))
        .await
        .unwrap();

    assert!(
        result.get("fingerprint").is_some(),
        "result should contain fingerprint"
    );

    let track = store.get_track(track_id).await.unwrap().unwrap();
    let fp = track
        .tags
        .get("acoustid_fingerprint")
        .and_then(|v| v.as_str())
        .expect("acoustid_fingerprint should be in track tags");
    assert!(!fp.is_empty(), "fingerprint should not be empty");
}

#[tokio::test]
async fn test_fingerprint_missing_track_id_returns_error() {
    let db = make_db().await;
    let handler = FingerprintJobHandler;
    let result = handler.run(db, serde_json::json!({})).await;
    assert!(result.is_err(), "missing track_id should return an error");
}

#[tokio::test]
async fn test_fingerprint_nonexistent_track_returns_error() {
    let db = make_db().await;
    let handler = FingerprintJobHandler;
    let result = handler
        .run(db, serde_json::json!({"track_id": 99999}))
        .await;
    assert!(
        result.is_err(),
        "nonexistent track should return not-found error"
    );
}

#[tokio::test]
async fn test_update_track_fingerprint_dal() {
    let db = make_db().await;
    let lib = db
        .create_library("Test", "/music", "flac", None)
        .await
        .unwrap();
    let track = db
        .upsert_track(UpsertTrack {
            library_id: lib.id,
            relative_path: "test.flac".into(),
            file_hash: "hash1".into(),
            title: None,
            artist: None,
            albumartist: None,
            album: None,
            tracknumber: None,
            discnumber: None,
            totaldiscs: None,
            totaltracks: None,
            date: None,
            genre: None,
            composer: None,
            label: None,
            catalognumber: None,
            tags: serde_json::json!({"title": "Some Song"}),
            duration_secs: None,
            bitrate: None,
            sample_rate: None,
            channels: None,
            bit_depth: None,
            has_embedded_art: false,
        })
        .await
        .unwrap();

    db.update_track_fingerprint(track.id, "AQAA5kmtest", 45.3)
        .await
        .unwrap();

    let updated = db.get_track(track.id).await.unwrap().unwrap();

    // Check dedicated column
    assert_eq!(
        updated.acoustid_fingerprint.as_deref(),
        Some("AQAA5kmtest"),
        "acoustid_fingerprint column should be set"
    );

    // Check tags JSONB / JSON field
    let fp_tag = updated
        .tags
        .get("acoustid_fingerprint")
        .and_then(|v| v.as_str())
        .expect("acoustid_fingerprint should be in tags JSON");
    assert_eq!(fp_tag, "AQAA5kmtest");

    // Existing tags should be preserved
    assert_eq!(
        updated.tags.get("title").and_then(|v| v.as_str()),
        Some("Some Song"),
        "existing tags should be preserved after fingerprint update"
    );

    // Duration should be updated
    let dur = updated.duration_secs.expect("duration_secs should be set");
    assert!(
        (dur - 45.3).abs() < 0.001,
        "duration_secs should be ~45.3, got {dur}"
    );
}

#[tokio::test]
async fn test_scan_enqueues_fingerprint_jobs() {
    use suzuran_server::scanner::scan_library;
    use tokio::fs;

    let db = make_db().await;
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    // Write two audio files (empty content — scanner will upsert even if tags fail)
    fs::write(root.join("track01.flac"), b"").await.unwrap();
    fs::write(root.join("track02.flac"), b"").await.unwrap();

    let lib = db
        .create_library("Test", root.to_str().unwrap(), "flac", None)
        .await
        .unwrap();

    let result = scan_library(&db, lib.id, &root).await.unwrap();
    assert_eq!(result.inserted, 2, "should insert 2 tracks");

    // Two fingerprint jobs should have been enqueued
    let jobs = db.list_jobs(Some("pending"), 50).await.unwrap();
    let fp_jobs: Vec<_> = jobs.iter().filter(|j| j.job_type == "fingerprint").collect();
    assert_eq!(
        fp_jobs.len(),
        2,
        "should enqueue 2 fingerprint jobs for newly inserted tracks"
    );

    drop(dir);
}
