use super::*;
use rstest::rstest;
use serde_json::json;

// ── EmbyClient::ws_url ───────────────────────────────────────────────────

use mbv_net::mock_http::MockHttp;

/// A URL that never leaves the process: the mock transport ignores it, but it
/// must remain an IP literal so the default resolver never attempts DNS.
const TEST_URL: &str = "http://127.0.0.1:1";

fn client_with_url(url: &str) -> EmbyClient {
    let cfg = crate::config::Config {
        server_url: url.into(),
        ..crate::config::Config::default()
    };
    let mut c = EmbyClient::new(cfg);
    c.token = "tok".into();
    c
}

fn mock_client(url: &str) -> (EmbyClient, MockHttp) {
    let http = MockHttp::new();
    let agent = http.agent();
    (client_with_url(url).with_test_agent(agent), http)
}

fn session_response(playable_media_types: Option<serde_json::Value>) -> String {
    let mut session = json!({
        "Id": "session-id",
        "DeviceId": "other-device",
        "DeviceName": "Other device",
        "Client": "Emby",
        "UserName": "user",
        "SupportsRemoteControl": true,
    });
    if let Some(playable_media_types) = playable_media_types {
        session["PlayableMediaTypes"] = playable_media_types;
    }
    json!([session]).to_string()
}

fn parse_session_playable_media_types(
    playable_media_types: Option<serde_json::Value>,
) -> Vec<String> {
    let (mut client, http) = mock_client(TEST_URL);
    client.device_id = "this-device".into();
    let body = session_response(playable_media_types);
    http.respond(200, &body);
    client
        .get_sessions_unfiltered()
        .unwrap()
        .pop()
        .expect("mock session must be retained")
        .playable_media_types
}

#[rstest]
#[case::present_audio_and_video(
    Some(json!(["Audio", "Video"])),
    vec!["Audio".to_string(), "Video".to_string()]
)]
#[case::absent(None, Vec::new())]
#[case::empty(Some(json!([])), Vec::new())]
fn sessions_parse_playable_media_types(
    #[case] playable_media_types: Option<serde_json::Value>,
    #[case] expected: Vec<String>,
) {
    assert_eq!(
        parse_session_playable_media_types(playable_media_types),
        expected
    );
}

#[test]
fn upcoming_request_scopes_to_library_and_parses_episodes() {
    let (mut client, http) = mock_client(TEST_URL);
    client.user_id = "user".into();
    http.respond(
        200,
        r#"{"Items":[{"Id":"episode-1","Name":"Episode One","Type":"Episode","MediaType":"Video","SeriesId":"series-1","SeriesName":"Series One","IndexNumber":2,"ParentIndexNumber":1,"DateCreated":"2026-09-22T00:00:00Z"}]}"#,
    );

    let episodes = client.get_upcoming("library-7", 30).unwrap();
    let request = &http.requests()[0];
    assert!(request.starts_with("GET /Shows/Upcoming?"));
    assert!(request.contains("ParentId=library-7"));
    assert!(request.contains("Limit=30"));
    assert!(request.contains("Fields="));
    assert_eq!(episodes.len(), 1);
    assert_eq!(episodes[0].id, "episode-1");
    assert_eq!(episodes[0].item_type, "Episode");
    assert_eq!(episodes[0].series_id, "series-1");
}

