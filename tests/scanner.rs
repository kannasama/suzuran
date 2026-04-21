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

/// Create a temp dir with two fake .flac files (empty — lofty will error on tags
/// but the scanner still upserts the track with an empty tag map).
async fn make_temp_library() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().to_path_buf();
    fs::write(path.join("track01.flac"), b"").await.unwrap();
    fs::write(path.join("track02.flac"), b"").await.unwrap();
    (dir, path)
}

#[tokio::test]
async fn scanner_inserts_new_files() {
    let db = make_db().await;
    let (dir, root) = make_temp_library().await;

    let lib = db.create_library("Test", root.to_str().unwrap(), "flac").await.unwrap();

    let result = scan_library(&db, lib.id, &root).await.unwrap();
    assert_eq!(result.inserted, 2, "should insert 2 files");
    assert_eq!(result.removed, 0);

    let tracks = db.list_tracks_by_library(lib.id).await.unwrap();
    assert_eq!(tracks.len(), 2);
    drop(dir);
}

#[tokio::test]
async fn scanner_removes_deleted_files() {
    let db = make_db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    fs::write(root.join("track01.flac"), b"").await.unwrap();
    fs::write(root.join("track02.flac"), b"").await.unwrap();

    let lib = db.create_library("Test", root.to_str().unwrap(), "flac").await.unwrap();
    scan_library(&db, lib.id, &root).await.unwrap();

    // Remove one file
    fs::remove_file(root.join("track02.flac")).await.unwrap();

    let result = scan_library(&db, lib.id, &root).await.unwrap();
    assert_eq!(result.removed, 1);

    let tracks = db.list_tracks_by_library(lib.id).await.unwrap();
    assert_eq!(tracks.len(), 1);
    drop(dir);
}

#[tokio::test]
async fn scanner_skips_unchanged_files() {
    let db = make_db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    fs::write(root.join("track01.flac"), b"data").await.unwrap();

    let lib = db.create_library("Test", root.to_str().unwrap(), "flac").await.unwrap();
    scan_library(&db, lib.id, &root).await.unwrap();

    // Second scan — file unchanged
    let result = scan_library(&db, lib.id, &root).await.unwrap();
    assert_eq!(result.inserted, 0);
    assert_eq!(result.updated, 0);
    drop(dir);
}
