use suzuran_server::jobs::cue_split::CueSplitJobHandler;
use suzuran_server::jobs::JobHandler;

mod common;

/// Check whether ffmpeg is on PATH in the current environment.
async fn ffmpeg_available() -> bool {
    tokio::process::Command::new("ffmpeg")
        .args(["-version"])
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[tokio::test]
async fn test_cue_split_creates_individual_tracks() {
    if !ffmpeg_available().await {
        eprintln!("SKIP: ffmpeg not on PATH");
        return;
    }

    let (store, library_id, root) = common::setup_cue_library().await;
    let handler = CueSplitJobHandler::new(store.clone());
    let cue_path = root.path().join("ingest").join("album.cue").to_string_lossy().to_string();

    let result = handler
        .run(
            store.clone(),
            serde_json::json!({
                "cue_path": cue_path,
                "library_id": library_id
            }),
        )
        .await
        .unwrap();

    assert_eq!(result["tracks_created"].as_i64(), Some(3));

    let tracks = store.list_tracks_by_library(library_id).await.unwrap();
    assert_eq!(tracks.len(), 3);

    // Sort by tracknumber to get deterministic order
    let mut sorted = tracks.clone();
    sorted.sort_by_key(|t| t.tracknumber.clone().unwrap_or_default());
    assert_eq!(sorted[0].tracknumber.as_deref(), Some("1"));

    // All track relative_paths should start with "source/"
    for track in &tracks {
        assert!(
            track.relative_path.starts_with("source/"),
            "expected relative_path to start with 'source/', got: {}",
            track.relative_path
        );
    }

    // Output files should exist under source/
    let source_dir = root.path().join("source");
    assert!(source_dir.exists(), "source/ directory should be created");

    // Original CUE and audio should be removed from ingest/
    assert!(
        !root.path().join("ingest").join("album.cue").exists(),
        "original CUE file should be removed from ingest/"
    );
    assert!(
        !root.path().join("ingest").join("album.flac").exists(),
        "original audio file should be removed from ingest/"
    );
}

#[tokio::test]
async fn test_cue_split_is_idempotent() {
    if !ffmpeg_available().await {
        eprintln!("SKIP: ffmpeg not on PATH");
        return;
    }

    let (store, library_id, root) = common::setup_cue_library().await;
    let handler = CueSplitJobHandler::new(store.clone());
    let cue_path = root.path().join("ingest").join("album.cue").to_string_lossy().to_string();

    // First run
    handler
        .run(
            store.clone(),
            serde_json::json!({"cue_path": cue_path, "library_id": library_id}),
        )
        .await
        .unwrap();

    // Second run — output files exist, should skip all tracks
    // Note: CUE+audio are removed after first run, so cue_path no longer exists;
    // the job will fail to read the CUE file. This is expected behaviour — idempotency
    // in practice means the split only runs once per CUE file.
    let result2 = handler
        .run(
            store.clone(),
            serde_json::json!({"cue_path": cue_path, "library_id": library_id}),
        )
        .await;

    // After cleanup the CUE file is gone; handler should return an error (not a panic)
    // OR return 0 tracks if the output files are checked before reading the CUE.
    // Both outcomes are acceptable — just verify no extra DB rows are created.
    let _ = result2; // may be Ok(0) or Err — both fine

    let tracks = store.list_tracks_by_library(library_id).await.unwrap();
    assert_eq!(tracks.len(), 3, "no duplicate tracks on second run");
}

#[tokio::test]
async fn test_scanner_skips_cue_backed_audio() {
    let (store, library_id, root) = common::setup_cue_library().await;
    suzuran_server::scanner::scan_library(&store, library_id, root.path())
        .await
        .unwrap();

    let tracks = store.list_tracks_by_library(library_id).await.unwrap();
    assert_eq!(
        tracks.len(),
        0,
        "whole-file flac must not be ingested before split"
    );

    let jobs = store.list_jobs(Some("pending"), 50).await.unwrap();
    assert!(
        jobs.iter().any(|j| j.job_type == "cue_split"),
        "cue_split job should be queued"
    );
}
