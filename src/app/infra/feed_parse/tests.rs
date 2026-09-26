use super::{duration_secs, parse_atom_entries, parse_rss_entries};
use mbv_core::config::FeedKind;

#[test]
fn rss_entry_with_enclosure_guid_and_duration() {
    let item = r#"<item>
            <guid>ep-42</guid>
            <title>Episode 42</title>
            <link>https://example.test/ep-42</link>
            <enclosure url="https://example.test/ep-42.mp3" type="audio/mpeg" length="12345"/>
            <itunes:duration>01:02:03</itunes:duration>
            <pubDate>Sat, 09 Aug 2026 12:00:00 +0000</pubDate>
        </item>"#;
    let entries = parse_rss_entries(
        &format!("<channel>{item}</channel>"),
        FeedKind::Audio,
        "https://example.test/feed",
    );
    assert_eq!(entries.len(), 1);
    let e = &entries[0];
    assert_eq!(e.guid, "ep-42");
    assert_eq!(e.title, "Episode 42");
    assert_eq!(
        e.enclosure_url.as_deref(),
        Some("https://example.test/ep-42.mp3")
    );
    assert_eq!(e.mime_type.as_deref(), Some("audio/mpeg"));
    assert_eq!(e.duration_ticks, Some((3723) * 10_000_000));
    assert_eq!(e.pub_date_secs, Some(1_786_276_800));
}

#[test]
fn malformed_duration_yields_none_without_failing_the_feed() {
    let item = r#"<item>
            <guid>g1</guid>
            <title>Bad duration</title>
            <enclosure url="https://example.test/a.mp4" type="video/mp4"/>
            <itunes:duration>not-a-duration</itunes:duration>
        </item>
        <item>
            <guid>g2</guid>
            <title>Good entry after bad</title>
            <enclosure url="https://example.test/b.mp4" type="video/mp4"/>
        </item>"#;
    let entries = parse_rss_entries(
        &format!("<channel>{item}</channel>"),
        FeedKind::Video,
        "https://example.test/feed",
    );
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].duration_ticks, None);
    assert_eq!(entries[1].guid, "g2");
}

#[test]
fn atom_entry_with_enclosure_and_published() {
    let entry = r#"<entry>
            <id>tag:example.test,2026:ep7</id>
            <title>Atom Episode</title>
            <link rel="alternate" href="https://example.test/ep7"/>
            <link rel="enclosure" href="https://example.test/ep7.m4a" type="audio/mp4"/>
            <published>2026-08-09T12:00:00Z</published>
            <updated>2026-08-09T13:00:00Z</updated>
        </entry>"#;
    let entries = parse_atom_entries(
        &format!("<feed>{entry}</feed>"),
        FeedKind::Audio,
        "https://example.test/feed",
    );
    assert_eq!(entries.len(), 1);
    let e = &entries[0];
    assert_eq!(e.guid, "tag:example.test,2026:ep7");
    assert_eq!(
        e.enclosure_url.as_deref(),
        Some("https://example.test/ep7.m4a")
    );
    assert_eq!(e.mime_type.as_deref(), Some("audio/mp4"));
    assert_eq!(e.link.as_deref(), Some("https://example.test/ep7"));
    // `published` wins over `updated`.
    assert_eq!(e.pub_date_secs, Some(1_786_276_800));
}

#[test]
fn entries_without_any_source_are_skipped() {
    let item = r"<item><title>No source at all</title></item>";
    let entries = parse_rss_entries(
        &format!("<channel>{item}</channel>"),
        FeedKind::Video,
        "https://example.test/feed",
    );
    assert!(entries.is_empty());
}

#[test]
fn duration_formats_parse_and_garbage_does_not() {
    assert_eq!(duration_secs("3723"), Some(3723));
    assert_eq!(duration_secs("62:03"), Some(62 * 60 + 3));
    assert_eq!(duration_secs("01:02:03"), Some(3723));
    assert_eq!(duration_secs("1:2:3"), Some(3723));
    assert_eq!(duration_secs("01:99:00"), None);
    assert_eq!(duration_secs("abc"), None);
    assert_eq!(duration_secs("1:2:3:4"), None);
}
