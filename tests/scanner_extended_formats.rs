use std::{path::PathBuf, sync::Arc};
use tokio::fs;
use suzuran_server::{
    dal::{sqlite::SqliteStore, Store},
    scanner::scan_library,
};

async fn make_db() -> Arc<dyn Store> {
    let store = SqliteStore::new("sqlite::memory:").await.unwrap();
    store.migrate().await.unwrap();
    Arc::new(store)
}

/// Returns a temp dir containing a single file with the given filename (empty content).
/// The scanner accepts empty files — tag read errors fall back to an empty tag map.
async fn make_temp_library_with_file(filename: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();
    fs::write(root.join(filename), b"").await.unwrap();
    (dir, root)
}

#[tokio::test]
async fn test_wavpack_file_ingested() {
    let db = make_db().await;
    let (dir, root) = make_temp_library_with_file("silence.wv").await;

    let lib = db.create_library("Test", root.to_str().unwrap(), "wv", None).await.unwrap();
    scan_library(&db, lib.id, &root).await.unwrap();

    let tracks = db.list_tracks_by_library(lib.id).await.unwrap();
    assert_eq!(tracks.len(), 1, "WavPack file should be ingested");
    drop(dir);
}

#[tokio::test]
async fn test_ape_file_ingested() {
    let db = make_db().await;
    let (dir, root) = make_temp_library_with_file("silence.ape").await;

    let lib = db.create_library("Test", root.to_str().unwrap(), "ape", None).await.unwrap();
    scan_library(&db, lib.id, &root).await.unwrap();

    let tracks = db.list_tracks_by_library(lib.id).await.unwrap();
    assert_eq!(tracks.len(), 1, "APE file should be ingested");
    drop(dir);
}

#[tokio::test]
async fn test_tta_file_ingested() {
    let db = make_db().await;
    let (dir, root) = make_temp_library_with_file("silence.tta").await;

    let lib = db.create_library("Test", root.to_str().unwrap(), "tta", None).await.unwrap();
    scan_library(&db, lib.id, &root).await.unwrap();

    let tracks = db.list_tracks_by_library(lib.id).await.unwrap();
    assert_eq!(tracks.len(), 1, "TrueAudio file should be ingested");
    drop(dir);
}
