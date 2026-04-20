use suzuran_server::services::transcode_compat::is_compatible;
use suzuran_server::models::EncodingProfile;

fn profile(codec: &str, sample_rate: Option<i64>, bit_depth: Option<i64>, bitrate: Option<&str>) -> EncodingProfile {
    EncodingProfile {
        id: 1, name: "test".into(), codec: codec.into(),
        bitrate: bitrate.map(str::to_string),
        sample_rate, channels: None, bit_depth,
        advanced_args: None,
        created_at: chrono::Utc::now(),
    }
}

#[test]
fn test_lossy_to_lossless_rejected() {
    assert!(!is_compatible("aac",  None, None, Some(192), &profile("flac", None, None, None)));
    assert!(!is_compatible("mp3",  None, None, Some(320), &profile("flac", None, None, None)));
    assert!(!is_compatible("opus", None, None, Some(128), &profile("flac", None, None, None)));
}

#[test]
fn test_lossless_to_lossy_allowed() {
    assert!(is_compatible("flac", Some(44100), Some(16), None, &profile("aac",  None, None, Some("256k"))));
    assert!(is_compatible("wv",   Some(96000), Some(24), None, &profile("mp3",  None, None, Some("320k"))));
}

#[test]
fn test_upsample_rejected() {
    assert!(!is_compatible("flac", Some(44100), Some(16), None, &profile("flac", Some(96000), Some(24), None)));
    assert!(is_compatible("flac", Some(96000), Some(24), None, &profile("flac", Some(96000), Some(24), None)));
    assert!(is_compatible("flac", Some(96000), Some(24), None, &profile("flac", Some(44100), Some(16), None)));
}

#[test]
fn test_bit_depth_inflation_rejected() {
    assert!(!is_compatible("flac", Some(44100), Some(16), None, &profile("flac", Some(44100), Some(24), None)));
    assert!(is_compatible("flac", Some(44100), Some(24), None, &profile("flac", Some(44100), Some(16), None)));
}

#[test]
fn test_bitrate_upscale_rejected() {
    assert!(!is_compatible("mp3", None, None, Some(128), &profile("mp3", None, None, Some("320k"))));
    assert!(is_compatible("mp3", None, None, Some(320), &profile("mp3", None, None, Some("128k"))));
}

#[test]
fn test_unknown_values_pass_through() {
    assert!(is_compatible("flac", None, None, None, &profile("flac", Some(96000), Some(24), None)));
}
