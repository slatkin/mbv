use super::*;
use mbv_net::mock_http::MockHttp;

/// A URL that never leaves the process: the mock transport ignores it, but it
/// must remain an IP literal so the default resolver never attempts DNS.
const TEST_URL: &str = "http://127.0.0.1:1";

fn client_with_url(url: &str) -> EmbyClient {
    let cfg = mbv_config::Config {
        server_url: url.into(),
        ..mbv_config::Config::default()
    };
    let mut c = EmbyClient::new(cfg);
    c.token = "tok".into();
    c
}

// ── authenticate: token clearing vs. preservation ─────────────────────────

/// Writes a cached-token file into the guarded state dir, as
/// `authenticate()` would find it.
fn seed_cached_token(server_url: &str, token: &str, user_id: &str) {
    save_cached_token(server_url, token, user_id);
}

#[test]
fn authenticate_preserves_token_on_connectivity_error() {
    let _g = mbv_config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let agent = http.agent();
    // Connection dropped without responding — a transport-level
    // (connectivity-class) failure.
    http.fail(std::io::ErrorKind::UnexpectedEof);
    seed_cached_token(TEST_URL, "cache-token", "cache-user");
    let mut client = client_with_url(TEST_URL).with_test_agent(agent);
    let result = client.authenticate();
    match &result {
        Err(e) => {
            assert!(
                e.to_string()
                    .starts_with("Cached credential validation failed:"),
                "expected connectivity error, got {e:?}"
            );
            assert!(e.is_auth());
        }
        Ok(()) => panic!("expected a connectivity error"),
    }
    // The cached token survives a connectivity failure (issue #192): the
    // server may be back on the next launch, and the token is still valid.
    assert_eq!(client.token, "cache-token");
    assert_eq!(client.user_id, "cache-user");
    // The on-disk cache must also survive (not just the in-memory fields).
    assert!(mbv_config::token_cache_path().exists());
}

#[test]
fn authenticate_clears_token_on_401() {
    let _g = mbv_config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let agent = http.agent();
    http.respond(401, "");
    seed_cached_token(TEST_URL, "cache-token", "cache-user");
    let mut client = client_with_url(TEST_URL).with_test_agent(agent);
    let result = client.authenticate();
    assert_eq!(
        result.unwrap_err().to_string(),
        "Cached credentials expired"
    );
    assert_eq!(client.token, "");
    assert_eq!(client.user_id, "");
    assert!(!mbv_config::token_cache_path().exists());
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
