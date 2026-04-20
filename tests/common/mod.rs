use std::sync::Arc;
use tempfile::TempDir;
use suzuran_server::dal::{sqlite::SqliteStore, Store, UpsertEncodingProfile, UpsertTrack};

/// Returns true if `ffmpeg` is available on PATH.
#[allow(dead_code)]
pub fn ffmpeg_available() -> bool {
    std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

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

/// Set up an in-memory DB and a temp directory with a 3-track CUE sheet pointing
/// to a minimal FLAC file (`album.flac`). The CUE timestamps are chosen so that
/// all three tracks fall within the (very short) file duration:
///   Track 1: 00:00:00 (0 s)
///   Track 2: 00:00:01 (1 s) — ffmpeg -c:a copy will produce a zero/near-zero length segment
///   Track 3: 00:00:02 (2 s)
/// Returns `(store, library_id, TempDir)` — keep TempDir alive.
pub async fn setup_cue_library() -> (Arc<dyn Store>, i64, TempDir) {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    // Write the FLAC source file
    let flac_path = root.join("album.flac");
    tokio::fs::write(&flac_path, TAGGED_FLAC).await.unwrap();

    // Write the CUE sheet
    let cue_content = r#"TITLE "Test Album"
PERFORMER "Test Artist"
REM DATE 2024
REM GENRE Rock
FILE "album.flac" WAVE

  TRACK 01 AUDIO
    TITLE "Track One"
    PERFORMER "Test Artist"
    INDEX 01 00:00:00

  TRACK 02 AUDIO
    TITLE "Track Two"
    PERFORMER "Test Artist"
    INDEX 01 00:00:01

  TRACK 03 AUDIO
    TITLE "Track Three"
    PERFORMER "Test Artist"
    INDEX 01 00:00:02
"#;
    let cue_path = root.join("album.cue");
    tokio::fs::write(&cue_path, cue_content).await.unwrap();

    let db = make_db().await;
    let lib = db
        .create_library("CUE Test", root.to_str().unwrap(), "flac", None)
        .await
        .unwrap();

    (db, lib.id, dir)
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

/// Set up an in-memory DB with:
/// - A source library (FLAC) containing one AAC track with no encoding profile
/// - A target library (aac) with no encoding_profile_id
/// Returns `(store, source_track_id, target_library_id)`.
#[allow(dead_code)]
pub async fn setup_transcode_scenario_no_profile() -> (Arc<dyn Store>, i64, i64) {
    let db = make_db().await;

    let src_lib = db
        .create_library("Source", "/music/source", "flac", None)
        .await
        .unwrap();

    let track = db
        .upsert_track(UpsertTrack {
            library_id: src_lib.id,
            relative_path: "artist/album/01 - Song.flac".into(),
            file_hash: "transcode_src_hash_001".into(),
            title: Some("Song".into()),
            artist: Some("Artist".into()),
            sample_rate: Some(44100),
            bit_depth: Some(16),
            bitrate: Some(1000),
            tags: serde_json::json!({}),
            ..UpsertTrack::default()
        })
        .await
        .unwrap();

    let tgt_lib = db
        .create_library("Target AAC", "/music/target", "aac", None)
        .await
        .unwrap();
    // target library intentionally has no encoding_profile_id

    (db, track.id, tgt_lib.id)
}

/// Set up an in-memory DB with:
/// - A source library containing one AAC track (lossy)
/// - A target library with a FLAC encoding profile (lossless)
/// This scenario should be skipped by the transcode job (lossy → lossless guard).
/// Returns `(store, source_track_id, target_library_id)`.
#[allow(dead_code)]
pub async fn setup_transcode_lossy_to_lossless_scenario() -> (Arc<dyn Store>, i64, i64) {
    let db = make_db().await;

    let src_lib = db
        .create_library("Source AAC", "/music/source_aac", "aac", None)
        .await
        .unwrap();

    let track = db
        .upsert_track(UpsertTrack {
            library_id: src_lib.id,
            relative_path: "01 - Song.aac".into(),
            file_hash: "transcode_aac_hash_001".into(),
            title: Some("Song".into()),
            artist: Some("Artist".into()),
            sample_rate: Some(44100),
            bit_depth: None,
            bitrate: Some(256),
            tags: serde_json::json!({}),
            ..UpsertTrack::default()
        })
        .await
        .unwrap();

    let tgt_lib = db
        .create_library("Target FLAC", "/music/target_flac", "flac", None)
        .await
        .unwrap();

    // Create a FLAC encoding profile and attach to target library
    let flac_profile = db
        .create_encoding_profile(UpsertEncodingProfile {
            name: "FLAC Lossless".into(),
            codec: "flac".into(),
            bitrate: None,
            sample_rate: None,
            channels: None,
            bit_depth: None,
            advanced_args: None,
        })
        .await
        .unwrap();

    db.set_library_encoding_profile(tgt_lib.id, Some(flac_profile.id))
        .await
        .unwrap();

    (db, track.id, tgt_lib.id)
}
