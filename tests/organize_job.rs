use std::sync::Arc;
use tokio::fs;
use suzuran_server::{
    dal::{sqlite::SqliteStore, Store, UpsertTrack},
    jobs::{organize::OrganizeJobHandler, JobHandler, OrganizePayload},
};

async fn make_db() -> Arc<dyn Store> {
    let store = SqliteStore::new("sqlite::memory:").await.unwrap();
    store.migrate().await.unwrap();
    Arc::new(store)
}

#[tokio::test]
async fn organize_moves_file_and_updates_path() {
    let db = make_db().await;

    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path();

    // Seed a file at the "wrong" path
    let old_rel = "unsorted/track.flac";
    let old_abs = root.join(old_rel);
    fs::create_dir_all(old_abs.parent().unwrap()).await.unwrap();
    fs::write(&old_abs, b"audio").await.unwrap();

    let lib = db.create_library("FLAC", root.to_str().unwrap(), "flac").await.unwrap();
    let track = db.upsert_track(UpsertTrack {
        library_id: lib.id,
        relative_path: old_rel.to_string(),
        file_hash: "abc".to_string(),
        title: Some("Comfortably Numb".to_string()),
        artist: Some("Pink Floyd".to_string()),
        albumartist: Some("Pink Floyd".to_string()),
        album: Some("The Wall".to_string()),
        tracknumber: Some("6".to_string()),
        discnumber: Some("1".to_string()),
        totaldiscs: Some("1".to_string()),
        totaltracks: Some("26".to_string()),
        date: Some("1979".to_string()),
        genre: None, composer: None, label: None, catalognumber: None,
        tags: serde_json::json!({
            "title": "Comfortably Numb",
            "albumartist": "Pink Floyd",
            "album": "The Wall",
            "tracknumber": "6",
            "discnumber": "1",
            "totaldiscs": "1",
            "date": "1979"
        }),
        duration_secs: None, bitrate: None, sample_rate: None, channels: None,
        bit_depth: None, has_embedded_art: false,
        status: "active".into(), library_profile_id: None,
    }).await.unwrap();

    let rule = db.create_organization_rule(
        "Default", None, 0, None,
        "{albumartist}/{date} - {album}/{tracknumber:02} - {title}",
        true,
    ).await.unwrap();
    db.set_library_org_rule(lib.id, Some(rule.id)).await.unwrap();

    let handler = OrganizeJobHandler;
    let payload = serde_json::to_value(OrganizePayload { track_id: track.id, dry_run: false }).unwrap();
    let result = handler.run(db.clone(), payload).await.unwrap();

    let expected_new = "source/Pink Floyd/1979 - The Wall/06 - Comfortably Numb.flac";
    assert_eq!(result["new_path"], serde_json::json!(expected_new));
    assert_eq!(result["moved"], serde_json::json!(true));

    assert!(root.join(expected_new).exists(), "file should be at new path");
    assert!(!old_abs.exists(), "file should no longer be at old path");

    let updated = db.get_track(track.id).await.unwrap().unwrap();
    assert_eq!(updated.relative_path, expected_new);

    drop(dir);
}

#[tokio::test]
async fn organize_dry_run_does_not_move() {
    let db = make_db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path();

    let old_rel = "track.flac";
    fs::write(root.join(old_rel), b"audio").await.unwrap();

    let lib = db.create_library("FLAC", root.to_str().unwrap(), "flac").await.unwrap();
    let track = db.upsert_track(UpsertTrack {
        library_id: lib.id,
        relative_path: old_rel.to_string(),
        file_hash: "xyz".to_string(),
        title: Some("Song".to_string()),
        albumartist: Some("Artist".to_string()),
        tags: serde_json::json!({"title":"Song","albumartist":"Artist","date":"2000","tracknumber":"1"}),
        artist: None, album: None, tracknumber: Some("1".to_string()),
        discnumber: None, totaldiscs: None, totaltracks: None,
        date: Some("2000".to_string()), genre: None, composer: None,
        label: None, catalognumber: None,
        duration_secs: None, bitrate: None, sample_rate: None, channels: None,
        bit_depth: None, has_embedded_art: false,
        status: "active".into(), library_profile_id: None,
    }).await.unwrap();

    let rule = db.create_organization_rule("Default", None, 0, None, "{albumartist}/{date}/{title}", true).await.unwrap();
    db.set_library_org_rule(lib.id, Some(rule.id)).await.unwrap();

    let handler = OrganizeJobHandler;
    let payload = serde_json::to_value(OrganizePayload { track_id: track.id, dry_run: true }).unwrap();
    let result = handler.run(db.clone(), payload).await.unwrap();

    assert_eq!(result["dry_run"], serde_json::json!(true));
    assert!(result["proposed_path"].is_string());
    assert!(root.join(old_rel).exists(), "file should still be at original path");

    drop(dir);
}

#[tokio::test]
async fn organize_dry_run_no_matching_rule_returns_null_path() {
    let db = make_db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path();

    let old_rel = "track.flac";
    tokio::fs::write(root.join(old_rel), b"audio").await.unwrap();

    let lib = db.create_library("FLAC", root.to_str().unwrap(), "flac").await.unwrap();
    let track = db.upsert_track(UpsertTrack {
        library_id: lib.id,
        relative_path: old_rel.to_string(),
        file_hash: "xyz".to_string(),
        tags: serde_json::json!({}),
        ..UpsertTrack::default()
    }).await.unwrap();

    // No rules created — apply_rules returns None
    let handler = OrganizeJobHandler;
    let payload = serde_json::to_value(OrganizePayload { track_id: track.id, dry_run: true }).unwrap();
    let result = handler.run(db.clone(), payload).await.unwrap();

    assert_eq!(result["dry_run"], serde_json::json!(true));
    assert!(result["proposed_path"].is_null(), "proposed_path should be null when no rule matches");
    assert!(root.join(old_rel).exists(), "file should be untouched");

    drop(dir);
}
