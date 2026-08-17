use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/audiobookshelf/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn serve(responses: Vec<(u16, String)>) -> (String, std::sync::mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for (status, body) in responses {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0; 4096];
            loop {
                let read = stream.read(&mut chunk).unwrap_or(0);
                bytes.extend_from_slice(&chunk[..read]);
                let text = String::from_utf8_lossy(&bytes);
                let Some(header_end) = text.find("\r\n\r\n") else {
                    continue;
                };
                let content_length = text[..header_end]
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .and_then(|value| value.parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if bytes.len() >= header_end + 4 + content_length {
                    break;
                }
            }
            let request = String::from_utf8(bytes).unwrap();
            let reason = if status == 200 { "OK" } else { "Error" };
            write!(
                stream,
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            let _ = tx.send(request);
        }
    });
    (format!("http://{address}"), rx)
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
        let (base, _) = serve(vec![(200, body)]);
        let client = AudiobookshelfClient::new(&base).unwrap();
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
        assert_eq!(session.source.url, format!("{base}{path}"));
    }

    for body in [
        fixture("play-direct.json").replace("<DIRECT_PATH>", "/direct.mp3").replace(
            "\"episodeId\": \"<EPISODE_ID>\"",
            "\"episodeId\": \"other\"",
        ),
        fixture("play-direct.json").replace("<DIRECT_PATH>", "/direct.mp3").replace(
            "\"audioTracks\": [{",
            "\"audioTracks\": [{\"duration\":1,\"contentUrl\":\"/two\",\"mimeType\":\"audio/mpeg\"},{",
        ),
    ] {
        let (base, _) = serve(vec![(200, body), (200, "{}".into())]);
        let error = AudiobookshelfClient::new(&base)
            .unwrap()
            .create_playback_session_bounded(
                "secret",
                "device",
                "<LIBRARY_ITEM_ID>",
                "<EPISODE_ID>",
                false,
                Duration::from_secs(1),
            )
            .unwrap_err();
        assert_eq!(error.class, AudiobookshelfFailureClass::Protocol);
    }
}

#[test]
fn session_requests_are_bearer_post_json_and_bounded() {
    let direct = fixture("play-direct.json").replace("<DIRECT_PATH>", "/direct.mp3");
    let (base, requests) = serve(vec![
        (200, direct),
        (200, fixture("session-sync.json")),
        (200, fixture("session-close.json")),
    ]);
    let client = AudiobookshelfClient::new(&base).unwrap();
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

    let captured: Vec<_> = (0..3).map(|_| requests.recv().unwrap()).collect();
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

    let (base, _) = serve(vec![(401, fixture("authentication-failure.json"))]);
    let error = AudiobookshelfClient::new(&base)
        .unwrap()
        .create_playback_session_bounded(
            "do-not-leak",
            "device",
            "item",
            "episode",
            false,
            Duration::from_secs(1),
        )
        .unwrap_err();
    assert_eq!(
        error.class,
        AudiobookshelfFailureClass::AuthenticationRejected
    );
    assert!(!format!("{error:?}").contains("do-not-leak"));

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        let (_stream, _) = listener.accept().unwrap();
        std::thread::sleep(Duration::from_millis(200));
    });
    let started = Instant::now();
    let error = AudiobookshelfClient::new(&base)
        .unwrap()
        .create_playback_session_bounded(
            "secret",
            "device",
            "item",
            "episode",
            false,
            Duration::from_millis(20),
        )
        .unwrap_err();
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
        let (base, _) = serve(vec![(status, body)]);
        let error = AudiobookshelfClient::new(&base)
            .unwrap()
            .create_playback_session_bounded(
                "secret",
                "device",
                "item",
                "episode",
                false,
                Duration::from_secs(1),
            )
            .unwrap_err();
        assert_eq!(error.class, class);
    }

    let (base, request) = serve(vec![(200, "#EXTM3U\n#EXT-X-VERSION:3\n".into())]);
    AudiobookshelfClient::new(&base)
        .unwrap()
        .wait_for_hls_ready_bounded(
            &format!("{base}/hls/session/index.m3u8"),
            Duration::from_secs(1),
        )
        .unwrap();
    let request = request.recv().unwrap();
    assert!(request.starts_with("GET /hls/session/index.m3u8 HTTP/1.1"));
    assert!(!request.to_ascii_lowercase().contains("authorization:"));
}

#[test]
fn late_success_after_create_bound_is_closed_on_loopback() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (closed_tx, closed_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let (mut create, _) = listener.accept().unwrap();
        let mut request = [0; 4096];
        let _ = create.read(&mut request).unwrap();
        std::thread::sleep(Duration::from_millis(30));
        let body = fixture("play-direct.json").replace("<DIRECT_PATH>", "/direct.mp3");
        write!(
            create,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();

        let (mut close, _) = listener.accept().unwrap();
        let read = close.read(&mut request).unwrap();
        closed_tx
            .send(String::from_utf8_lossy(&request[..read]).into_owned())
            .unwrap();
        let response = fixture("session-close.json");
        write!(
            close,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
            response.len()
        )
        .unwrap();
    });

    let error = AudiobookshelfClient::new(&base)
        .unwrap()
        .create_playback_session_bounded(
            "secret",
            "device",
            "<LIBRARY_ITEM_ID>",
            "<EPISODE_ID>",
            false,
            Duration::from_millis(5),
        )
        .unwrap_err();
    assert_eq!(error.class, AudiobookshelfFailureClass::Connectivity);
    let close = closed_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(close.starts_with("POST /api/session/%3CSESSION_ID%3E/close HTTP/1.1"));
}
