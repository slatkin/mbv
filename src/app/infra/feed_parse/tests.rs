use super::{
    duration_secs, extract_atom_enclosure, extract_atom_link, extract_enclosure, extract_tag,
    infer_feed_kind_from_mime, normalize_feed_url, parse_atom_entries, parse_pub_date_secs,
    parse_rss_entries,
};
use mbv_core::config::FeedKind;

#[test]
fn extract_tag_unwraps_cdata_decodes_entities_and_strips_control_chars() {
    let cdata = "<item><title><![CDATA[Just a normal title]]></title></item>";
    assert_eq!(
        extract_tag(cdata, "title").as_deref(),
        Some("Just a normal title")
    );

    let entity = "<item><title>Fish &amp; Chips</title></item>";
    assert_eq!(
        extract_tag(entity, "title").as_deref(),
        Some("Fish & Chips")
    );

    let control_char = "<item><title>Evil\x1btitle\x07</title></item>";
    assert_eq!(
        extract_tag(control_char, "title").as_deref(),
        Some("Eviltitle")
    );
}

#[test]
fn extract_atom_link_decodes_and_sanitizes_href() {
    assert_eq!(
        extract_atom_link(r#"<entry><link href="https://example.test/a&amp;b" /></entry>"#)
            .as_deref(),
        Some("https://example.test/a&b")
    );
}

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
fn rss_entry_with_only_link_is_kept() {
    let item = r#"<item>
            <title>No enclosure</title>
            <link>https://example.test/post</link>
        </item>"#;
    let entries = parse_rss_entries(
        &format!("<channel>{item}</channel>"),
        FeedKind::Audio,
        "https://example.test/feed",
    );
    assert_eq!(entries.len(), 1);
    let e = &entries[0];
    assert_eq!(e.enclosure_url, None);
    assert_eq!(e.link.as_deref(), Some("https://example.test/post"));
    assert_eq!(e.guid, "https://example.test/post"); // guid falls back to the source
    assert_eq!(e.duration_ticks, None);
    assert_eq!(e.pub_date_secs, None);
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
    let item = r#"<item><title>No source at all</title></item>"#;
    let entries = parse_rss_entries(
        &format!("<channel>{item}</channel>"),
        FeedKind::Video,
        "https://example.test/feed",
    );
    assert!(entries.is_empty());
}

#[test]
fn extract_enclosure_reads_url_and_type() {
    let text = r#"<enclosure url="https://x.test/e.mp4" type="video/mp4" length="42"/>"#;
    let (url, mime) = extract_enclosure(text).unwrap();
    assert_eq!(url, "https://x.test/e.mp4");
    assert_eq!(mime.as_deref(), Some("video/mp4"));
    assert_eq!(extract_enclosure(r#"<enclosure type="audio/mpeg"/>"#), None);
}

#[test]
fn extract_atom_enclosure_skips_non_enclosure_links() {
    let text = r#"<link rel="alternate" href="https://x.test/a"/>
                      <link rel="enclosure" href="https://x.test/b.mp3" type="audio/mpeg"/>"#;
    let (url, mime) = extract_atom_enclosure(text).unwrap();
    assert_eq!(url, "https://x.test/b.mp3");
    assert_eq!(mime.as_deref(), Some("audio/mpeg"));
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

#[test]
fn pub_date_formats_parse() {
    // RFC 2822 with named zone and with numeric zone.
    assert_eq!(
        parse_pub_date_secs("Sat, 09 Aug 2026 12:00:00 +0000"),
        Some(1_786_276_800)
    );
    assert_eq!(
        parse_pub_date_secs("09 Aug 2026 07:00:00 -0500"),
        Some(1_786_276_800)
    );
    assert_eq!(
        parse_pub_date_secs("Sun, 14 Aug 2022 10:00:00 GMT"),
        Some(1_660_471_200)
    );
    // ISO 8601 with Z, explicit offset, and fractional seconds.
    assert_eq!(
        parse_pub_date_secs("2026-08-09T12:00:00Z"),
        Some(1_786_276_800)
    );
    assert_eq!(
        parse_pub_date_secs("2026-08-09T12:00:00+02:00"),
        Some(1_786_269_600)
    );
    assert_eq!(
        parse_pub_date_secs("2026-08-09T10:00:00.500-02:00"),
        Some(1_786_276_800)
    );
    assert_eq!(parse_pub_date_secs("not a date"), None);
}

#[test]
fn mime_inference_defaults_video() {
    assert_eq!(
        infer_feed_kind_from_mime(Some("audio/mpeg")),
        FeedKind::Audio
    );
    assert_eq!(
        infer_feed_kind_from_mime(Some("audio/mp4")),
        FeedKind::Audio
    );
    assert_eq!(
        infer_feed_kind_from_mime(Some("video/mp4")),
        FeedKind::Video
    );
    assert_eq!(infer_feed_kind_from_mime(None), FeedKind::Video);
    assert_eq!(
        infer_feed_kind_from_mime(Some("application/octet-stream")),
        FeedKind::Video
    );
}

#[test]
fn normalize_feed_url_pure_paths() {
    assert_eq!(
        normalize_feed_url("https://youtube.com/channel/UCabc").unwrap(),
        "https://www.youtube.com/feeds/videos.xml?channel_id=UCabc"
    );
    let feed = "https://www.youtube.com/feeds/videos.xml?channel_id=UCabc";
    assert_eq!(normalize_feed_url(feed).unwrap(), feed);
    let other = "https://example.com/feed.xml";
    assert_eq!(normalize_feed_url(other).unwrap(), other);
}
