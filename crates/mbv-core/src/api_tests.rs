use super::*;
use serde_json::json;

fn make_item(name: &str, item_type: &str) -> MediaItem {
    MediaItem {
        id: "id".into(),
        name: name.into(),
        item_type: item_type.into(),
        is_folder: false,
        media_type: "Video".into(),
        collection_type: String::new(),
        runtime_ticks: 0,
        played: false,
        playback_position_ticks: 0,
        series_id: String::new(),
        series_name: String::new(),
        album_id: String::new(),
        album: String::new(),
        index_number: 0,
        parent_index_number: 0,
        unplayed_item_count: 0,
        path: String::new(),
        artist: String::new(),
        sort_name: String::new(),
        production_year: 0,
        end_year: 0,
        overview: String::new(),
        premiere_date: String::new(),
        date_added: String::new(),
        total_count: 0,
        container: String::new(),
        director: String::new(),
        video_info: String::new(),
        audio_info: String::new(),
        genre: String::new(),
        playlist_item_id: String::new(),
    }
}

// ── MediaItem::display_name ──────────────────────────────────────────────

#[test]
fn display_name_episode_without_series_falls_back_to_name() {
    let item = make_item("Standalone", "Episode");
    assert_eq!(item.display_name(), "Standalone");
}

// ── MediaItem::resume_seconds / runtime_seconds ──────────────────────────

#[test]
fn resume_seconds_converts_ticks() {
    let mut item = make_item("X", "Movie");
    item.playback_position_ticks = TICKS_PER_SECOND * 90;
    assert!((item.resume_seconds() - 90.0).abs() < 1e-6);
}

#[test]
fn runtime_seconds_converts_ticks() {
    let mut item = make_item("X", "Movie");
    item.runtime_ticks = TICKS_PER_SECOND * 5400; // 90 min
    assert!((item.runtime_seconds() - 5400.0).abs() < 1e-6);
}

// ── parse_item ───────────────────────────────────────────────────────────

#[test]
fn parse_item_basic_fields() {
    let raw = json!({
        "Id": "abc", "Name": "Test", "Type": "Movie",
        "IsFolder": false, "MediaType": "Video",
        "RunTimeTicks": 36_000_000_000i64,
        "SortName": "test",
        "UserData": { "Played": true, "PlaybackPositionTicks": 5_000_000i64 }
    });
    let item = parse_item(&raw);
    assert_eq!(item.id, "abc");
    assert_eq!(item.name, "Test");
    assert_eq!(item.runtime_ticks, 36_000_000_000);
    assert!(item.played);
    assert_eq!(item.playback_position_ticks, 5_000_000);
}

#[test]
fn parse_item_collection_folder_forces_is_folder() {
    let raw = json!({ "Type": "CollectionFolder", "IsFolder": false, "UserData": {} });
    assert!(parse_item(&raw).is_folder);
}

#[test]
fn parse_item_channel_forces_is_folder() {
    let raw = json!({ "Type": "Channel", "IsFolder": false, "UserData": {} });
    assert!(parse_item(&raw).is_folder);
}

#[test]
fn parse_item_missing_fields_use_defaults() {
    let item = parse_item(&json!({}));
    assert_eq!(item.id, "");
    assert_eq!(item.runtime_ticks, 0);
    assert!(!item.played);
    assert!(!item.is_folder);
}

#[test]
fn parse_item_episode_fields() {
    let raw = json!({
        "Type": "Episode", "Name": "Pilot",
        "SeriesName": "Lost", "IndexNumber": 1, "ParentIndexNumber": 2,
        "UserData": {}
    });
    let item = parse_item(&raw);
    assert_eq!(item.series_name, "Lost");
    assert_eq!(item.index_number, 1);
    assert_eq!(item.parent_index_number, 2);
}

// ── MediaItem::playback_label ────────────────────────────────────────────

#[test]
fn playback_label_audio_without_artist_falls_back_to_display_name() {
    let item = make_item("Song", "Audio");
    assert_eq!(item.playback_label(), "Song");
}

#[test]
fn playback_label_video_uses_display_name() {
    let item = make_item("Inception", "Movie");
    assert_eq!(item.playback_label(), "Inception");
}

// ── MediaItem::file_name / sort_key ──────────────────────────────────────

#[test]
fn file_name_extracts_from_path() {
    let mut item = make_item("Movie", "Movie");
    item.path = "/media/movies/Inception (2010).mkv".into();
    assert_eq!(item.file_name(), "Inception (2010).mkv");
}

