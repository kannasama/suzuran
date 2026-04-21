mod common;

use suzuran_server::jobs::transcode::TranscodeJobHandler;
use suzuran_server::jobs::JobHandler;

// ── unit helpers ─────────────────────────────────────────────────────────────

#[test]
fn test_codec_extension_known() {
    use suzuran_server::jobs::transcode::codec_extension;
    assert_eq!(codec_extension("aac"), "m4a");
    assert_eq!(codec_extension("mp3"), "mp3");
    assert_eq!(codec_extension("libmp3lame"), "mp3");
    assert_eq!(codec_extension("opus"), "opus");
    assert_eq!(codec_extension("libopus"), "opus");
    assert_eq!(codec_extension("flac"), "flac");
    assert_eq!(codec_extension("vorbis"), "ogg");
    assert_eq!(codec_extension("libvorbis"), "ogg");
}

#[test]
fn test_codec_extension_unknown_passthrough() {
    use suzuran_server::jobs::transcode::codec_extension;
    assert_eq!(codec_extension("alac"), "alac");
    assert_eq!(codec_extension("pcm_s16le"), "pcm_s16le");
}

// ── integration tests ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_transcode_fails_without_library_profile() {
    // Pass a non-existent library_profile_id — should return Err (not found)
    let store = common::setup_store().await;
    use suzuran_server::dal::Store;
    let src_lib = store
        .create_library("Source", "/music/source", "flac")
        .await
        .unwrap();
    let track = store
        .upsert_track(suzuran_server::dal::UpsertTrack {
            library_id: src_lib.id,
            relative_path: "source/artist/album/01.flac".into(),
            file_hash: "abc".into(),
            sample_rate: Some(44100),
            bit_depth: Some(16),
            bitrate: Some(1000),
            tags: serde_json::json!({}),
            ..suzuran_server::dal::UpsertTrack::default()
        })
        .await
        .unwrap();

    let handler = TranscodeJobHandler::new(store.clone());
    let result = handler
        .run(
            store.clone(),
            serde_json::json!({
                "track_id": track.id,
                "library_profile_id": 99999,
            }),
        )
        .await;

    assert!(
        result.is_err(),
        "expected Err when library_profile_id does not exist"
    );
}

#[tokio::test]
async fn test_transcode_skips_lossy_to_lossless() {
    let (store, source_track_id, library_profile_id) =
        common::setup_transcode_lossy_to_lossless_scenario().await;

    let handler = TranscodeJobHandler::new(store.clone());
    let result = handler
        .run(
            store.clone(),
            serde_json::json!({
                "track_id": source_track_id,
                "library_profile_id": library_profile_id,
            }),
        )
        .await
        .expect("handler should not return Err for a skip scenario");

    assert_eq!(
        result["status"].as_str(),
        Some("skipped"),
        "expected status=skipped for AAC → FLAC transcode, got: {result}"
    );
    assert_eq!(
        result["track_id"].as_i64(),
        Some(source_track_id),
        "track_id should be echoed in skip result"
    );
}

#[tokio::test]
async fn test_transcode_missing_source_track() {
    let store = common::setup_store().await;

    let handler = TranscodeJobHandler::new(store.clone());
    let result = handler
        .run(
            store.clone(),
            serde_json::json!({
                "track_id": 99999,
                "library_profile_id": 1,
            }),
        )
        .await;

    assert!(
        result.is_err(),
        "expected Err when source track does not exist"
    );
}
