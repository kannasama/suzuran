use std::{path::PathBuf, sync::Arc};
use tokio::fs;
use suzuran_server::{
    dal::{sqlite::SqliteStore, Store, UpsertTrack},
    scanner::scan_library,
};

async fn make_db() -> Arc<dyn Store> {
    let store = SqliteStore::new("sqlite::memory:").await.unwrap();
    store.migrate().await.unwrap();
    Arc::new(store)
}

/// Create a temp dir with source/ and ingest/ subdirs.
/// Returns (TempDir, root_path).
async fn make_temp_library_dirs() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().to_path_buf();
    fs::create_dir_all(path.join("source")).await.unwrap();
    fs::create_dir_all(path.join("ingest")).await.unwrap();
    (dir, path)
}

#[tokio::test]
async fn scanner_inserts_new_files_from_source() {
    let db = make_db().await;
    let (dir, root) = make_temp_library_dirs().await;

    fs::write(root.join("source").join("track01.flac"), b"").await.unwrap();
    fs::write(root.join("source").join("track02.flac"), b"").await.unwrap();

    let lib = db.create_library("Test", root.to_str().unwrap(), "flac").await.unwrap();

    let result = scan_library(&db, lib.id, &root).await.unwrap();
    assert_eq!(result.inserted, 2, "should insert 2 files from source/");
    assert_eq!(result.removed, 0);

    let tracks = db.list_tracks_by_library(lib.id).await.unwrap();
    assert_eq!(tracks.len(), 2);
    // All source tracks should be "active"
    assert!(tracks.iter().all(|t| t.status == "active"));
    drop(dir);
}

#[tokio::test]
async fn scanner_staged_track_in_ingest() {
    let db = make_db().await;
    let (dir, root) = make_temp_library_dirs().await;

    // Write a file into ingest/
    fs::write(root.join("ingest").join("new_track.flac"), b"staged content").await.unwrap();

    let lib = db.create_library("Test", root.to_str().unwrap(), "flac").await.unwrap();

    let result = scan_library(&db, lib.id, &root).await.unwrap();
    assert_eq!(result.inserted, 1, "should insert 1 staged file from ingest/");
    assert_eq!(result.removed, 0);

    let tracks = db.list_tracks_by_library(lib.id).await.unwrap();
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].status, "staged", "ingest/ track should have status=staged");
    assert!(
        tracks[0].relative_path.starts_with("ingest/"),
        "relative_path should start with ingest/"
    );
    drop(dir);
}

#[tokio::test]
async fn scanner_removes_deleted_files_in_source() {
    let db = make_db().await;
    let (dir, root) = make_temp_library_dirs().await;

    fs::write(root.join("source").join("track01.flac"), b"").await.unwrap();
    fs::write(root.join("source").join("track02.flac"), b"").await.unwrap();

    let lib = db.create_library("Test", root.to_str().unwrap(), "flac").await.unwrap();
    scan_library(&db, lib.id, &root).await.unwrap();

    // Remove one file
    fs::remove_file(root.join("source").join("track02.flac")).await.unwrap();

    let result = scan_library(&db, lib.id, &root).await.unwrap();
    assert_eq!(result.removed, 1);

    drop(dir);
}

#[tokio::test]
async fn scanner_removes_deleted_files_in_ingest() {
    let db = make_db().await;
    let (dir, root) = make_temp_library_dirs().await;

    fs::write(root.join("ingest").join("track01.flac"), b"").await.unwrap();

    let lib = db.create_library("Test", root.to_str().unwrap(), "flac").await.unwrap();
    scan_library(&db, lib.id, &root).await.unwrap();

    // Remove the ingest file
    fs::remove_file(root.join("ingest").join("track01.flac")).await.unwrap();

    let result = scan_library(&db, lib.id, &root).await.unwrap();
    assert_eq!(result.removed, 1);

    drop(dir);
}

#[tokio::test]
async fn scanner_skips_unchanged_files() {
    let db = make_db().await;
    let (dir, root) = make_temp_library_dirs().await;

    fs::write(root.join("source").join("track01.flac"), b"data").await.unwrap();

    let lib = db.create_library("Test", root.to_str().unwrap(), "flac").await.unwrap();
    scan_library(&db, lib.id, &root).await.unwrap();

    // Second scan — file unchanged
    let result = scan_library(&db, lib.id, &root).await.unwrap();
    assert_eq!(result.inserted, 0);
    assert_eq!(result.updated, 0);
    drop(dir);
}

#[tokio::test]
async fn scanner_detects_hash_change_in_source() {
    let db = make_db().await;
    let (dir, root) = make_temp_library_dirs().await;

    let track_path = root.join("source").join("track01.flac");
    fs::write(&track_path, b"original content").await.unwrap();

    let lib = db.create_library("Test", root.to_str().unwrap(), "flac").await.unwrap();
    scan_library(&db, lib.id, &root).await.unwrap();

    // Modify the file
    fs::write(&track_path, b"modified content").await.unwrap();

    let result = scan_library(&db, lib.id, &root).await.unwrap();
    assert_eq!(result.updated, 1, "changed file should be counted as updated");
    drop(dir);
}

#[tokio::test]
async fn scanner_does_not_enqueue_transcode_jobs() {
    let db = make_db().await;
    let (dir, root) = make_temp_library_dirs().await;

    fs::write(root.join("source").join("track01.flac"), b"").await.unwrap();

    let lib = db.create_library("Test", root.to_str().unwrap(), "flac").await.unwrap();
    scan_library(&db, lib.id, &root).await.unwrap();

    let jobs = db.list_jobs(Some("pending"), 50).await.unwrap();
    assert!(
        !jobs.iter().any(|j| j.job_type == "transcode"),
        "scanner should NOT enqueue transcode jobs"
    );
    drop(dir);
}

#[tokio::test]
async fn scanner_skips_missing_ingest_and_source_dirs() {
    let db = make_db().await;
    // Create an empty root (no ingest/ or source/ subdir)
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    let lib = db.create_library("Test", root.to_str().unwrap(), "flac").await.unwrap();

    // Should not error — missing dirs are silently skipped
    let result = scan_library(&db, lib.id, &root).await.unwrap();
    assert_eq!(result.inserted, 0);
    assert_eq!(result.removed, 0);
    drop(dir);
}