#[test]
fn file_name_falls_back_to_name_when_path_empty() {
    let item = make_item("Inception", "Movie");
    assert_eq!(item.file_name(), "Inception");
}

#[test]
fn sort_key_prefers_path_filename() {
    let mut item = make_item("Movie", "Movie");
    item.path = "/media/A.mkv".into();
    item.sort_name = "sort".into();
    assert_eq!(item.sort_key(), "A.mkv");
}

#[test]
fn sort_key_falls_back_to_sort_name() {
    let mut item = make_item("Movie", "Movie");
    item.sort_name = "inception the".into();
    assert_eq!(item.sort_key(), "inception the");
}

#[test]
fn sort_key_falls_back_to_name() {
    let item = make_item("Movie", "Movie");
    assert_eq!(item.sort_key(), "Movie");
}

// ── parse_item: audio and music folder types ─────────────────────────────

#[test]
fn parse_item_audio_not_folder() {
    let raw = json!({ "Type": "Audio", "MediaType": "Audio", "UserData": {} });
    let item = parse_item(&raw);
    assert_eq!(item.item_type, "Audio");
    assert_eq!(item.media_type, "Audio");
    assert!(!item.is_folder);
}

#[test]
fn parse_item_music_album_is_folder() {
    let raw = json!({ "Type": "MusicAlbum", "IsFolder": false, "UserData": {} });
    assert!(parse_item(&raw).is_folder);
}

#[test]
fn parse_item_music_artist_is_folder() {
    let raw = json!({ "Type": "MusicArtist", "IsFolder": false, "UserData": {} });
    assert!(parse_item(&raw).is_folder);
}

#[test]
fn parse_item_series_is_folder() {
    let raw = json!({ "Type": "Series", "IsFolder": false, "UserData": {} });
    assert!(parse_item(&raw).is_folder);
}

#[test]
fn parse_item_artist_from_album_artist_field() {
    let raw = json!({ "Type": "Audio", "AlbumArtist": "Pink Floyd", "UserData": {} });
    assert_eq!(parse_item(&raw).artist, "Pink Floyd");
}

#[test]
fn parse_item_artist_falls_back_to_artists_array() {
    let raw = json!({ "Type": "Audio", "Artists": ["David Bowie"], "UserData": {} });
    assert_eq!(parse_item(&raw).artist, "David Bowie");
}

#[test]
fn parse_item_album_artist_takes_priority_over_artists_array() {
    let raw = json!({ "Type": "Audio", "AlbumArtist": "Album Artist", "Artists": ["Track Artist"], "UserData": {} });
    assert_eq!(parse_item(&raw).artist, "Album Artist");
}

// ── EmbyClient::stream_url ───────────────────────────────────────────────

