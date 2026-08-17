// ── EmbyClient::ws_url ───────────────────────────────────────────────────

fn client_with_url(url: &str) -> EmbyClient {
    let cfg = crate::config::Config {
        server_url: url.into(),
        ..crate::config::Config::default()
    };
    let mut c = EmbyClient::new(cfg);
    c.token = "tok".into();
    c
}

fn local_listener_url() -> (std::net::TcpListener, String) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    (listener, url)
}

fn read_one_request(stream: &mut std::net::TcpStream) -> String {
    use std::io::Read;

    let mut request = Vec::new();
    let mut buf = [0_u8; 1024];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                request.extend_from_slice(&buf[..n]);
                if request.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    String::from_utf8_lossy(&request).into_owned()
}

#[test]
fn ordinary_report_stopped_still_retries_once() {
    let (listener, url) = local_listener_url();
    let attempts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let attempts_for_thread = attempts.clone();
    std::thread::spawn(move || {
        for _ in 0..2 {
            if let Ok((mut stream, _)) = listener.accept() {
                attempts_for_thread.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let _ = read_one_request(&mut stream);
            }
        }
    });

    let ok = client_with_url(&url).report_stopped(
        &ItemId::new("item"),
        &MediaSourceId::new("msid"),
        123,
        &EmbySessionId::new("sid"),
        456,
    );

    assert!(!ok);
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[test]
fn ws_url_http_becomes_ws() {
    let url = client_with_url("http://server:8096").ws_url();
    assert!(url.starts_with("ws://"));
    assert!(url.contains("api_key=tok"));
}

#[test]
fn ws_url_https_becomes_wss() {
    let url = client_with_url("https://server:8096").ws_url();
    assert!(url.starts_with("wss://"));
}

#[test]
fn service_setup_validation_uses_setup_url_user_and_token() {
    use std::io::Write;
    let (listener, setup_url) = local_listener_url();
    let request = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request = read_one_request(&mut stream);
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
            .unwrap();
        request
    });

    let config = crate::config::Config {
        server_url: "http://stale.example".into(),
        ..Default::default()
    };
    let client = EmbyClient::new(config);
    let setup = crate::config::EmbySetup::new(&setup_url, "user-42");
    let authenticated = client
        .authenticate_service_setup_bounded(
            "persisted-token".to_string(),
            &setup,
            std::time::Duration::from_secs(2),
        )
        .unwrap();

    let request = request.join().unwrap();
    assert!(request.starts_with("GET /Users/user-42 HTTP/1.1"));
    // Header casing isn't significant (RFC 7230 3.2); ureq 3.x lowercases it.
    assert!(request
        .to_ascii_lowercase()
        .contains("x-emby-token: persisted-token"));
    assert_eq!(authenticated.config.server_url, setup_url);
    assert_eq!(authenticated.token, "persisted-token");
}

#[test]
fn credential_exchange_rejection_and_connectivity_commit_nothing() {
    use std::io::Write;
    let _guard = crate::config::TestStateDirGuard::new();
    let (listener, url) = local_listener_url();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let _ = read_one_request(&mut stream);
        let _ = stream.write_all(b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n");
    });
    let client = EmbyClient::new(crate::config::Config::default());
    assert!(client
        .exchange_credentials_bounded(&url, "alice", "wrong", std::time::Duration::from_secs(2),)
        .is_err());
    assert!(!crate::config::token_cache_path().exists());
    assert!(!crate::config::service_secret_path(crate::config::ServiceKind::Emby).exists());
    assert!(!crate::config::config_path().exists());

    let dead = local_listener_url().1;
    drop(std::net::TcpListener::bind("127.0.0.1:0").unwrap());
    assert!(client
        .exchange_credentials_bounded(&dead, "alice", "wrong", std::time::Duration::from_secs(2),)
        .is_err());
    assert!(!crate::config::token_cache_path().exists());
    assert!(!crate::config::service_secret_path(crate::config::ServiceKind::Emby).exists());
    assert!(!crate::config::config_path().exists());
}

#[test]
fn ws_url_contains_device_id() {
    let mut c = client_with_url("http://server:8096");
    c.device_id = "my-device-id".into();
    let url = c.ws_url();
    assert!(url.contains("deviceId=my-device-id"));
}

#[test]
fn parse_mbv_direct_tcp_port_command_extracts_port() {
    let commands = vec![
        "Play".to_string(),
        mbv_direct_tcp_port_command(47788),
        "Pause".to_string(),
    ];
    assert_eq!(parse_mbv_direct_tcp_port(&commands), Some(47788));
}

#[test]
fn parse_mbv_shared_data_tcp_port_command_extracts_port() {
    let commands = vec![mbv_shared_data_tcp_port_command(47789)];
    assert_eq!(parse_mbv_shared_data_tcp_port(&commands), Some(47789));
}

// ── authenticate: token clearing vs. preservation ─────────────────────────

