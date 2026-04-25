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

    Mock::given(method("GET"))
        .and(path_regex("/release/rel-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "rel-1",
            "title": "Test Album",
            "date": "2000",
            "status": "Official",
            "media": [{"position": 1, "track-count": 1, "tracks": [
                {"position": 1, "number": "1", "recording": {"id": "rec-1"}}
            ]}],
            "artist-credit": null,
            "release-group": null,
            "label-info": null
        })))
        .mount(&mb_server)
        .await;

    let (store, track_id) = common::setup_with_fingerprinted_track().await;
    store.set_setting("acoustid_api_key", "test-key").await.unwrap();
    let mb_svc = Arc::new(MusicBrainzService::with_base_urls(
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
    let mb_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_regex("/v2/lookup"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "ok",
            "results": [{"id": "aid-1", "score": 0.5, "recordings": [{"id": "rec-1"}]}]
        })))
        .mount(&acoustid_server)
        .await;

    // AcoustID HAD results (score 0.5 but still returned), so text search is NOT tried.
    // No MB server call expected.

    let (store, track_id) = common::setup_with_fingerprinted_track().await;
    store.set_setting("acoustid_api_key", "test-key").await.unwrap();
    let mb_svc = Arc::new(MusicBrainzService::with_base_urls(
        mb_server.uri(),
        acoustid_server.uri(),
    ));

    let handler = MbLookupJobHandler::new(mb_svc);
    let result = handler
        .run(store.clone(), serde_json::json!({"track_id": track_id}))
        .await
        .unwrap();

    assert_eq!(result["suggestions_created"].as_i64(), Some(0));

    // Should NOT enqueue freedb_lookup — AcoustID recognised the track but confidence was low
    let jobs = store.list_jobs(Some("pending"), 50).await.unwrap();
    assert!(
        !jobs.iter().any(|j| j.job_type == "freedb_lookup"),
        "freedb_lookup should not be enqueued for below-threshold AcoustID matches"
    );
}

#[tokio::test]
async fn test_mb_lookup_no_acoustid_no_tags_no_discid_no_freedb() {
    // AcoustID returns empty → text search has no title/artist to search with →
    // no DISCID → freedb_lookup NOT enqueued.
    let acoustid_server = MockServer::start().await;
    let mb_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_regex("/v2/lookup"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "ok",
            "results": []
        })))
        .mount(&acoustid_server)
        .await;

    let (store, track_id) = common::setup_with_fingerprinted_track().await;
    store.set_setting("acoustid_api_key", "test-key").await.unwrap();
    let mb_svc = Arc::new(MusicBrainzService::with_base_urls(
        mb_server.uri(),
        acoustid_server.uri(),
    ));

    let handler = MbLookupJobHandler::new(mb_svc);
    handler
        .run(store.clone(), serde_json::json!({"track_id": track_id}))
        .await
        .unwrap();

    // The fingerprinted track has title="Test Song" and artist="Test Artist" in tags,
    // so text search IS tried — but we haven't mocked the MB recording search endpoint,
    // so it will attempt but since no mock is mounted the call may fail gracefully
    // (we unwrap_or_default in mb_lookup). No DISCID → no freedb.
    let jobs = store.list_jobs(Some("pending"), 50).await.unwrap();
    // Either freedb is not enqueued (no DISCID) or the text search returned no results
    // but also no DISCID tag exists on the test track.
    assert!(
        !jobs.iter().any(|j| j.job_type == "freedb_lookup"),
        "freedb_lookup should NOT be enqueued when no DISCID tag present"
    );
}

#[tokio::test]
async fn test_mb_lookup_text_search_fallback_creates_suggestion() {
    // AcoustID returns empty → text search returns 1 result → suggestion created with source="mb_search"
    let acoustid_server = MockServer::start().await;
    let mb_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_regex("/v2/lookup"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "ok",
            "results": []
        })))
        .mount(&acoustid_server)
        .await;

    // Mock the MB text search endpoint
    Mock::given(method("GET"))
        .and(path_regex("/recording/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "recordings": [
                {
                    "id": "rec-search-1",
                    "title": "Test Song",
                    "score": 80,
                    "length": 210000,
                    "releases": [{"id": "rel-search-1", "title": "Test Album", "date": "2000"}],
                    "artist-credit": [{"name": "Test Artist", "artist": {"id": "a1", "name": "Test Artist"}}]
                }
            ]
        })))
        .mount(&mb_server)
        .await;

    let (store, track_id) = common::setup_with_fingerprinted_track().await;
    store.set_setting("acoustid_api_key", "test-key").await.unwrap();
    let mb_svc = Arc::new(MusicBrainzService::with_base_urls(
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
    assert_eq!(suggestions[0].source, "mb_search", "source should be mb_search");
    assert!(
        suggestions[0].confidence <= 0.6,
        "mb_search confidence should be capped at 0.6, got {}",
        suggestions[0].confidence
    );
}

#[tokio::test]
async fn test_mb_lookup_discid_enqueues_freedb_when_no_mb_results() {
    // AcoustID empty → MB text search returns 0 results → DISCID present → freedb_lookup enqueued
    let acoustid_server = MockServer::start().await;
    let mb_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_regex("/v2/lookup"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "ok",
            "results": []
        })))
        .mount(&acoustid_server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex("/recording/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "recordings": []
        })))
        .mount(&mb_server)
        .await;

    let (store, track_id) = common::setup_with_discid_track("1234abcd", 1).await;
    store.set_setting("acoustid_api_key", "test-key").await.unwrap();

    // Also add a fingerprint so mb_lookup doesn't fail
    store
        .update_track_fingerprint(track_id, "AQADfakeFingerprint", 200.0)
        .await
        .unwrap();

    let mb_svc = Arc::new(MusicBrainzService::with_base_urls(
        mb_server.uri(),
        acoustid_server.uri(),
    ));

    let handler = MbLookupJobHandler::new(mb_svc);
    handler
        .run(store.clone(), serde_json::json!({"track_id": track_id}))
        .await
        .unwrap();

    let jobs = store.list_jobs(Some("pending"), 50).await.unwrap();
    assert!(
        jobs.iter().any(|j| j.job_type == "freedb_lookup"),
        "freedb_lookup should be enqueued when AcoustID empty + MB text search empty + DISCID present"
    );
}

#[tokio::test]
async fn test_mb_lookup_no_fingerprint_returns_error() {
    let (store, track_id) = common::setup_with_track().await; // no fingerprint
    let mb_svc = Arc::new(MusicBrainzService::with_base_urls(
        "http://unused".into(),
        "http://unused".into(),
    ));

    let handler = MbLookupJobHandler::new(mb_svc);
    let result = handler
        .run(store, serde_json::json!({"track_id": track_id}))
        .await;
    assert!(result.is_err(), "missing fingerprint should return an error");
}
