use std::sync::Arc;
use suzuran_server::dal::{sqlite::SqliteStore, Store, UpsertTrack};

pub async fn make_db() -> Arc<dyn Store> {
    let store = SqliteStore::new("sqlite::memory:").await.unwrap();
    store.migrate().await.unwrap();
    Arc::new(store)
}

/// Set up an in-memory DB with a track that has an AcoustID fingerprint in
/// both `acoustid_fingerprint` column and `tags` JSON.
/// Returns `(store, track_id)`.
pub async fn setup_with_fingerprinted_track() -> (Arc<dyn Store>, i64) {
    let db = make_db().await;
    let lib = db
        .create_library("Test", "/music", "flac", None)
        .await
        .unwrap();

    let track = db
        .upsert_track(UpsertTrack {
            library_id: lib.id,
            relative_path: "test_track.flac".into(),
            file_hash: "fp_hash_001".into(),
            title: Some("Test Song".into()),
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
            tags: serde_json::json!({"acoustid_fingerprint": "AQADtNmybFIAAA"}),
            duration_secs: Some(210.0),
            bitrate: None,
            sample_rate: None,
            channels: None,
            has_embedded_art: false,
        })
        .await
        .unwrap();

    // Also write the fingerprint to the dedicated column via DAL.
    db.update_track_fingerprint(track.id, "AQADtNmybFIAAA", 210.0)
        .await
        .unwrap();

    (db, track.id)
}

/// Set up an in-memory DB with a plain track (no fingerprint).
/// Returns `(store, track_id)`.
pub async fn setup_with_track() -> (Arc<dyn Store>, i64) {
    let db = make_db().await;
    let lib = db
        .create_library("Test", "/music", "flac", None)
        .await
        .unwrap();

    let track = db
        .upsert_track(UpsertTrack {
            library_id: lib.id,
            relative_path: "no_fp_track.flac".into(),
            file_hash: "no_fp_hash_001".into(),
            title: Some("No Fingerprint".into()),
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
            tags: serde_json::json!({}),
            duration_secs: Some(180.0),
            bitrate: None,
            sample_rate: None,
            channels: None,
            has_embedded_art: false,
        })
        .await
        .unwrap();

    (db, track.id)
}
