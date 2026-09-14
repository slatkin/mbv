use super::*;
use std::time::{Duration, Instant};

use crate::mock_http::MockHttp;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/audiobookshelf/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn mock_client(responses: Vec<(u16, String)>) -> (AudiobookshelfClient, MockHttp) {
    let http = MockHttp::new();
    for (status, body) in responses {
        http.respond(status, &body);
    }
    let client = AudiobookshelfClient::new("http://127.0.0.1:1")
        .unwrap()
        .with_test_agent(http.agent());
    (client, http)
}

fn session_error(
    client: &AudiobookshelfClient,
    api_key: &str,
    hard_bound: Duration,
) -> AudiobookshelfError {
    client
        .create_playback_session_bounded(
            api_key,
            "device",
            "<LIBRARY_ITEM_ID>",
            "<EPISODE_ID>",
            false,
            hard_bound,
        )
        .unwrap_err()
}

#[test]
fn captured_sessions_decode_and_validate_requested_single_podcast_track() {
    for (name, path, method) in [
        (
            "play-direct.json",
            "/direct.mp3",
            AudiobookshelfSourceMethod::Direct,
        ),
        (
            "play-transcode.json",
            "/hls/session/index.m3u8",
            AudiobookshelfSourceMethod::Hls,
        ),
    ] {
        let body = fixture(name)
            .replace("<DIRECT_PATH>", path)
            .replace("<HLS_PATH>", path);
        let (client, _) = mock_client(vec![(200, body)]);
        let session = client
            .create_playback_session_bounded(
                "secret",
                "device",
                "<LIBRARY_ITEM_ID>",
                "<EPISODE_ID>",
                method == AudiobookshelfSourceMethod::Hls,
                Duration::from_secs(1),
            )
            .unwrap();
        assert_eq!(session.source.method, method);
        assert_eq!(session.current_time_seconds, 1.0);
        assert_eq!(session.source.url, format!("http://127.0.0.1:1{path}"));
    }

    let bodies = [
        fixture("play-direct.json").replace("<DIRECT_PATH>", "/direct.mp3").replace(
            "\"episodeId\": \"<EPISODE_ID>\"",
            "\"episodeId\": \"other\"",
        ),
        fixture("play-direct.json").replace("<DIRECT_PATH>", "/direct.mp3").replace(
            "\"audioTracks\": [{",
            "\"audioTracks\": [{\"duration\":1,\"contentUrl\":\"/two\",\"mimeType\":\"audio/mpeg\"},{",
        ),
    ];
    for body in bodies {
        // The second response is never consumed: the protocol check rejects
        // the first response before any second request is made.
        let (client, _) = mock_client(vec![(200, body), (200, "{}".into())]);
        let error = session_error(&client, "secret", Duration::from_secs(1));
        assert_eq!(error.class, AudiobookshelfFailureClass::Protocol);
    }
}

