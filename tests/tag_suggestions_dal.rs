use std::sync::Arc;
use suzuran_server::dal::{sqlite::SqliteStore, Store, UpsertTagSuggestion};

async fn make_db() -> Arc<dyn Store> {
    let store = SqliteStore::new("sqlite::memory:").await.unwrap();
    store.migrate().await.unwrap();
    Arc::new(store)
}

async fn make_db_with_track() -> (Arc<dyn Store>, i64) {
    let db = make_db().await;
    let lib = db.create_library("Test", "/music", "flac", None).await.unwrap();
    let track = db.upsert_track(suzuran_server::dal::UpsertTrack {
        library_id: lib.id,
        relative_path: "test/track01.flac".into(),
        file_hash: "abc123".into(),
        title: Some("Test Title".into()),
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
        has_embedded_art: false,
    }).await.unwrap();
    (db, track.id)
}

#[tokio::test]
async fn test_create_and_list_pending() {
    let (store, track_id) = make_db_with_track().await;

    let dto = UpsertTagSuggestion {
        track_id,
        source: "acoustid".into(),
        suggested_tags: serde_json::json!({"title": "Test Title", "artist": "Test Artist"}),
        confidence: 0.92,
        mb_recording_id: Some("rec-uuid".into()),
        mb_release_id: Some("rel-uuid".into()),
        cover_art_url: None,
    };
    let s = store.create_tag_suggestion(dto).await.unwrap();
    assert_eq!(s.status, "pending");
    assert_eq!(s.source, "acoustid");
    assert!((s.confidence - 0.92).abs() < 0.001);

    let pending = store.list_pending_tag_suggestions(None).await.unwrap();
    assert_eq!(pending.len(), 1);

    store.set_tag_suggestion_status(s.id, "accepted").await.unwrap();
    let pending2 = store.list_pending_tag_suggestions(None).await.unwrap();
    assert_eq!(pending2.len(), 0);

    let count = store.pending_tag_suggestion_count().await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_filter_by_track_id() {
    let (store, track_id) = make_db_with_track().await;

    store.create_tag_suggestion(UpsertTagSuggestion {
        track_id,
        source: "mb_search".into(),
        suggested_tags: serde_json::json!({}),
        confidence: 0.7,
        mb_recording_id: None,
        mb_release_id: None,
        cover_art_url: None,
    }).await.unwrap();

    let filtered = store.list_pending_tag_suggestions(Some(track_id)).await.unwrap();
    assert_eq!(filtered.len(), 1);

    let wrong_id = store.list_pending_tag_suggestions(Some(track_id + 999)).await.unwrap();
    assert_eq!(wrong_id.len(), 0);
}

#[tokio::test]
async fn test_get_tag_suggestion() {
    let (store, track_id) = make_db_with_track().await;

    let s = store.create_tag_suggestion(UpsertTagSuggestion {
        track_id,
        source: "acoustid".into(),
        suggested_tags: serde_json::json!({"title": "Get Test"}),
        confidence: 0.85,
        mb_recording_id: None,
        mb_release_id: None,
        cover_art_url: Some("https://example.com/art.jpg".into()),
    }).await.unwrap();

    let fetched = store.get_tag_suggestion(s.id).await.unwrap()
        .expect("tag suggestion should exist");
    assert_eq!(fetched.id, s.id);
    assert_eq!(fetched.source, "acoustid");
    assert_eq!(fetched.cover_art_url, Some("https://example.com/art.jpg".into()));
}
