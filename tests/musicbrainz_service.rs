use suzuran_server::services::musicbrainz::MusicBrainzService;
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path_regex};

#[tokio::test]
async fn test_acoustid_lookup_returns_scored_results() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex("/v2/lookup"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "ok",
            "results": [
                {
                    "id": "acoustid-abc",
                    "score": 0.96,
                    "recordings": [{"id": "rec-uuid-1"}]
                }
            ]
        })))
        .mount(&server)
        .await;

    let svc = MusicBrainzService::with_base_urls(
        "https://musicbrainz.org/ws/2".into(), // MB URL not used in this test
        server.uri(),
    );

    let results = svc.acoustid_lookup("test-key", "AQABz0kkdeRiJI...", 210.0).await.unwrap();
    assert_eq!(results.len(), 1);
    assert!((results[0].score - 0.96).abs() < 0.01);
    assert_eq!(results[0].recordings.as_ref().unwrap()[0].id, "rec-uuid-1");
}

#[tokio::test]
async fn test_acoustid_lookup_empty_results() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex("/v2/lookup"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "ok",
            "results": []
        })))
        .mount(&server)
        .await;

    let svc = MusicBrainzService::with_base_urls(
        "https://musicbrainz.org/ws/2".into(),
        server.uri(),
    );
    let results = svc.acoustid_lookup("test-key", "fp", 60.0).await.unwrap();
    assert_eq!(results.len(), 0);
}

#[tokio::test]
async fn test_get_recording_fetches_metadata() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex("/recording/rec-uuid-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "rec-uuid-1",
            "title": "Comfortably Numb",
            "length": 382000,
            "releases": [
                {
                    "id": "rel-uuid-1",
                    "title": "The Wall",
                    "date": "1979-11-30",
                    "artist-credit": [{"name": "Pink Floyd"}]
                }
            ]
        })))
        .mount(&server)
        .await;

    let svc = MusicBrainzService::with_base_urls(
        server.uri(),
        "https://api.acoustid.org".into(),
    );
    let rec = svc.get_recording("rec-uuid-1").await.unwrap();
    assert_eq!(rec.title, "Comfortably Numb");
    let releases = rec.releases.unwrap();
    assert_eq!(releases[0].title, "The Wall");
    assert_eq!(releases[0].date.as_deref(), Some("1979-11-30"));
}

#[tokio::test]
async fn test_to_tag_map_extracts_fields() {
    use suzuran_server::services::musicbrainz::{MbRecording, MbRelease, MbArtistCredit};

    let rec = MbRecording {
        id: "rec-1".into(),
        title: "Comfortably Numb".into(),
        length: Some(382000),
        releases: None,
        artist_credit: Some(vec![MbArtistCredit {
            name: Some("Pink Floyd".into()),
            artist: None,
        }]),
    };
    let release = MbRelease {
        id: "rel-1".into(),
        title: "The Wall".into(),
        date: Some("1979".into()),
        status: None,
        artist_credit: None,
        label_info: None,
        release_group: None,
        media: None,
    };

    let tags = MusicBrainzService::to_tag_map(&rec, &release);
    assert_eq!(tags.get("title").map(String::as_str), Some("Comfortably Numb"));
    assert_eq!(tags.get("album").map(String::as_str), Some("The Wall"));
    assert_eq!(tags.get("artist").map(String::as_str), Some("Pink Floyd"));
    // albumartist falls back to recording artist when release has no artist_credit
    assert_eq!(tags.get("albumartist").map(String::as_str), Some("Pink Floyd"));
    assert_eq!(tags.get("date").map(String::as_str), Some("1979"));
    assert_eq!(tags.get("musicbrainz_trackid").map(String::as_str), Some("rec-1"));
    assert_eq!(tags.get("musicbrainz_releaseid").map(String::as_str), Some("rel-1"));
}

