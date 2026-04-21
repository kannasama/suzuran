mod common;

#[tokio::test]
async fn test_create_and_query_track_link() {
    let store = common::setup_store().await;

    use suzuran_server::dal::UpsertTrack;

    // Create a library
    let lib = store
        .create_library("test", "/tmp/test", "flac")
        .await
        .unwrap();

    // Insert source track
    let src = store
        .upsert_track(UpsertTrack {
            library_id: lib.id,
            relative_path: "a.flac".into(),
            file_hash: "aaa".into(),
            title: None,
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
            duration_secs: None,
            bitrate: None,
            sample_rate: None,
            channels: None,
            bit_depth: None,
            has_embedded_art: false,
            status: "active".into(),
            library_profile_id: None,
        })
        .await
        .unwrap();

    // Insert derived track
    let derived = store
        .upsert_track(UpsertTrack {
            library_id: lib.id,
            relative_path: "a.aac".into(),
            file_hash: "bbb".into(),
            title: None,
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
            duration_secs: None,
            bitrate: None,
            sample_rate: None,
            channels: None,
            bit_depth: None,
            has_embedded_art: false,
            status: "active".into(),
            library_profile_id: None,
        })
        .await
        .unwrap();

    store
        .create_track_link(src.id, derived.id)
        .await
        .unwrap();

    let derived_links = store.list_derived_tracks(src.id).await.unwrap();
    assert_eq!(derived_links.len(), 1);
    assert_eq!(derived_links[0].derived_track_id, derived.id);

    let source_links = store.list_source_tracks(derived.id).await.unwrap();
    assert_eq!(source_links.len(), 1);
    assert_eq!(source_links[0].source_track_id, src.id);
}
