mod common;

use std::sync::Arc;
use suzuran_server::dal::{UpsertTrack, UpsertVirtualLibrary};
use suzuran_server::jobs::virtual_sync::VirtualSyncJobHandler;
use suzuran_server::jobs::JobHandler;
use tempfile::TempDir;

#[tokio::test]
async fn test_virtual_sync_creates_symlinks() {
    let src_dir = TempDir::new().unwrap();
    let vlib_dir = TempDir::new().unwrap();

    let store = common::setup_store().await;

    // Write a dummy file at the expected path
    let track_rel = "artist/album/01.flac";
    let track_abs = src_dir.path().join(track_rel);
    tokio::fs::create_dir_all(track_abs.parent().unwrap()).await.unwrap();
    tokio::fs::write(&track_abs, b"dummy").await.unwrap();

    // Create library + track in DB
    let lib = store
        .create_library(
            "FLAC",
            src_dir.path().to_str().unwrap(),
            "flac",
            None,
        )
        .await
        .unwrap();

    store
        .upsert_track(UpsertTrack {
            library_id: lib.id,
            relative_path: track_rel.to_string(),
            file_hash: "abc".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    // Create virtual library
    let vlib = store
        .create_virtual_library(UpsertVirtualLibrary {
            name: "Best".into(),
            root_path: vlib_dir.path().to_str().unwrap().to_string(),
            link_type: "symlink".into(),
        })
        .await
        .unwrap();
    store
        .set_virtual_library_sources(vlib.id, &[(lib.id, 1)])
        .await
        .unwrap();

    let handler = VirtualSyncJobHandler::new(store.clone());
    let result = handler
        .run(
            store.clone() as Arc<dyn suzuran_server::dal::Store>,
            serde_json::json!({ "virtual_library_id": vlib.id }),
        )
        .await
        .unwrap();

    assert_eq!(result["status"].as_str(), Some("completed"));
    assert_eq!(result["tracks_linked"].as_i64(), Some(1));

    // Symlink should exist at vlib_dir/artist/album/01.flac
    let link_path = vlib_dir.path().join(track_rel);
    assert!(
        link_path.exists() || link_path.is_symlink(),
        "symlink should exist at {}",
        link_path.display()
    );

    let db_tracks = store.list_virtual_library_tracks(vlib.id).await.unwrap();
    assert_eq!(db_tracks.len(), 1);
}

#[tokio::test]
async fn test_virtual_sync_priority_order() {
    let src1 = TempDir::new().unwrap();
    let src2 = TempDir::new().unwrap();
    let vlib_dir = TempDir::new().unwrap();

    let store = common::setup_store().await;

    // Both source dirs have a file at "01.flac"
    tokio::fs::write(src1.path().join("01.flac"), b"src1").await.unwrap();
    tokio::fs::write(src2.path().join("01.flac"), b"src2").await.unwrap();

    let lib1 = store
        .create_library("L1", src1.path().to_str().unwrap(), "flac")
        .await
        .unwrap();
    let lib2 = store
        .create_library("L2", src2.path().to_str().unwrap(), "flac")
        .await
        .unwrap();

    // Both tracks have the same tag-tuple identity (no MusicBrainz ID)
    let t1 = store
        .upsert_track(UpsertTrack {
            library_id: lib1.id,
            relative_path: "01.flac".into(),
            file_hash: "h1".into(),
            artist: Some("Artist".into()),
            album: Some("Album".into()),
            tracknumber: Some("1".into()),
            discnumber: Some("1".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    store
        .upsert_track(UpsertTrack {
            library_id: lib2.id,
            relative_path: "01.flac".into(),
            file_hash: "h2".into(),
            artist: Some("Artist".into()),
            album: Some("Album".into()),
            tracknumber: Some("1".into()),
            discnumber: Some("1".into()),
            ..Default::default()
        })
        .await
        .unwrap();

    let vlib = store
        .create_virtual_library(UpsertVirtualLibrary {
            name: "Best".into(),
            root_path: vlib_dir.path().to_str().unwrap().to_string(),
            link_type: "symlink".into(),
        })
        .await
        .unwrap();
    // lib1 = priority 1 (wins), lib2 = priority 2
    store
        .set_virtual_library_sources(vlib.id, &[(lib1.id, 1), (lib2.id, 2)])
        .await
        .unwrap();

    let handler = VirtualSyncJobHandler::new(store.clone());
    handler
        .run(
            store.clone() as Arc<dyn suzuran_server::dal::Store>,
            serde_json::json!({ "virtual_library_id": vlib.id }),
        )
        .await
        .unwrap();

    let db_tracks = store.list_virtual_library_tracks(vlib.id).await.unwrap();
    assert_eq!(db_tracks.len(), 1, "only one track (priority dedup)");
    assert_eq!(
        db_tracks[0].source_track_id,
        t1.id,
        "lib1 (priority 1) should win"
    );
}

#[tokio::test]
async fn test_virtual_sync_is_idempotent() {
    let src_dir = TempDir::new().unwrap();
    let vlib_dir = TempDir::new().unwrap();
    let store = common::setup_store().await;

    tokio::fs::write(src_dir.path().join("01.flac"), b"dummy").await.unwrap();
    let lib = store
        .create_library("L", src_dir.path().to_str().unwrap(), "flac")
        .await
        .unwrap();
    store
        .upsert_track(UpsertTrack {
            library_id: lib.id,
            relative_path: "01.flac".into(),
            file_hash: "h".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let vlib = store
        .create_virtual_library(UpsertVirtualLibrary {
            name: "V".into(),
            root_path: vlib_dir.path().to_str().unwrap().to_string(),
            link_type: "symlink".into(),
        })
        .await
        .unwrap();
    store
        .set_virtual_library_sources(vlib.id, &[(lib.id, 1)])
        .await
        .unwrap();

    let handler = VirtualSyncJobHandler::new(store.clone());
    let payload = serde_json::json!({ "virtual_library_id": vlib.id });

    handler
        .run(
            store.clone() as Arc<dyn suzuran_server::dal::Store>,
            payload.clone(),
        )
        .await
        .unwrap();
    handler
        .run(
            store.clone() as Arc<dyn suzuran_server::dal::Store>,
            payload,
        )
        .await
        .unwrap();

    let db_tracks = store.list_virtual_library_tracks(vlib.id).await.unwrap();
    assert_eq!(db_tracks.len(), 1, "no duplicate tracks after re-sync");
}
