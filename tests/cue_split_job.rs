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
    let cue_path = root.path().join("album.cue").to_string_lossy().to_string();

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
}

#[tokio::test]
async fn test_cue_split_is_idempotent() {
    if !ffmpeg_available().await {
        eprintln!("SKIP: ffmpeg not on PATH");
        return;
    }

    let (store, library_id, root) = common::setup_cue_library().await;
    let handler = CueSplitJobHandler::new(store.clone());
    let cue_path = root.path().join("album.cue").to_string_lossy().to_string();

    // First run
    handler
        .run(
            store.clone(),
            serde_json::json!({"cue_path": cue_path, "library_id": library_id}),
        )
        .await
        .unwrap();

    // Second run — output files exist, should skip all tracks
    let result2 = handler
        .run(
            store.clone(),
            serde_json::json!({"cue_path": cue_path, "library_id": library_id}),
        )
        .await
        .unwrap();

    assert_eq!(result2["tracks_created"].as_i64(), Some(0));

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