#[test]
fn session_requests_are_bearer_post_json_and_bounded() {
    let direct = fixture("play-direct.json").replace("<DIRECT_PATH>", "/direct.mp3");
    let (client, http) = mock_client(vec![
        (200, direct),
        (200, fixture("session-sync.json")),
        (200, fixture("session-close.json")),
    ]);
    let session = client
        .create_playback_session_bounded(
            "secret",
            "device",
            "<LIBRARY_ITEM_ID>",
            "<EPISODE_ID>",
            false,
            Duration::from_secs(1),
        )
        .unwrap();
    let progress = AudiobookshelfPlaybackProgress {
        current_time: 1.0,
        time_listened: 1.0,
        duration: session.duration_seconds,
    };
    client
        .sync_playback_session_bounded("secret", &session.id, progress, Duration::from_secs(1))
        .unwrap();
    client
        .close_playback_session_bounded("secret", &session.id, progress, Duration::from_secs(1))
        .unwrap();

    let captured = http.requests();
    assert_eq!(captured.len(), 3);
    for request in &captured {
        // Header name casing is not significant per RFC 7230 3.2, and ureq
        // 3.x lowercases header names on the wire (2.x sent them as-set).
        let lower = request.to_ascii_lowercase();
        assert!(lower.contains("authorization: bearer secret\r\n"));
        assert!(lower.contains("content-type: application/json\r\n"));
    }
    assert!(captured[0]
        .starts_with("POST /api/items/%3CLIBRARY_ITEM_ID%3E/play/%3CEPISODE_ID%3E HTTP/1.1"));
    // ureq 3.x's send_json pretty-prints the body (2.x sent compact JSON);
    // strip whitespace before matching so both formats pass.
    let no_ws = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    assert!(no_ws(&captured[0]).contains("\"deviceId\":\"device\""));
    assert!(captured[1].starts_with("POST /api/session/%3CSESSION_ID%3E/sync HTTP/1.1"));
    assert!(captured[2].starts_with("POST /api/session/%3CSESSION_ID%3E/close HTTP/1.1"));
    assert!(no_ws(&captured[1])
        .contains("\"currentTime\":1.0,\"timeListened\":1.0,\"duration\":3054.336"));

    let (client, _) = mock_client(vec![(401, fixture("authentication-failure.json"))]);
    let error = session_error(&client, "do-not-leak", Duration::from_secs(1));
    assert_eq!(
        error.class,
        AudiobookshelfFailureClass::AuthenticationRejected
    );
    assert!(!format!("{error:?}").contains("do-not-leak"));

    // A stalled server: the hard bound wins and classifies as connectivity.
    let http = MockHttp::new();
    let agent = http.agent();
    http.stall(Duration::from_secs(3600));
    let client = AudiobookshelfClient::new("http://127.0.0.1:1")
        .unwrap()
        .with_test_agent(agent);
    let started = Instant::now();
    let error = session_error(&client, "secret", Duration::from_millis(20));
    assert_eq!(error.class, AudiobookshelfFailureClass::Connectivity);
    assert!(started.elapsed() < Duration::from_millis(150));
}

#[test]
fn playback_failures_and_rest_only_hls_readiness_are_classified() {
    for (status, body, class) in [
        (
            500,
            fixture("server-failure.json"),
            AudiobookshelfFailureClass::Server,
        ),
        (
            200,
            fixture("malformed-response.txt"),
            AudiobookshelfFailureClass::MalformedResponse,
        ),
    ] {
        let (client, _) = mock_client(vec![(status, body)]);
        let error = session_error(&client, "secret", Duration::from_secs(1));
        assert_eq!(error.class, class);
    }

    let http = MockHttp::new();
    http.respond(200, "#EXTM3U\n#EXT-X-VERSION:3\n");
    let client = AudiobookshelfClient::new("http://127.0.0.1:1")
        .unwrap()
        .with_test_agent(http.agent());
    let base = "http://127.0.0.1:1";
    client
        .wait_for_hls_ready_bounded(
            &format!("{base}/hls/session/index.m3u8"),
            Duration::from_secs(1),
        )
        .unwrap();
    let request = &http.requests()[0];
    assert!(request.starts_with("GET /hls/session/index.m3u8 HTTP/1.1"));
    assert!(!request.to_ascii_lowercase().contains("authorization:"));
}

#[test]
fn late_success_after_create_bound_is_closed_on_loopback() {
    let http = MockHttp::new();
    let agent = http.agent();
    // The create response arrives far too late for the 5ms bound; the next
    // scripted response serves the cleanup close.
    http.delayed(
        Duration::from_millis(50),
        &fixture("play-direct.json").replace("<DIRECT_PATH>", "/direct.mp3"),
    );
    http.respond(200, &fixture("session-close.json"));
    let client = AudiobookshelfClient::new("http://127.0.0.1:1")
        .unwrap()
        .with_test_agent(agent);

    let error = session_error(&client, "secret", Duration::from_millis(5));
    assert_eq!(error.class, AudiobookshelfFailureClass::Connectivity);
    // The cleanup close is sent by the abandoned worker once it observes the
    // bound was missed; poll for it rather than assuming it is already there.
    let deadline = Instant::now() + Duration::from_secs(1);
    let close = loop {
        let mut requests = http.requests();
        if requests.len() >= 2 {
            break requests.remove(1);
        }
        assert!(Instant::now() < deadline, "cleanup close was never sent");
        std::thread::sleep(Duration::from_millis(5));
    };
    assert!(close.starts_with("POST /api/session/%3CSESSION_ID%3E/close HTTP/1.1"));
}
