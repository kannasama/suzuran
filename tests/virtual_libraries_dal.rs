mod common;
use suzuran_server::dal::{UpsertTrack, UpsertVirtualLibrary};

#[tokio::test]
async fn test_virtual_library_crud_and_sources() {
    let store = common::setup_store().await;

    let vlib = store.create_virtual_library(UpsertVirtualLibrary {
        name: "Best Quality".into(),
        root_path: "/tmp/vlib".into(),
        link_type: "symlink".into(),
    }).await.unwrap();

    assert_eq!(vlib.link_type, "symlink");

    let all = store.list_virtual_libraries().await.unwrap();
    assert_eq!(all.len(), 1);

    // Create two real libraries to use as sources
    let lib1 = store.create_library("FLAC", "/tmp/flac", "flac", None).await.unwrap();
    let lib2 = store.create_library("AAC", "/tmp/aac", "aac", None).await.unwrap();

    // Set sources with priority ordering
    store.set_virtual_library_sources(vlib.id, &[(lib1.id, 1), (lib2.id, 2)]).await.unwrap();

    let sources = store.list_virtual_library_sources(vlib.id).await.unwrap();
    assert_eq!(sources.len(), 2);
    assert_eq!(sources[0].library_id, lib1.id);
    assert_eq!(sources[0].priority, 1);

    // Delete
    store.delete_virtual_library(vlib.id).await.unwrap();
    assert!(store.list_virtual_libraries().await.unwrap().is_empty());
}

#[tokio::test]
async fn test_virtual_library_tracks() {
    let store = common::setup_store().await;

    // Create vlib and a track
    let vlib = store.create_virtual_library(UpsertVirtualLibrary {
        name: "Test".into(),
        root_path: "/tmp/vt".into(),
        link_type: "symlink".into(),
    }).await.unwrap();

    // Create a library + track for FK
    let lib = store.create_library("src", "/tmp/src", "flac", None).await.unwrap();
    let track = store.upsert_track(UpsertTrack {
        library_id: lib.id,
        relative_path: "a.flac".into(),
        file_hash: "abc".into(),
        ..Default::default()
    }).await.unwrap();

    store.upsert_virtual_library_track(vlib.id, track.id, "a.flac").await.unwrap();
    let tracks = store.list_virtual_library_tracks(vlib.id).await.unwrap();
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].source_track_id, track.id);

    // Re-insert (upsert) should not error and count stays at 1
    store.upsert_virtual_library_track(vlib.id, track.id, "a_new.flac").await.unwrap();
    let tracks = store.list_virtual_library_tracks(vlib.id).await.unwrap();
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].link_path, "a_new.flac");

    store.clear_virtual_library_tracks(vlib.id).await.unwrap();
    assert!(store.list_virtual_library_tracks(vlib.id).await.unwrap().is_empty());
}