#[test]
fn library_items_request_includes_external_urls_field() {
    let (mut client, http) = mock_client(TEST_URL);
    client.user_id = "user".into();
    http.respond(200, r#"{"Items":[],"TotalRecordCount":0}"#);
    client
        .get_items_sorted("library", None, false, 0, 10, "SortName", "Ascending")
        .unwrap();
    assert!(http.requests()[0].contains("Fields="));
    assert!(http.requests()[0].contains("ExternalUrls"));
    assert!(http.requests()[0].contains("ProviderIds"));
}

#[test]
fn library_items_request_includes_artist_items_field() {
    // Task 1.2: the Music album/item browse request must explicitly ask for
    // `ArtistItems` so parsed items retain stable artist identity pairs.
    let (mut client, http) = mock_client(TEST_URL);
    client.user_id = "user".into();
    http.respond(200, r#"{"Items":[],"TotalRecordCount":0}"#);
    client
        .get_items_sorted(
            "library",
            Some("MusicAlbum"),
            false,
            0,
            10,
            "SortName",
            "Ascending",
        )
        .unwrap();
    let request = &http.requests()[0];
    assert!(request.contains("Fields="));
    assert!(request.contains("ArtistItems"));
}

#[test]
fn artist_audio_tracks_request_uses_artist_ids_query() {
    // Task 1.4: the only artist-track query mbv issues. The path stays on
    // the user-items endpoint and the ID enters solely as `ArtistIds`;
    // mbv has no `/Artists` listing request anywhere to source IDs from.
    let (mut client, http) = mock_client(TEST_URL);
    client.user_id = "user".into();
    http.respond(
        200,
        r#"{"Items":[{"Id":"track-1","Name":"Song One","Type":"Audio","MediaType":"Audio"},{"Id":"track-2","Name":"Song Two","Type":"Audio","MediaType":"Audio"}],"TotalRecordCount":2}"#,
    );
    let tracks = client.get_artist_audio_tracks("artist-9").unwrap();
    let request = &http.requests()[0];
    assert!(request.starts_with("GET /Users/user/Items?"));
    assert!(request.contains("ArtistIds=artist-9"));
    assert!(request.contains("IncludeItemTypes=Audio"));
    assert!(request.contains("Recursive=true"));
    assert!(!request.contains("/Artists"));
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0].id, "track-1");
    assert_eq!(tracks[1].name, "Song Two");
}

