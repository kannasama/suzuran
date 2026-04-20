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
async fn test_transcode_fails_without_encoding_profile() {
    let (store, source_track_id, target_library_id) =
        common::setup_transcode_scenario_no_profile().await;

    let handler = TranscodeJobHandler::new(store.clone());
    let result = handler
        .run(
            store.clone(),
            serde_json::json!({
                "source_track_id": source_track_id,
                "target_library_id": target_library_id,
            }),
        )
        .await;

    assert!(
        result.is_err(),
        "expected Err when target library has no encoding_profile_id"
    );
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("encoding_profile_id"),
        "error message should mention encoding_profile_id, got: {err_str}"
    );
}

#[tokio::test]
async fn test_transcode_skips_lossy_to_lossless() {
    let (store, source_track_id, target_library_id) =
        common::setup_transcode_lossy_to_lossless_scenario().await;

    let handler = TranscodeJobHandler::new(store.clone());
    let result = handler
        .run(
            store.clone(),
            serde_json::json!({
                "source_track_id": source_track_id,
                "target_library_id": target_library_id,
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
        result["source_track_id"].as_i64(),
        Some(source_track_id),
        "source_track_id should be echoed in skip result"
    );
}

#[tokio::test]
async fn test_transcode_missing_source_track() {
    let store = common::setup_store().await;

    // Create a target library with an encoding profile
    use suzuran_server::dal::{Store, UpsertEncodingProfile};
    let tgt_lib = store
        .create_library("Target", "/music/target", "aac", None)
        .await
        .unwrap();
    let profile = store
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
        .set_library_encoding_profile(tgt_lib.id, Some(profile.id))
        .await
        .unwrap();

    let handler = TranscodeJobHandler::new(store.clone());
    let result = handler
        .run(
            store.clone(),
            serde_json::json!({
                "source_track_id": 99999,
                "target_library_id": tgt_lib.id,
            }),
        )
        .await;

    assert!(
        result.is_err(),
        "expected Err when source track does not exist"
    );
}