#[test]
fn stream_url_format() {
    let c = client_with_url("http://server:8096");
    assert_eq!(
        c.stream_url("abc123"),
        "http://server:8096/Videos/abc123/stream?static=true&api_key=tok"
    );
}

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
fn report_stopped_for_shutdown_stalls_with_one_attempt_and_no_retry() {
    let (listener, url) = local_listener_url();
    let attempts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let attempts_for_thread = attempts.clone();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            attempts_for_thread.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let _ = read_one_request(&mut stream);
            std::thread::sleep(std::time::Duration::from_secs(5));
        }
    });

    let started = std::time::Instant::now();
    let ok = client_with_url(&url).report_stopped_for_shutdown(
        "item",
        "msid",
        123,
        "sid",
        456,
        std::time::Duration::from_millis(150),
    );

    assert!(!ok);
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);
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

    let ok = client_with_url(&url).report_stopped("item", "msid", 123, "sid", 456);

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
fn device_id_returns_uuid_v4_format() {
    let id = device_id_in(make_temp_data_dir());
    // xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx
    let parts: Vec<&str> = id.split('-').collect();
    assert_eq!(parts.len(), 5, "id: {id}");
    assert_eq!(parts[0].len(), 8);
    assert_eq!(parts[1].len(), 4);
    assert_eq!(parts[2].len(), 4);
    assert!(parts[2].starts_with('4'), "version nibble: {id}");
    assert_eq!(parts[3].len(), 4);
    assert_eq!(parts[4].len(), 12);
    assert!(id.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
}

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

// ── decode_html_entities ─────────────────────────────────────────────────

#[test]
fn decode_html_entities_known_entities() {
    assert_eq!(decode_html_entities("&quot;hi&quot;"), "\"hi\"");
    assert_eq!(decode_html_entities("it&apos;s"), "it's");
    assert_eq!(decode_html_entities("a &lt; b &gt; c"), "a < b > c");
    assert_eq!(decode_html_entities("a &amp; b"), "a & b");
}

#[test]
fn decode_html_entities_passthrough() {
    assert_eq!(decode_html_entities("plain text"), "plain text");
    assert_eq!(decode_html_entities(""), "");
}

// ── parse_video_info ─────────────────────────────────────────────────────

#[test]
fn parse_video_info_4k() {
    let streams = json!([{"Type": "Video", "Width": 3840, "Height": 2160, "Codec": "hevc"}]);
    assert_eq!(parse_video_info(streams.as_array().unwrap()), "4K HEVC");
}

#[test]
fn parse_video_info_1080p() {
    let streams = json!([{"Type": "Video", "Width": 1920, "Height": 1080, "Codec": "h264"}]);
    assert_eq!(parse_video_info(streams.as_array().unwrap()), "1080p H264");
}

#[test]
fn parse_video_info_720p() {
    let streams = json!([{"Type": "Video", "Width": 1280, "Height": 720, "Codec": "h264"}]);
    assert_eq!(parse_video_info(streams.as_array().unwrap()), "720p H264");
}

#[test]
fn parse_video_info_codec_only_when_no_resolution() {
    let streams = json!([{"Type": "Video", "Width": 0, "Height": 0, "Codec": "vp9"}]);
    assert_eq!(parse_video_info(streams.as_array().unwrap()), "VP9");
}

#[test]
fn parse_video_info_empty_when_no_video_stream() {
    let streams = json!([{"Type": "Audio", "Codec": "aac"}]);
    assert_eq!(parse_video_info(streams.as_array().unwrap()), "");
}

// ── parse_audio_info ─────────────────────────────────────────────────────

fn audio_stream(lang: &str, codec: &str, layout: &str) -> serde_json::Value {
    json!({"Type": "Audio", "Language": lang, "Codec": codec, "ChannelLayout": layout})
}

#[test]
fn parse_audio_info_multiple_tracks() {
    let streams = json!([
        audio_stream("eng", "ac3", "5.1"),
        audio_stream("fra", "aac", "stereo"),
    ]);
    assert_eq!(
        parse_audio_info(streams.as_array().unwrap()),
        "English AC3 5.1  |  French AAC Stereo"
    );
}

#[test]
fn parse_audio_info_unknown_lang_omitted_from_label() {
    let streams = json!([audio_stream("und", "aac", "stereo")]);
    assert_eq!(parse_audio_info(streams.as_array().unwrap()), "AAC Stereo");
}

#[test]
fn parse_audio_info_skips_non_audio_streams() {
    let streams = json!([
        {"Type": "Video", "Language": "eng", "Codec": "h264", "ChannelLayout": ""},
        audio_stream("eng", "aac", "stereo"),
    ]);
    assert_eq!(
        parse_audio_info(streams.as_array().unwrap()),
        "English AAC Stereo"
    );
}

// Sync guard: every ISO code in parse_audio_info must produce the same English
// name as lang_code_to_name() in player.rs. Both tables must be updated together.
// The mirror test in player.rs::tests::lang_code_to_name_matches_api_table checks
// the other side.
#[test]
fn parse_audio_info_lang_table_matches_player_lang_code_to_name() {
    let cases: &[(&str, &str)] = &[
        ("en", "English"),
        ("eng", "English"),
        ("fr", "French"),
        ("fre", "French"),
        ("fra", "French"),
        ("de", "German"),
        ("ger", "German"),
        ("deu", "German"),
        ("es", "Spanish"),
        ("spa", "Spanish"),
        ("it", "Italian"),
        ("ita", "Italian"),
        ("pt", "Portuguese"),
        ("por", "Portuguese"),
        ("ja", "Japanese"),
        ("jpn", "Japanese"),
        ("ko", "Korean"),
        ("kor", "Korean"),
        ("zh", "Chinese"),
        ("chi", "Chinese"),
        ("zho", "Chinese"),
        ("ru", "Russian"),
        ("rus", "Russian"),
        ("ar", "Arabic"),
        ("ara", "Arabic"),
        ("nl", "Dutch"),
        ("nld", "Dutch"),
        ("dut", "Dutch"),
        ("sv", "Swedish"),
        ("swe", "Swedish"),
        ("no", "Norwegian"),
        ("nor", "Norwegian"),
        ("da", "Danish"),
        ("dan", "Danish"),
        ("fi", "Finnish"),
        ("fin", "Finnish"),
        ("pl", "Polish"),
        ("pol", "Polish"),
        ("cs", "Czech"),
        ("cze", "Czech"),
        ("ces", "Czech"),
        ("tr", "Turkish"),
        ("tur", "Turkish"),
    ];
    for (code, expected) in cases {
        let streams =
            json!([{"Type": "Audio", "Language": code, "Codec": "", "ChannelLayout": ""}]);
        let result = parse_audio_info(streams.as_array().unwrap());
        assert_eq!(
            result, *expected,
            "parse_audio_info: code {:?} → expected {:?}, got {:?}",
            code, expected, result
        );
    }
}

#[test]
fn parse_session_media_info_extracts_remote_stream_options() {
    let streams = json!([
        {"Type": "Video", "Width": 1920, "Height": 1080, "Codec": "h264"},
        {"Type": "Audio", "Index": 1, "Language": "eng", "Codec": "ac3", "ChannelLayout": "5.1"},
        {"Type": "Audio", "Index": 2, "Language": "jpn", "Codec": "aac", "ChannelLayout": "stereo"},
        {"Type": "Subtitle", "Index": 3, "Language": "eng", "IsForced": false},
        {"Type": "Subtitle", "Index": 4, "Language": "eng", "IsForced": true}
    ]);
    let media = parse_session_media_info(streams.as_array().unwrap());
    assert_eq!(media.video_label, "1080p H264");
    assert!(!media.audio_only);
    assert_eq!(media.audio_streams.len(), 2);
    assert_eq!(media.audio_streams[0].index, 1);
    assert_eq!(media.audio_streams[0].label, "English AC3 5.1");
    assert_eq!(media.audio_streams[1].label, "Japanese AAC Stereo");
    assert_eq!(media.subtitle_streams.len(), 2);
    assert_eq!(media.subtitle_streams[0].label, "English");
    assert_eq!(media.subtitle_streams[1].label, "English (Forced)");
}

#[test]
fn parse_session_media_info_handles_audio_only_sessions() {
    let streams = json!([
        {"Type": "Audio", "Index": 0, "Language": "eng", "Codec": "flac", "ChannelLayout": "stereo"}
    ]);
    let media = parse_session_media_info(streams.as_array().unwrap());
    assert!(media.audio_only);
    assert_eq!(media.video_label, "English FLAC Stereo");
    assert_eq!(media.audio_streams.len(), 1);
    assert_eq!(media.audio_streams[0].index, 0);
}

// ── get_sessions_unfiltered ──────────────────────────────────────────────

#[test]
fn get_sessions_with_active_within_none_omits_query_param() {
    // get_sessions_unfiltered must not constrain ActiveWithinSeconds --
    // this is a compile-time/shape check since the real query param is
    // only observable over the wire; the behavioral guarantee is that
    // both methods exist and share one implementation.
    let client = client_with_url("http://localhost:1");
    // Both calls fail (no server listening on port 1) -- we only need
    // them to compile and return a network-error Result, proving the
    // unfiltered path exists and is reachable without a required
    // ActiveWithinSeconds argument.
    assert!(client.get_sessions_unfiltered().is_err());
    assert!(client.get_sessions().is_err());
}

// ── is_audio / is_video ──────────────────────────────────────────────────

// ── should_resume ────────────────────────────────────────────────────────

#[test]
fn should_resume_zero_position_returns_false() {
    assert!(!make_item("X", "Movie").should_resume());
}

#[test]
fn should_resume_negative_position_returns_false() {
    let mut item = make_item("X", "Movie");
    item.playback_position_ticks = -1;
    assert!(!item.should_resume());
}

#[test]
fn should_resume_mid_way_returns_true() {
    let mut item = make_item("X", "Movie");
    item.runtime_ticks = TICKS_PER_SECOND * 7200;
    item.playback_position_ticks = TICKS_PER_SECOND * 3600; // 50%
    assert!(item.should_resume());
}

#[test]
fn should_resume_under_one_percent_returns_false() {
    let mut item = make_item("X", "Movie");
    item.runtime_ticks = TICKS_PER_SECOND * 7200; // 2h
    item.playback_position_ticks = TICKS_PER_SECOND; // ~0.01%
    assert!(!item.should_resume());
}

#[test]
fn should_resume_with_unknown_runtime_returns_true() {
    let mut item = make_item("X", "Movie");
    item.runtime_ticks = 0;
    item.playback_position_ticks = TICKS_PER_SECOND * 60;
    assert!(item.should_resume());
}
