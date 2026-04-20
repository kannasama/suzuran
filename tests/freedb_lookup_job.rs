use std::sync::Arc;
use suzuran_server::jobs::{freedb_lookup::FreedBLookupJobHandler, JobHandler};
use suzuran_server::services::freedb::FreedBService;
use wiremock::matchers::{method, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

mod common;

#[tokio::test]
async fn test_freedb_job_creates_suggestion_for_discid_track() {
    let server = MockServer::start().await;

    // Mock query call
    Mock::given(method("GET"))
        .and(query_param("cmd", "cddb query a50e1d13 1 0 60"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "211 Found inexact matches\nrock a50e1d13 Test Artist / Test Album\n.\n",
        ))
        .mount(&server)
        .await;

    // Mock read call
    Mock::given(method("GET"))
        .and(query_param("cmd", "cddb read rock a50e1d13"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "200 OK\nDTITLE=Test Artist / Test Album\nDYEAR=1999\nDGENRE=Rock\nTTITLE0=Track One\nTTITLE1=Track Two\nTTITLE2=Track Three\n.\n",
        ))
        .mount(&server)
        .await;

    let (store, track_id) = common::setup_with_discid_track("a50e1d13", 3).await;
    let svc = Arc::new(FreedBService::with_base_url(server.uri()));

    let handler = FreedBLookupJobHandler::new(svc);
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
    assert_eq!(suggestions[0].source, "freedb");
    assert!(
        (suggestions[0].confidence - 0.5_f32).abs() < 0.01,
        "confidence should be 0.5, got {}",
        suggestions[0].confidence
    );

    // The track is track 3, so index 2 → "Track Three"
    let tags: serde_json::Value =
        serde_json::from_value(suggestions[0].suggested_tags.clone()).unwrap();
    assert_eq!(tags["title"].as_str(), Some("Track Three"));
    assert_eq!(tags["artist"].as_str(), Some("Test Artist"));
    assert_eq!(tags["album"].as_str(), Some("Test Album"));
}

#[tokio::test]
async fn test_freedb_job_skips_track_without_discid() {
    let (store, track_id) = common::setup_with_track().await; // no DISCID
    let svc = Arc::new(FreedBService::with_base_url("http://unused".into()));

    let handler = FreedBLookupJobHandler::new(svc);
    let result = handler
        .run(store.clone(), serde_json::json!({"track_id": track_id}))
        .await
        .unwrap();

    assert_eq!(result["skipped"].as_bool(), Some(true));

    let suggestions = store
        .list_pending_tag_suggestions(Some(track_id))
        .await
        .unwrap();
    assert_eq!(suggestions.len(), 0);
}

#[tokio::test]
async fn test_freedb_job_creates_zero_suggestions_on_no_match() {
    let server = MockServer::start().await;

    // Server returns 202 (no match)
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("202 No match found\n"))
        .mount(&server)
        .await;

    let (store, track_id) = common::setup_with_discid_track("00000000", 1).await;
    let svc = Arc::new(FreedBService::with_base_url(server.uri()));

    let handler = FreedBLookupJobHandler::new(svc);
    let result = handler
        .run(store.clone(), serde_json::json!({"track_id": track_id}))
        .await
        .unwrap();

    assert_eq!(result["suggestions_created"].as_i64(), Some(0));

    let suggestions = store
        .list_pending_tag_suggestions(Some(track_id))
        .await
        .unwrap();
    assert_eq!(suggestions.len(), 0);
}

#[tokio::test]
async fn test_freedb_job_returns_error_for_missing_track() {
    let svc = Arc::new(FreedBService::with_base_url("http://unused".into()));
    let db = common::make_db().await;

    let handler = FreedBLookupJobHandler::new(svc);
    let result = handler
        .run(db, serde_json::json!({"track_id": 999999}))
        .await;

    assert!(result.is_err(), "should error when track_id does not exist");
}
