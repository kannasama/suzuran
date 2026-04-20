use std::sync::Arc;
use suzuran_server::jobs::{mb_lookup::MbLookupJobHandler, JobHandler};
use suzuran_server::services::musicbrainz::MusicBrainzService;
use wiremock::{matchers::{method, path_regex}, Mock, MockServer, ResponseTemplate};

mod common;

#[tokio::test]
async fn test_mb_lookup_creates_suggestion() {
    let acoustid_server = MockServer::start().await;
    let mb_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_regex("/v2/lookup"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "ok",
            "results": [{"id": "aid-1", "score": 0.95, "recordings": [{"id": "rec-1"}]}]
        })))
        .mount(&acoustid_server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex("/recording/rec-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "rec-1",
            "title": "Test Song",
            "length": null,
            "releases": [{"id": "rel-1", "title": "Test Album", "date": "2000"}],
            "artist-credit": null
        })))
        .mount(&mb_server)
        .await;

    let (store, track_id) = common::setup_with_fingerprinted_track().await;
    let mb_svc = Arc::new(MusicBrainzService::with_base_urls(
        "test-key".into(),
        mb_server.uri(),
        acoustid_server.uri(),
    ));

    let handler = MbLookupJobHandler::new(mb_svc);
    let result = handler
        .run(store.clone(), serde_json::json!({"track_id": track_id}))
        .await
        .unwrap();

    assert_eq!(result["suggestions_created"].as_i64(), Some(1));

    let suggestions = store
        .list_pending_tag_suggestions(Some(track_id))
        .await
        .unwrap();
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].source, "acoustid");
    assert!(
        (suggestions[0].confidence - 0.95_f32).abs() < 0.01,
        "confidence should be ~0.95, got {}",
        suggestions[0].confidence
    );
    assert_eq!(
        suggestions[0].mb_recording_id.as_deref(),
        Some("rec-1")
    );
    assert_eq!(
        suggestions[0].mb_release_id.as_deref(),
        Some("rel-1")
    );
    assert!(
        suggestions[0]
            .cover_art_url
            .as_deref()
            .map(|u| u.contains("rel-1"))
            .unwrap_or(false),
        "cover_art_url should contain the release ID"
    );
}

#[tokio::test]
async fn test_mb_lookup_below_threshold_creates_no_suggestion() {
    let acoustid_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_regex("/v2/lookup"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "ok",
            "results": [{"id": "aid-1", "score": 0.5, "recordings": [{"id": "rec-1"}]}]
        })))
        .mount(&acoustid_server)
        .await;

    let (store, track_id) = common::setup_with_fingerprinted_track().await;
    let mb_svc = Arc::new(MusicBrainzService::with_base_urls(
        "test-key".into(),
        "http://unused".into(),
        acoustid_server.uri(),
    ));

    let handler = MbLookupJobHandler::new(mb_svc);
    let result = handler
        .run(store.clone(), serde_json::json!({"track_id": track_id}))
        .await
        .unwrap();

    assert_eq!(result["suggestions_created"].as_i64(), Some(0));

    // Should have enqueued a freedb_lookup fallback
    let jobs = store.list_jobs(Some("pending"), 50).await.unwrap();
    assert!(
        jobs.iter().any(|j| j.job_type == "freedb_lookup"),
        "expected a freedb_lookup job to be enqueued"
    );
}

#[tokio::test]
async fn test_mb_lookup_no_fingerprint_returns_error() {
    let (store, track_id) = common::setup_with_track().await; // no fingerprint
    let mb_svc = Arc::new(MusicBrainzService::with_base_urls(
        "test-key".into(),
        "http://unused".into(),
        "http://unused".into(),
    ));

    let handler = MbLookupJobHandler::new(mb_svc);
    let result = handler
        .run(store, serde_json::json!({"track_id": track_id}))
        .await;
    assert!(result.is_err(), "missing fingerprint should return an error");
}