#[test]
fn artist_audio_tracks_error_propagates() {
    // Unsupported or rejected artist-ID queries surface as Err so the
    // caller falls back to per-album aggregation (design D7).
    let (mut client, http) = mock_client(TEST_URL);
    client.user_id = "user".into();
    http.respond(500, r#"{"error":"unsupported"}"#);
    client.get_artist_audio_tracks("artist-9").unwrap_err();
}

#[test]
fn continue_watching_request_enables_user_data() {
    let (mut client, http) = mock_client(TEST_URL);
    client.user_id = "user".into();
    http.respond(200, r#"{"Items":[]}"#);
    client.get_continue_watching(10).unwrap();
    assert!(http.requests()[0].contains("EnableUserData=true"));
}

#[test]
fn playlist_items_request_includes_external_urls_field() {
    let (mut client, http) = mock_client(TEST_URL);
    client.user_id = "user".into();
    http.respond(200, r#"{"Items":[]}"#);
    client.get_playlist_items("playlist").unwrap();
    assert!(http.requests()[0].contains("Fields="));
    assert!(http.requests()[0].contains("ExternalUrls"));
    assert!(http.requests()[0].contains("ProviderIds"));
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
    let http = MockHttp::new();
    let agent = http.agent();
    http.respond(200, "");

    let config = crate::config::Config {
        server_url: "http://stale.example".into(),
        ..Default::default()
    };
    let client = EmbyClient::new(config).with_test_agent(agent);
    let setup = crate::config::EmbySetup::new(TEST_URL, "user-42");
    let authenticated = client
        .authenticate_service_setup_bounded(
            "persisted-token".to_string(),
            &setup,
            std::time::Duration::from_secs(2),
        )
        .unwrap();

    let request = &http.requests()[0];
    assert!(request.starts_with("GET /Users/user-42 HTTP/1.1"));
    // Header casing isn't significant (RFC 7230 3.2); ureq 3.x lowercases it.
    assert!(request
        .to_ascii_lowercase()
        .contains("x-emby-token: persisted-token"));
    assert_eq!(authenticated.config.server_url, TEST_URL);
    assert_eq!(authenticated.token, "persisted-token");
}

#[test]
fn credential_exchange_rejection_and_connectivity_commit_nothing() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let agent = http.agent();
    http.respond(401, "");
    http.fail(std::io::ErrorKind::ConnectionRefused);
    let client = EmbyClient::new(crate::config::Config::default()).with_test_agent(agent);
    client
        .exchange_credentials_bounded(
            TEST_URL,
            "alice",
            "wrong",
            std::time::Duration::from_secs(2),
        )
        .unwrap_err();
    assert!(!crate::config::token_cache_path().exists());
    assert!(!crate::config::service_secret_path(crate::config::ServiceKind::Emby).exists());
    assert!(!crate::config::config_path().exists());

    client
        .exchange_credentials_bounded(
            TEST_URL,
            "alice",
            "wrong",
            std::time::Duration::from_secs(2),
        )
        .unwrap_err();
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

// ── authenticate: token clearing vs. preservation ─────────────────────────

/// Writes a cached-token file into the guarded state dir, as
/// `authenticate()` would find it.
fn seed_cached_token(server_url: &str, token: &str, user_id: &str) {
    save_cached_token(server_url, token, user_id);
}

#[test]
fn authenticate_preserves_token_on_connectivity_error() {
    let _g = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let agent = http.agent();
    // Connection dropped without responding — a transport-level
    // (connectivity-class) failure.
    http.fail(std::io::ErrorKind::UnexpectedEof);
    seed_cached_token(TEST_URL, "cache-token", "cache-user");
    let mut client = client_with_url(TEST_URL).with_test_agent(agent);
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
    let http = MockHttp::new();
    let agent = http.agent();
    http.respond(401, "");
    seed_cached_token(TEST_URL, "cache-token", "cache-user");
    let mut client = client_with_url(TEST_URL).with_test_agent(agent);
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
    assert!(!name.contains('\n'), "name contains newline: {name:?}");
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

// ── get_playback_info_for_cast (task 4.2) ─────────────────────────────────

#[test]
fn get_playback_info_for_cast_direct_play_uses_the_standard_stream_url() {
    let (client, http) = mock_client(TEST_URL);
    http.respond(
        200,
        &json!({"MediaSources": [{"Id": "msid1", "SupportsDirectPlay": true}]}).to_string(),
    );
    let profile = crate::cast::dispatch::build_cast_device_profile(
        crate::cast::dispatch::CastSubtitleKind::None,
    );
    let info = client
        .get_playback_info_for_cast("item123", false, &profile)
        .unwrap();
    assert_eq!(
        info.item.url,
        format!("{TEST_URL}/Videos/item123/stream?static=true&api_key=tok&MediaSourceId=msid1")
    );
    assert_eq!(info.item.content_type, "video/mp4");
    assert_eq!(info.media_source_id.as_str(), "msid1");
}

#[test]
fn get_playback_info_for_cast_transcode_uses_the_server_supplied_url() {
    let (client, http) = mock_client(TEST_URL);
    http.respond(
        200,
        &json!({"MediaSources": [{
            "Id": "msid2",
            "SupportsDirectPlay": false,
            "TranscodingUrl": "/videos/item123/master.m3u8?DeviceId=x",
        }]})
        .to_string(),
    );
    let profile = crate::cast::dispatch::build_cast_device_profile(
        crate::cast::dispatch::CastSubtitleKind::None,
    );
    let info = client
        .get_playback_info_for_cast("item123", false, &profile)
        .unwrap();
    assert_eq!(
        info.item.url,
        format!("{TEST_URL}/videos/item123/master.m3u8?DeviceId=x")
    );
    assert_eq!(info.item.content_type, "application/vnd.apple.mpegurl");
}

#[test]
fn get_playback_info_for_cast_failed_request_is_an_error() {
    let (client, http) = mock_client(TEST_URL);
    http.fail(std::io::ErrorKind::UnexpectedEof);
    let profile = crate::cast::dispatch::build_cast_device_profile(
        crate::cast::dispatch::CastSubtitleKind::None,
    );
    client
        .get_playback_info_for_cast("item123", false, &profile)
        .unwrap_err();
}
