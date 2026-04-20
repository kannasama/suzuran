use suzuran_server::services::freedb::{parse_xmcd, FreedBCandidate, FreedBService};
use wiremock::matchers::{method, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_disc_lookup_returns_candidate() {
    let server = MockServer::start().await;

    // Mock the query call — matched by cmd starting with "cddb query"
    Mock::given(method("GET"))
        .and(query_param("cmd", "cddb query a50e1d13 1 0 60"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(
                "211 Found inexact matches\nrock a50e1d13 Test Artist / Test Album\n.\n",
            ),
        )
        .mount(&server)
        .await;

    // Mock the read call
    Mock::given(method("GET"))
        .and(query_param("cmd", "cddb read rock a50e1d13"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "200 OK\nDTITLE=Test Artist / Test Album\nDYEAR=1999\nDGENRE=Rock\nTTITLE0=Track One\nTTITLE1=Track Two\n.\n",
        ))
        .mount(&server)
        .await;

    let svc = FreedBService::with_base_url(server.uri());
    let result = svc.disc_lookup("a50e1d13").await.unwrap();
    assert!(result.is_some());
    let candidate = result.unwrap();
    assert_eq!(candidate.artist, "Test Artist");
    assert_eq!(candidate.album, "Test Album");
    assert_eq!(candidate.year.as_deref(), Some("1999"));
    assert_eq!(candidate.genre.as_deref(), Some("Rock"));
    assert_eq!(candidate.tracks.len(), 2);
    assert_eq!(candidate.tracks[0], "Track One");
    assert_eq!(candidate.tracks[1], "Track Two");
}

#[tokio::test]
async fn test_disc_lookup_returns_none_on_202() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("202 No match found\n"))
        .mount(&server)
        .await;

    let svc = FreedBService::with_base_url(server.uri());
    let result = svc.disc_lookup("deadbeef").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_disc_lookup_returns_none_when_read_fails() {
    let server = MockServer::start().await;

    // Query succeeds and returns a result
    Mock::given(method("GET"))
        .and(query_param("cmd", "cddb query deadc0de 1 0 60"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("200 Found exact match\nrock deadc0de Artist / Album\n.\n"),
        )
        .mount(&server)
        .await;

    // Read returns non-200 status line
    Mock::given(method("GET"))
        .and(query_param("cmd", "cddb read rock deadc0de"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string("401 Permission denied\n"),
        )
        .mount(&server)
        .await;

    let svc = FreedBService::with_base_url(server.uri());
    let result = svc.disc_lookup("deadc0de").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_to_tag_map_extracts_fields() {
    let candidate = FreedBCandidate {
        artist: "Test Artist".into(),
        album: "Test Album".into(),
        year: Some("1999".into()),
        genre: Some("Rock".into()),
        tracks: vec!["Track One".into(), "Track Two".into()],
    };
    let tags = FreedBService::to_tag_map(&candidate, 1);
    assert_eq!(tags["artist"], "Test Artist");
    assert_eq!(tags["albumartist"], "Test Artist");
    assert_eq!(tags["album"], "Test Album");
    assert_eq!(tags["date"], "1999");
    assert_eq!(tags["genre"], "Rock");
    assert_eq!(tags["title"], "Track Two");
    assert_eq!(tags["totaltracks"], "2");
}

#[tokio::test]
async fn test_to_tag_map_no_optional_fields() {
    let candidate = FreedBCandidate {
        artist: "Artist".into(),
        album: "Album".into(),
        year: None,
        genre: None,
        tracks: vec!["Only Track".into()],
    };
    let tags = FreedBService::to_tag_map(&candidate, 0);
    assert_eq!(tags["title"], "Only Track");
    assert!(!tags.contains_key("date"));
    assert!(!tags.contains_key("genre"));
    assert_eq!(tags["totaltracks"], "1");
}

#[test]
fn test_parse_xmcd_multiline_ttitle() {
    // CDDB spec allows a field to span multiple lines by repeating the key.
    // The continuation parts should be appended (no separator) to the first segment.
    let xmcd = "200 OK\n\
        DTITLE=Test Artist / Test Album\n\
        DYEAR=2000\n\
        DGENRE=Jazz\n\
        TTITLE0=Long track\n\
        TTITLE0= title continuation\n\
        TTITLE1=Short track\n\
        .\n";

    let candidate = parse_xmcd(xmcd);
    assert_eq!(candidate.tracks.len(), 2);
    assert_eq!(candidate.tracks[0], "Long tracktitle continuation");
    assert_eq!(candidate.tracks[1], "Short track");
}
