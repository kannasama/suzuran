mod common;
use suzuran_server::dal::UpsertEncodingProfile;

#[tokio::test]
async fn test_encoding_profile_crud() {
    let store = common::setup_store().await;

    let ep = store.create_encoding_profile(UpsertEncodingProfile {
        name: "AAC 256k".into(),
        codec: "aac".into(),
        bitrate: Some("256k".into()),
        sample_rate: Some(44100),
        channels: Some(2),
        bit_depth: None,
        advanced_args: None,
    }).await.unwrap();

    assert_eq!(ep.codec, "aac");

    let all = store.list_encoding_profiles().await.unwrap();
    assert_eq!(all.len(), 1);

    let fetched = store.get_encoding_profile(ep.id).await.unwrap();
    assert_eq!(fetched.name, "AAC 256k");

    let updated = store.update_encoding_profile(ep.id, UpsertEncodingProfile {
        name: "AAC 320k".into(),
        codec: "aac".into(),
        bitrate: Some("320k".into()),
        sample_rate: Some(44100),
        channels: Some(2),
        bit_depth: None,
        advanced_args: None,
    }).await.unwrap();
    assert_eq!(updated.bitrate.as_deref(), Some("320k"));

    store.delete_encoding_profile(ep.id).await.unwrap();
    assert!(store.list_encoding_profiles().await.unwrap().is_empty());
}
