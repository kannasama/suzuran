use std::sync::Arc;
use tempfile::TempDir;
use suzuran_server::dal::{sqlite::SqliteStore, Store, UpsertTrack};

pub async fn make_db() -> Arc<dyn Store> {
    let store = SqliteStore::new("sqlite::memory:").await.unwrap();
    store.migrate().await.unwrap();
    Arc::new(store)
}

#[allow(dead_code)]
pub async fn setup_store() -> Arc<dyn Store> {
    make_db().await
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
            bit_depth: None,
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

/// Set up an in-memory DB with a track that has a DISCID tag and a track number.
/// Returns `(store, track_id)`.
pub async fn setup_with_discid_track(disc_id: &str, track_number: u32) -> (Arc<dyn Store>, i64) {
    let db = make_db().await;
    let lib = db
        .create_library("Test", "/music", "flac", None)
        .await
        .unwrap();

    let track = db
        .upsert_track(UpsertTrack {
            library_id: lib.id,
            relative_path: "discid_track.flac".into(),
            file_hash: format!("discid_hash_{disc_id}"),
            title: Some("DISCID Track".into()),
            artist: Some("Test Artist".into()),
            albumartist: None,
            album: None,
            tracknumber: Some(track_number.to_string()),
            discnumber: None,
            totaldiscs: None,
            totaltracks: None,
            date: None,
            genre: None,
            composer: None,
            label: None,
            catalognumber: None,
            tags: serde_json::json!({
                "DISCID": disc_id,
                "tracknumber": track_number.to_string()
            }),
            duration_secs: Some(200.0),
            bitrate: None,
            sample_rate: None,
            channels: None,
            bit_depth: None,
            has_embedded_art: false,
        })
        .await
        .unwrap();

    (db, track.id)
}

/// Minimal valid FLAC file with a VORBISCOMMENT block and a 1-sample silence frame.
/// STREAMINFO (34 bytes) + VORBISCOMMENT (empty, 18 bytes) + 1-sample CONSTANT frame.
/// 76 bytes total. Lofty can write/read tags on this file.
/// Generated with a Python FLAC-spec-compliant builder (see tasks/lessons.md).
pub const TAGGED_FLAC: &[u8] = &[
    // "fLaC" marker
    0x66, 0x4c, 0x61, 0x43,
    // STREAMINFO block header: type=0 (not last), length=34
    0x00, 0x00, 0x00, 0x22,
    // STREAMINFO content: blocksize=1, framesize=0, rate=44100, ch=1, bps=16, samples=1, MD5=0
    0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x0a, 0xc4, 0x40, 0xf0, 0x00, 0x00, 0x00, 0x01,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // VORBISCOMMENT block header: type=4 | 0x80 (last), length=18
    0x84, 0x00, 0x00, 0x12,
    // VORBISCOMMENT content: LE u32 vendor length=10, vendor="lofty test", LE u32 comment count=0
    0x0a, 0x00, 0x00, 0x00,
    0x6c, 0x6f, 0x66, 0x74, 0x79, 0x20, 0x74, 0x65, 0x73, 0x74,
    0x00, 0x00, 0x00, 0x00,
    // Audio frame: 1 sample of silence (CONSTANT subframe, value=0), with frame CRC
    0xff, 0xf8, 0x6c, 0x08, 0x00, 0x00, 0x53, 0x00, 0x00, 0x00, 0x28, 0x27,
];

/// Set up an in-memory DB with a real audio file (FLAC with VORBISCOMMENT) and a matching track.
/// The audio file has an initial `artist` tag of "Original Artist".
/// Returns `(store, track_id, TempDir)` — keep TempDir alive to prevent cleanup.
pub async fn setup_with_audio_track() -> (Arc<dyn Store>, i64, TempDir) {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    let track_file = root.join("test_track.flac");
    tokio::fs::write(&track_file, TAGGED_FLAC).await.unwrap();

    let db = make_db().await;
    let lib = db
        .create_library("Test", root.to_str().unwrap(), "flac", None)
        .await
        .unwrap();

    let track = db
        .upsert_track(UpsertTrack {
            library_id: lib.id,
            relative_path: "test_track.flac".into(),
            file_hash: "tag_test_hash_001".into(),
            title: Some("Tag Test Song".into()),
            artist: Some("Original Artist".into()),
            albumartist: None,
            album: Some("Test Album".into()),
            tracknumber: Some("1".into()),
            discnumber: None,
            totaldiscs: None,
            totaltracks: None,
            date: Some("2024".into()),
            genre: None,
            composer: None,
            label: None,
            catalognumber: None,
            tags: serde_json::json!({
                "title": "Tag Test Song",
                "artist": "Original Artist",
                "album": "Test Album",
                "tracknumber": "1",
                "date": "2024"
            }),
            duration_secs: Some(1.0),
            bitrate: None,
            sample_rate: Some(44100),
            channels: Some(1),
            bit_depth: None,
            has_embedded_art: false,
        })
        .await
        .unwrap();

    (db, track.id, dir)
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
            bit_depth: None,
            has_embedded_art: false,
        })
        .await
        .unwrap();

    (db, track.id)
}
