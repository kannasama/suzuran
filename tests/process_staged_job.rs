mod common;

use suzuran_server::{
    dal::{Store, UpsertTrack, UpsertLibraryProfile},
    jobs::{process_staged::ProcessStagedJobHandler, JobHandler, ProcessStagedPayload},
};
use tempfile::TempDir;

use common::TAGGED_FLAC;

async fn make_db() -> std::sync::Arc<dyn Store> {
    let store = suzuran_server::dal::sqlite::SqliteStore::new("sqlite::memory:")
        .await
        .unwrap();
    store.migrate().await.unwrap();
    std::sync::Arc::new(store)
}

/// Create an ingest/ and source/ directory structure under `root`.
async fn create_library_dirs(root: &std::path::Path) {
    tokio::fs::create_dir_all(root.join("ingest")).await.unwrap();
    tokio::fs::create_dir_all(root.join("source")).await.unwrap();
}

#[tokio::test]
async fn test_process_staged_moves_file_to_source() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    // Setup directories
    let ingest_subdir = root.join("ingest").join("album");
    tokio::fs::create_dir_all(&ingest_subdir).await.unwrap();
    tokio::fs::create_dir_all(root.join("source")).await.unwrap();

    // Write a real FLAC file to ingest/album/test.flac
    let flac_path = ingest_subdir.join("test.flac");
    tokio::fs::write(&flac_path, TAGGED_FLAC).await.unwrap();

    let store = make_db().await;

    // Create library
    let lib = store
        .create_library("Test", root.to_str().unwrap(), "flac")
        .await
        .unwrap();

    // Create staged track record
    let track = store
        .upsert_track(UpsertTrack {
            library_id: lib.id,
            relative_path: "ingest/album/test.flac".into(),
            file_hash: "staged_hash_001".into(),
            title: Some("Test Song".into()),
            artist: Some("Test Artist".into()),
            tags: serde_json::json!({}),
            status: "staged".into(),
            ..UpsertTrack::default()
        })
        .await
        .unwrap();

    // Create encoding profile and library profile
    let enc_profile = store
        .create_encoding_profile(suzuran_server::dal::UpsertEncodingProfile {
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

    let lib_profile = store
        .create_library_profile(&UpsertLibraryProfile {
            library_id: lib.id,
            encoding_profile_id: enc_profile.id,
            derived_dir_name: "aac".into(),
            include_on_submit: false,
            auto_include_above_hz: None,
        })
        .await
        .unwrap();

    // Run the handler
    let payload = serde_json::to_value(ProcessStagedPayload {
        track_id: track.id,
        profile_ids: vec![lib_profile.id],
        ..Default::default()
    })
    .unwrap();

    let handler = ProcessStagedJobHandler::new(store.clone());
    let result = handler.run(store.clone(), payload).await.unwrap();

    assert_eq!(result["track_id"].as_i64(), Some(track.id));
    assert_eq!(result["profiles_enqueued"].as_i64(), Some(1));

    // File should now exist at source/album/test.flac
    let dest = root.join("source").join("album").join("test.flac");
    assert!(dest.exists(), "file should exist at source/album/test.flac");

    // File should NOT be at ingest/album/test.flac
    assert!(!flac_path.exists(), "file should be removed from ingest/album/test.flac");

    // Track DB record should be updated
    let updated = store.get_track(track.id).await.unwrap().unwrap();
    assert_eq!(updated.status, "active", "track status should be active");
    assert_eq!(
        updated.relative_path,
        "source/album/test.flac",
        "track relative_path should be source/album/test.flac"
    );

    // One transcode job should be enqueued
    let jobs = store.list_jobs(Some("pending"), 50).await.unwrap();
    let transcode_jobs: Vec<_> = jobs.iter().filter(|j| j.job_type == "transcode").collect();
    assert_eq!(transcode_jobs.len(), 1, "should enqueue 1 transcode job");
    assert_eq!(
        transcode_jobs[0].payload["library_profile_id"].as_i64(),
        Some(lib_profile.id),
        "transcode job should use lib_profile.id"
    );
    assert_eq!(
        transcode_jobs[0].payload["track_id"].as_i64(),
        Some(track.id),
        "transcode job should use the track id"
    );

    drop(dir);
}

#[tokio::test]
async fn test_process_staged_missing_track_returns_error() {
    let store = make_db().await;

    let payload = serde_json::to_value(ProcessStagedPayload {
        track_id: 99999,
        ..Default::default()
    })
    .unwrap();

    let handler = ProcessStagedJobHandler::new(store.clone());
    let result = handler.run(store.clone(), payload).await;
    assert!(result.is_err(), "non-existent track should return an error");
}

#[tokio::test]
async fn test_process_staged_already_active_returns_error() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_path_buf();
    tokio::fs::create_dir_all(root.join("source")).await.unwrap();
    tokio::fs::write(root.join("source").join("track.flac"), TAGGED_FLAC).await.unwrap();

    let store = make_db().await;
    let lib = store
        .create_library("Test", root.to_str().unwrap(), "flac")
        .await
        .unwrap();

    let track = store
        .upsert_track(UpsertTrack {
            library_id: lib.id,
            relative_path: "source/track.flac".into(),
            file_hash: "active_hash".into(),
            tags: serde_json::json!({}),
            status: "active".into(), // already active
            ..UpsertTrack::default()
        })
        .await
        .unwrap();

    let payload = serde_json::to_value(ProcessStagedPayload {
        track_id: track.id,
        ..Default::default()
    })
    .unwrap();

    let handler = ProcessStagedJobHandler::new(store.clone());
    let result = handler.run(store.clone(), payload).await;
    assert!(
        result.is_err(),
        "active track should return error (expected staged)"
    );

    drop(dir);
}
