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
        "test-key".into(),
        "https://musicbrainz.org/ws/2".into(), // MB URL not used in this test
        server.uri(),
    );

    let results = svc.acoustid_lookup("AQABz0kkdeRiJI...", 210.0).await.unwrap();
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
        "test-key".into(),
        "https://musicbrainz.org/ws/2".into(),
        server.uri(),
    );
    let results = svc.acoustid_lookup("fp", 60.0).await.unwrap();
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
        "test-key".into(),
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
        artist_credit: None,
        label_info: None,
        release_group: None,
        media: None,
    };

    let tags = MusicBrainzService::to_tag_map(&rec, &release);
    assert_eq!(tags.get("title").map(String::as_str), Some("Comfortably Numb"));
    assert_eq!(tags.get("album").map(String::as_str), Some("The Wall"));
    assert_eq!(tags.get("artist").map(String::as_str), Some("Pink Floyd"));
    assert_eq!(tags.get("date").map(String::as_str), Some("1979"));
    assert_eq!(tags.get("musicbrainz_trackid").map(String::as_str), Some("rec-1"));
    assert_eq!(tags.get("musicbrainz_releaseid").map(String::as_str), Some("rel-1"));
}
