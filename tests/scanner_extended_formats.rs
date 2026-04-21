use std::{path::PathBuf, sync::Arc};
use tokio::fs;
use suzuran_server::{
    dal::{sqlite::SqliteStore, Store},
    scanner::scan_library,
};

/// 1-second mono 44100 Hz WavPack silence — generated with ffmpeg lavfi anullsrc.
/// Ensures lofty can actually parse WavPack headers, not just that the extension filter works.
static SILENCE_WV: &[u8] = include_bytes!("fixtures/silence.wv");

/// 1-second mono 44100 Hz Monkey's Audio silence — generated with mac (Monkey's Audio v12.62).
/// Ensures lofty can actually parse APE headers, not just that the extension filter works.
static SILENCE_APE: &[u8] = include_bytes!("fixtures/silence.ape");

/// 1-second mono 44100 Hz TrueAudio silence — generated with ffmpeg lavfi anullsrc.
/// Ensures lofty can actually parse TTA headers, not just that the extension filter works.
static SILENCE_TTA: &[u8] = include_bytes!("fixtures/silence.tta");

async fn make_db() -> Arc<dyn Store> {
    let store = SqliteStore::new("sqlite::memory:").await.unwrap();
    store.migrate().await.unwrap();
    Arc::new(store)
}

/// Returns a temp dir containing a single file with the given filename and real audio bytes.
/// Writes real fixture bytes so lofty actually parses the file headers.
async fn make_temp_library_with_file(filename: &str, content: &[u8]) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();
    fs::write(root.join(filename), content).await.unwrap();
    (dir, root)
}

#[tokio::test]
async fn test_wavpack_file_ingested() {
    let db = make_db().await;
    let (dir, root) = make_temp_library_with_file("silence.wv", SILENCE_WV).await;

    let lib = db.create_library("Test", root.to_str().unwrap(), "wv").await.unwrap();
    scan_library(&db, lib.id, &root).await.unwrap();

    let tracks = db.list_tracks_by_library(lib.id).await.unwrap();
    assert_eq!(tracks.len(), 1, "WavPack file should be ingested");
    drop(dir);
}

#[tokio::test]
async fn test_ape_file_ingested() {
    let db = make_db().await;
    let (dir, root) = make_temp_library_with_file("silence.ape", SILENCE_APE).await;

    let lib = db.create_library("Test", root.to_str().unwrap(), "ape").await.unwrap();
    scan_library(&db, lib.id, &root).await.unwrap();

    let tracks = db.list_tracks_by_library(lib.id).await.unwrap();
    assert_eq!(tracks.len(), 1, "APE file should be ingested");
    drop(dir);
}

#[tokio::test]
async fn test_tta_file_ingested() {
    let db = make_db().await;
    let (dir, root) = make_temp_library_with_file("silence.tta", SILENCE_TTA).await;

    let lib = db.create_library("Test", root.to_str().unwrap(), "tta").await.unwrap();
    scan_library(&db, lib.id, &root).await.unwrap();

    let tracks = db.list_tracks_by_library(lib.id).await.unwrap();
    assert_eq!(tracks.len(), 1, "TrueAudio file should be ingested");
    drop(dir);
}
