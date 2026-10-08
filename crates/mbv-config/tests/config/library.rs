use mbv_config::parse_config;
use mbv_queue::FeedKind;

#[test]
fn parse_music_levels_group_album() {
    let toml = "[server]\nurl = \"http://host\"\n[library.music]\nlevels = [\"group\", \"album\"]";
    let cfg = parse_config(toml).unwrap();
    assert_eq!(cfg.music_levels, vec!["group", "album"]);
}

#[test]
fn parse_music_levels_album_only() {
    let toml = "[server]\nurl = \"http://host\"\n[library.music]\nlevels = [\"album\"]";
    let cfg = parse_config(toml).unwrap();
    assert_eq!(cfg.music_levels, vec!["album"]);
}

#[test]
fn parse_music_levels_missing_defaults_empty() {
    let toml = "[server]\nurl = \"http://host\"";
    assert_eq!(
        parse_config(toml).unwrap().music_levels,
        [] as [std::string::String; 0]
    );
}

// always_play_next and start_on_queue live in [queue].
#[test]
fn parse_always_play_next_in_wrong_section_is_ignored() {
    let toml = "[server]\nurl = \"http://host\"\nalways_play_next = true";
    assert!(
        !parse_config(toml).unwrap().always_play_next,
        "always_play_next must be in [queue], not [server]"
    );
}

#[test]
fn parse_feeds_basic() {
    let toml = r#"
[server]
url = "http://host"
[[feeds]]
name = "Nova"
url = "https://novaramedia.com/feed/"
kind = "video"
[[feeds]]
name = "Radio"
url = "https://example.com/podcast.xml"
kind = "audio"
"#;
    let cfg = parse_config(toml).unwrap();
    assert_eq!(cfg.feeds.len(), 2);
    assert_eq!(cfg.feeds[0].name, "Nova");
    assert_eq!(cfg.feeds[0].url, "https://novaramedia.com/feed/");
    assert_eq!(cfg.feeds[0].kind, FeedKind::Video);
    assert_eq!(cfg.feeds[1].name, "Radio");
    assert_eq!(cfg.feeds[1].kind, FeedKind::Audio);
}

#[test]
fn parse_feeds_tolerates_partial_rows() {
    let toml = r#"
[server]
url = "http://host"
[[feeds]]
name = "No URL"
[[feeds]]
url = "https://host.example/feed.xml"
[[feeds]]
name = "Odd kind"
url = "https://odd.example/rss"
kind = "podcast"
"#;
    let cfg = parse_config(toml).unwrap();
    // URL-less row skipped; name-less row falls back to the host;
    // unknown kind defaults to Video.
    assert_eq!(cfg.feeds.len(), 2);
    assert_eq!(cfg.feeds[0].name, "host.example");
    assert_eq!(cfg.feeds[0].kind, FeedKind::Video);
    assert_eq!(cfg.feeds[1].name, "Odd kind");
    assert_eq!(cfg.feeds[1].kind, FeedKind::Video);
}

#[test]
fn parse_feeds_without_server_section() {
    // No [server] at all: feeds still parse, everything else defaults.
    let toml = r#"
[[feeds]]
name = "Only"
url = "https://only.example/feed"
"#;
    let cfg = parse_config(toml).unwrap();
    assert_eq!(cfg.server_url, "");
    assert_eq!(cfg.hidden_libraries, vec!["live tv"]);
    assert_eq!(cfg.feeds.len(), 1);
    assert_eq!(cfg.feeds[0].name, "Only");
}

#[test]
fn parse_no_feeds_defaults_empty() {
    let cfg = parse_config("[server]\nurl = \"http://host\"").unwrap();
    assert!(cfg.feeds.is_empty());
}