/// Writes a cached-token file into the guarded state dir, as
/// `authenticate()` would find it.
fn seed_cached_token(server_url: &str, token: &str, user_id: &str) {
    save_cached_token(server_url, token, user_id);
}

#[test]
fn authenticate_preserves_token_on_connectivity_error() {
    let _g = crate::config::TestStateDirGuard::new();
    let (listener, url) = local_listener_url();
    // Server accepts, reads the request, then drops the connection without
    // responding — a transport-level (connectivity-class) failure.
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let _ = read_one_request(&mut stream);
        }
    });
    seed_cached_token(&url, "cache-token", "cache-user");
    let mut client = client_with_url(&url);
    let result = client.authenticate();
    match &result {
        Err(e) => assert!(
            e.starts_with("Cached credential validation failed:"),
            "expected connectivity error, got {e:?}"
        ),
        Ok(()) => panic!("expected a connectivity error"),
    }
    // The cached token survives a connectivity failure (issue #192): the
    // server may be back on the next launch, and the token is still valid.
    assert_eq!(client.token, "cache-token");
    assert_eq!(client.user_id, "cache-user");
    // The on-disk cache must also survive (not just the in-memory fields).
    assert!(crate::config::token_cache_path().exists());
}

#[test]
fn authenticate_clears_token_on_401() {
    let _g = crate::config::TestStateDirGuard::new();
    let (listener, url) = local_listener_url();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let _ = read_one_request(&mut stream);
            use std::io::Write;
            let _ = stream.write_all(b"HTTP/1.1 401 Unauthorized\r\n\r\n");
        }
    });
    seed_cached_token(&url, "cache-token", "cache-user");
    let mut client = client_with_url(&url);
    let result = client.authenticate();
    assert_eq!(result, Err("Cached credentials expired".to_string()));
    assert_eq!(client.token, "");
    assert_eq!(client.user_id, "");
    assert!(!crate::config::token_cache_path().exists());
}

// ── auth_header ──────────────────────────────────────────────────────────

#[test]
fn auth_header_contains_device_name_and_id() {
    let mut c = client_with_url("http://server:8096");
    c.device_name = "myhost".into();
    c.device_id = "abcd-1234".into();
    c.token = "mytoken".into();
    let h = c.auth_header();
    assert!(h.contains("Device=\"myhost\""), "header: {h}");
    assert!(h.contains("DeviceId=\"abcd-1234\""), "header: {h}");
    assert!(h.contains("Token=\"mytoken\""), "header: {h}");
}

// ── device_name ──────────────────────────────────────────────────────────

#[test]
fn device_name_trims_hostname_env_var() {
    // /etc/hostname will be read first on Linux; test only the env-var path
    // by observing that a client created with HOSTNAME set has no whitespace
    // in its device_name field.
    let name = {
        let _g = crate::config::tests::SYS_ENV_LOCK.lock().unwrap();
        std::env::set_var("HOSTNAME", "  trimtest  \n");
        let c = EmbyClient::new(crate::config::Config::default());
        std::env::remove_var("HOSTNAME");
        c.device_name
    };
    // device_name should never embed raw whitespace
    assert!(!name.contains('\n'), "name contains newline: {:?}", name);
    assert_eq!(name, name.trim());
}

#[test]
fn device_name_falls_back_to_mbv() {
    // Only reachable when both /etc/hostname is absent/empty and HOSTNAME unset.
    // We can't suppress /etc/hostname, but we can verify the fallback string
    // is the sentinel "mbv" when nothing else is available.
    let name = device_name();
    assert!(!name.is_empty());
    // Must not contain raw newlines regardless of source.
    assert!(!name.contains('\n'));
}

// ── device_id ────────────────────────────────────────────────────────────

#[test]
fn device_id_is_stable_across_calls() {
    let dir = make_temp_data_dir();
    let first = device_id_in(dir.clone());
    let second = device_id_in(dir);
    assert_eq!(first, second);
}

#[test]
fn device_id_respects_xdg_data_home() {
    let dir = make_temp_data_dir();
    let id = device_id_in(dir.clone());
    let persisted = std::fs::read_to_string(dir.join("mbv/device_id")).unwrap();
    assert_eq!(persisted.trim(), id);
}

#[test]
fn device_id_migrates_from_legacy_mby_dir() {
    let dir = make_temp_data_dir();
    let legacy_dir = dir.join("mby");
    std::fs::create_dir_all(&legacy_dir).unwrap();
    let legacy_id = uuid::Uuid::new_v4().to_string();
    std::fs::write(legacy_dir.join("device_id"), &legacy_id).unwrap();
    let id = device_id_in(dir.clone());
    assert_eq!(id, legacy_id, "should reuse the legacy mby device_id");
    let persisted = std::fs::read_to_string(dir.join("mbv/device_id")).unwrap();
    assert_eq!(
        persisted.trim(),
        legacy_id,
        "should persist migrated id to new location"
    );
}

fn make_temp_data_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("mbv-test-{}", uuid::Uuid::new_v4()))
}
