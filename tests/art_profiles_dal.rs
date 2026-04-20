mod common;
use suzuran_server::dal::UpsertArtProfile;

#[tokio::test]
async fn test_art_profile_crud() {
    let store = common::setup_store().await;

    let ap = store.create_art_profile(UpsertArtProfile {
        name: "Standard 500px".into(),
        max_width_px: 500,
        max_height_px: 500,
        max_size_bytes: Some(200_000),
        format: "jpeg".into(),
        quality: 90,
        apply_to_library_id: None,
    }).await.unwrap();

    assert_eq!(ap.format, "jpeg");
    assert_eq!(ap.quality, 90);

    let all = store.list_art_profiles().await.unwrap();
    assert_eq!(all.len(), 1);

    let fetched = store.get_art_profile(ap.id).await.unwrap();
    assert_eq!(fetched.name, "Standard 500px");

    let updated = store.update_art_profile(ap.id, UpsertArtProfile {
        name: "Standard 500px".into(),
        max_width_px: 800,
        max_height_px: 800,
        max_size_bytes: None,
        format: "png".into(),
        quality: 85,
        apply_to_library_id: None,
    }).await.unwrap();
    assert_eq!(updated.format, "png");
    assert_eq!(updated.max_width_px, 800);

    store.delete_art_profile(ap.id).await.unwrap();
    assert!(store.list_art_profiles().await.unwrap().is_empty());
}
