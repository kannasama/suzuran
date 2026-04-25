mod common;

use std::sync::Arc;
use suzuran_server::{
    dal::{Store, UpsertTagSuggestion},
    services::tagging::apply_suggestion,
    tagger,
};

/// Helper: create a tag suggestion for the given track with `suggested_tags`.
async fn create_suggestion(
    store: &Arc<dyn Store>,
    track_id: i64,
    suggested_tags: serde_json::Value,
) -> suzuran_server::models::TagSuggestion {
    store
        .create_tag_suggestion(UpsertTagSuggestion {
            track_id,
            source: "test".to_string(),
            suggested_tags,
            confidence: 1.0,
            mb_recording_id: None,
            mb_release_id: None,
            cover_art_url: None,
            alternatives: None,
        })
        .await
        .unwrap()
}

/// `apply_suggestion` writes updated tags to the audio file and updates the DB.
#[tokio::test]
async fn test_apply_suggestion_updates_file_and_db() {
    let (store, track_id, _dir) = common::setup_with_audio_track().await;

    // Build a suggestion that overrides the artist and adds a new genre
    let suggestion = create_suggestion(
        &store,
        track_id,
        serde_json::json!({
            "artist": "Suggested Artist",
            "genre": "Electronic"
        }),
    )
    .await;

    // Apply the suggestion
    apply_suggestion(&store, &suggestion, None, true).await.unwrap();

    // --- Verify DB ---
    let updated_track = store.get_track(track_id).await.unwrap().unwrap();

    assert_eq!(
        updated_track.artist.as_deref(),
        Some("Suggested Artist"),
        "indexed artist column should reflect the suggestion"
    );

    let tags_artist = updated_track
        .tags
        .get("artist")
        .and_then(|v| v.as_str())
        .expect("tags JSON should contain artist");
    assert_eq!(tags_artist, "Suggested Artist");

    let tags_genre = updated_track
        .tags
        .get("genre")
        .and_then(|v| v.as_str())
        .expect("tags JSON should contain genre");
    assert_eq!(tags_genre, "Electronic");

    // Original tags should be preserved after merge
    let tags_title = updated_track
        .tags
        .get("title")
        .and_then(|v| v.as_str())
        .expect("tags JSON should still contain title");
    assert_eq!(tags_title, "Tag Test Song");

    // --- Verify file on disk ---
    let dir_path = _dir.path().to_path_buf();
    let audio_path = dir_path.join("test_track.flac");

    let (file_tags, _props) = tagger::read_tags(&audio_path)
        .expect("read_tags should succeed on the updated file");

    assert_eq!(
        file_tags.get("artist").map(String::as_str),
        Some("Suggested Artist"),
        "audio file artist tag should be updated"
    );
    assert_eq!(
        file_tags.get("genre").map(String::as_str),
        Some("Electronic"),
        "audio file genre tag should be written"
    );
    assert_eq!(
        file_tags.get("title").map(String::as_str),
        Some("Tag Test Song"),
        "audio file title tag should be preserved from merge"
    );
}

/// `apply_suggestion` returns NotFound when the track does not exist.
#[tokio::test]
async fn test_apply_suggestion_missing_track_returns_error() {
    let (store, track_id, _dir) = common::setup_with_audio_track().await;

    // Create suggestion pointing at a nonexistent track
    let mut suggestion = create_suggestion(
        &store,
        track_id,
        serde_json::json!({"artist": "Ghost"}),
    )
    .await;
    suggestion.track_id = 99999; // override to nonexistent id

    let result = apply_suggestion(&store, &suggestion, None, true).await;
    assert!(result.is_err(), "should fail with NotFound for missing track");
}