#[tokio::test]
async fn test_to_tag_map_uses_release_artist_for_albumartist() {
    use suzuran_server::services::musicbrainz::{MbRecording, MbRelease, MbArtistCredit};

    // Compilation: recording artist is the track artist, release artist is "Various Artists"
    let rec = MbRecording {
        id: "rec-2".into(),
        title: "Blue Monday".into(),
        length: Some(450000),
        releases: None,
        artist_credit: Some(vec![MbArtistCredit {
            name: Some("New Order".into()),
            artist: None,
        }]),
    };
    let release = MbRelease {
        id: "rel-2".into(),
        title: "Now That's What I Call Music 1".into(),
        date: Some("1983".into()),
        status: None,
        artist_credit: Some(vec![MbArtistCredit {
            name: Some("Various Artists".into()),
            artist: None,
        }]),
        label_info: None,
        release_group: None,
        media: None,
    };

    let tags = MusicBrainzService::to_tag_map(&rec, &release);
    assert_eq!(tags.get("artist").map(String::as_str), Some("New Order"));
    assert_eq!(tags.get("albumartist").map(String::as_str), Some("Various Artists"));
}

#[tokio::test]
async fn test_get_recording_rate_limit_second_call_does_not_sleep_full_interval() {
    use std::time::Instant;

    let server = MockServer::start().await;
    // Mount the mock for two calls
    Mock::given(method("GET"))
        .and(path_regex("/recording/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "rec-rl-1",
            "title": "Rate Limit Test",
            "length": null,
            "releases": null,
            "artist-credit": null
        })))
        .expect(2)
        .mount(&server)
        .await;

    let svc = MusicBrainzService::with_base_urls(
        server.uri(),
        "https://api.acoustid.org".into(),
    );

    // First call — no prior request, should return immediately (no sleep)
    let t0 = Instant::now();
    svc.get_recording("rec-rl-1").await.unwrap();
    let first_call_ms = t0.elapsed().as_millis();

    // First call must not sleep the full 1100ms
    assert!(
        first_call_ms < 1100,
        "first call took {}ms, expected < 1100ms (no prior request)",
        first_call_ms
    );

    // Second call — issued immediately; should sleep only the remaining window
    // Total for two back-to-back calls must be < 2200ms (they share the rate-limit window)
    let t1 = Instant::now();
    svc.get_recording("rec-rl-1").await.unwrap();
    let two_calls_ms = t1.elapsed().as_millis();

    assert!(
        two_calls_ms < 1200,
        "second call took {}ms; expected the sleep to be well under 1200ms (most of the window already elapsed)",
        two_calls_ms
    );
}

#[tokio::test]
async fn test_search_recordings_returns_results() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_regex("/recording/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "recordings": [
                {
                    "id": "rec-search-1",
                    "title": "Test Song",
                    "score": 88,
                    "length": 240000,
                    "releases": [
                        {
                            "id": "rel-search-1",
                            "title": "Test Album",
                            "date": "2010"
                        }
                    ],
                    "artist-credit": [
                        {
                            "name": "Test Artist",
                            "artist": { "id": "art-1", "name": "Test Artist" }
                        }
                    ]
                }
            ]
        })))
        .mount(&server)
        .await;

    let svc = MusicBrainzService::with_base_urls(
        server.uri(),
        "https://api.acoustid.org".into(),
    );

    let results = svc
        .search_recordings("Test Song", "Test Artist", "Test Album")
        .await
        .unwrap();

    assert_eq!(results.len(), 1, "should return 1 result");

    let (tags, confidence) = &results[0];
    assert!(
        *confidence <= 0.6,
        "confidence should be capped at 0.6, got {confidence}"
    );
    assert_eq!(tags.get("title").map(String::as_str), Some("Test Song"));
    assert_eq!(tags.get("album").map(String::as_str), Some("Test Album"));
    assert_eq!(tags.get("artist").map(String::as_str), Some("Test Artist"));
}

#[tokio::test]
async fn test_search_recordings_empty_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_regex("/recording/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "recordings": []
        })))
        .mount(&server)
        .await;

    let svc = MusicBrainzService::with_base_urls(
        server.uri(),
        "https://api.acoustid.org".into(),
    );

    let results = svc
        .search_recordings("Unknown", "Nobody", "Nowhere")
        .await
        .unwrap();

    assert!(results.is_empty(), "empty recordings should return empty vec");
}
