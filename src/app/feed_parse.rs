use super::types_feed::IdleFeedItem;
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::config::FeedKind;
use mbv_core::playback_queue::FeedEntry;
use std::sync::Arc;

fn fetch_feed_body(url: &str) -> Result<String, String> {
    tls_agent()?
        .get(url)
        .call()
        .map_err(|e| format!("HTTP request failed: {e}"))?
        .into_string()
        .map_err(|e| format!("Failed to read response body: {e}"))
}
fn tls_agent() -> Result<ureq::Agent, String> {
    let connector =
        native_tls::TlsConnector::new().map_err(|e| format!("Failed to initialize TLS: {e}"))?;
    Ok(ureq::AgentBuilder::new()
        .tls_connector(Arc::new(connector))
        .build())
}
pub(super) fn normalize_feed_url(input: &str) -> Result<String, String> {
    let Some((host, path_and_query)) = url_authority_and_path(input) else {
        return Ok(input.to_string());
    };
    if !matches!(host, "youtube.com" | "www.youtube.com" | "m.youtube.com") {
        return Ok(input.to_string());
    }

    let (path, query) = path_and_query
        .split_once('?')
        .map_or((path_and_query, ""), |(path, query)| (path, query));
    if path == "/feeds/videos.xml"
        && query.split('&').any(|param| {
            param
                .strip_prefix("channel_id=")
                .is_some_and(|id| !id.is_empty())
        })
    {
        return Ok(input.to_string());
    }

    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    if segments.len() == 2 && segments[0] == "channel" && segments[1].starts_with("UC") {
        return Ok(format!(
            "https://www.youtube.com/feeds/videos.xml?channel_id={}",
            segments[1]
        ));
    }

    if (segments.len() == 1 && segments[0].starts_with('@') && segments[0].len() > 1)
        || (segments.len() == 2 && matches!(segments[0], "c" | "user") && !segments[1].is_empty())
    {
        let body = tls_agent()?
            .get(input)
            .call()
            .map_err(|e| format!("Failed to resolve YouTube channel: {e}"))?
            .into_string()
            .map_err(|e| format!("Failed to read YouTube channel page: {e}"))?;
        return extract_rss_link(&body)
            .ok_or_else(|| "YouTube channel page did not contain an RSS feed URL".to_string());
    }

    Err("URL is not a resolvable YouTube channel URL".to_string())
}

fn url_authority_and_path(input: &str) -> Option<(&str, &str)> {
    let rest = input
        .strip_prefix("https://")
        .or_else(|| input.strip_prefix("http://"))?;
    let (authority, path) = rest.split_once('/')?;
    let host = authority.split('@').next()?.split(':').next()?;
    Some((host, &rest[rest.len() - path.len() - 1..]))
}

fn extract_rss_link(body: &str) -> Option<String> {
    for link in body.split("<link").skip(1) {
        let Some(end) = link.find('>') else {
            continue;
        };
        let tag = &link[..end];
        if extract_attribute(tag, "rel").as_deref() == Some("alternate")
            && extract_attribute(tag, "type").as_deref() == Some("application/rss+xml")
        {
            return extract_attribute(tag, "href");
        }
    }
    None
}

pub(super) fn fetch_and_parse_rss(url: &str) -> Result<Vec<IdleFeedItem>, String> {
    let body = fetch_feed_body(url)?;

    let mut items = Vec::new();

    if let Some(start) = body.find("<item>") {
        let rest = &body[start..];
        for item_match in rest.split("<item>").skip(1) {
            let title = extract_tag(item_match, "title");
            let link = extract_tag(item_match, "link");
            if let Some(title) = title {
                items.push(IdleFeedItem { title, link });
            }
        }
    }

    if items.is_empty() {
        if let Some(start) = body.find("<entry>") {
            let rest = &body[start..];
            for entry_match in rest.split("<entry>").skip(1) {
                let title = extract_tag(entry_match, "title");
                let link = extract_atom_link(entry_match);
                if let Some(title) = title {
                    items.push(IdleFeedItem { title, link });
                }
            }
        }
    }

    Ok(items)
}

/// Fetch an RSS/Atom feed and parse each `<item>`/`<entry>` into a
/// [`FeedEntry`] (guid, title, enclosure URL, MIME type, duration → ticks).
///
/// `subscription_kind` is the subscription's `FeedKind` used as the
/// canonical fallback when enclosure MIME is absent or unrecognized.
pub(super) fn fetch_and_parse_entries(
    url: &str,
    subscription_kind: FeedKind,
) -> Result<Vec<FeedEntry>, String> {
    let body = fetch_feed_body(url)?;
    let mut entries = parse_rss_entries(&body, subscription_kind);
    if entries.is_empty() {
        entries = parse_atom_entries(&body, subscription_kind);
    }
    Ok(entries)
}

fn parse_rss_entries(body: &str, subscription_kind: FeedKind) -> Vec<FeedEntry> {
    let mut entries = Vec::new();
    let Some(start) = body.find("<item>") else {
        return entries;
    };
    for item in body[start..].split("<item>").skip(1) {
        let Some(title) = extract_tag(item, "title") else {
            continue;
        };
        let link = extract_tag(item, "link");
        let enclosure = extract_enclosure(item);
        let enclosure_url = enclosure.as_ref().map(|(url, _)| url.clone());
        let mime_type = enclosure.and_then(|(_, mime)| mime);
        let Some(source) = enclosure_url.as_deref().or(link.as_deref()) else {
            continue;
        };
        let guid = extract_tag(item, "guid").unwrap_or_else(|| source.to_string());
        let duration_ticks = extract_tag(item, "itunes:duration")
            .and_then(|d| duration_secs(&d))
            .map(|secs| secs * TICKS_PER_SECOND as u64);
        let pub_date_secs = extract_tag(item, "pubDate").and_then(|d| parse_pub_date_secs(&d));
        entries.push(FeedEntry {
            guid,
            title,
            enclosure_url,
            link,
            mime_type,
            duration_ticks,
            pub_date_secs,
            feed_kind: Some(subscription_kind),
        });
    }
    entries
}

fn parse_atom_entries(body: &str, subscription_kind: FeedKind) -> Vec<FeedEntry> {
    let mut entries = Vec::new();
    let Some(start) = body.find("<entry>") else {
        return entries;
    };
    for entry in body[start..].split("<entry>").skip(1) {
        let Some(title) = extract_tag(entry, "title") else {
            continue;
        };
        let link = extract_atom_link(entry);
        let enclosure = extract_atom_enclosure(entry);
        let enclosure_url = enclosure.as_ref().map(|(url, _)| url.clone());
        let mime_type = enclosure.as_ref().and_then(|(_, mime)| mime.clone());
        let Some(source) = enclosure_url.as_deref().or(link.as_deref()) else {
            continue;
        };
        let guid = extract_tag(entry, "id").unwrap_or_else(|| source.to_string());
        let duration_ticks = extract_tag(entry, "itunes:duration")
            .or_else(|| extract_tag(entry, "duration"))
            .and_then(|d| duration_secs(&d))
            .map(|secs| secs * TICKS_PER_SECOND as u64);
        let pub_date_secs = extract_tag(entry, "published")
            .or_else(|| extract_tag(entry, "updated"))
            .and_then(|d| parse_pub_date_secs(&d));
        entries.push(FeedEntry {
            guid,
            title,
            enclosure_url,
            link,
            mime_type,
            duration_ticks,
            pub_date_secs,
            feed_kind: Some(subscription_kind),
        });
    }
    entries
}

/// Infer the media kind of a feed entry from its enclosure MIME type;
/// absent or unrecognized values default to Video. Keep in one helper so
/// #472 can reuse it.
#[cfg(test)]
pub(super) fn infer_feed_kind_from_mime(mime: Option<&str>) -> FeedKind {
    match mime.map(|m| m.trim().to_ascii_lowercase()) {
        Some(m) if m.starts_with("audio/") => FeedKind::Audio,
        _ => FeedKind::Video,
    }
}

/// The first `<enclosure ...>` element's `url` and `type` attributes
/// (RSS), sanitized. None when no enclosure parses.
fn extract_enclosure(text: &str) -> Option<(String, Option<String>)> {
    let mut rest = text;
    while let Some(pos) = rest.find("<enclosure") {
        let tag = &rest[pos..];
        let len = tag.find('>')?;
        let tag_text = &tag[..len];
        rest = &tag[len + 1..];
        if let Some(url) = extract_attribute(tag_text, "url") {
            return Some((url, extract_attribute(tag_text, "type")));
        }
    }
    None
}

/// The first Atom `<link rel="enclosure">` element's `href` and `type`
/// attributes, scanning past any alternate/self links.
fn extract_atom_enclosure(text: &str) -> Option<(String, Option<String>)> {
    let mut rest = text;
    while let Some(pos) = rest.find("<link") {
        let tag = &rest[pos..];
        let len = tag.find('>')?;
        let tag_text = &tag[..len];
        rest = &tag[len + 1..];
        if extract_attribute(tag_text, "rel").as_deref() != Some("enclosure") {
            continue;
        }
        if let Some(url) = extract_attribute(tag_text, "href") {
            return Some((url, extract_attribute(tag_text, "type")));
        }
    }
    None
}

/// Extract the value of an `attr="..."` attribute from a single tag's
/// text (entities decoded, control chars stripped, trimmed).
fn extract_attribute(text: &str, attr: &str) -> Option<String> {
    let needle = format!(r#"{attr}=""#);
    let start = text.find(&needle)? + needle.len();
    let end = text[start..].find('"')?;
    let value = &text[start..start + end];
    let decoded = decode_xml_entities(value);
    let sanitized = strip_control_chars(&decoded);
    Some(sanitized.trim().to_string())
}

/// Parse an `itunes:duration` value ("HH:MM:SS", "MM:SS", or bare
/// seconds) into whole seconds; anything else yields None.
fn duration_secs(text: &str) -> Option<u64> {
    let parts: Vec<&str> = text.trim().split(':').collect();
    match parts.as_slice() {
        [s] => s.parse().ok(),
        [m, s] => {
            let m: u64 = m.parse().ok()?;
            let s: u64 = s.parse().ok()?;
            if s > 59 {
                return None;
            }
            Some(m * 60 + s)
        }
        [h, m, s] => {
            let h: u64 = h.parse().ok()?;
            let m: u64 = m.parse().ok()?;
            let s: u64 = s.parse().ok()?;
            if m > 59 || s > 59 {
                return None;
            }
            Some(h * 3600 + m * 60 + s)
        }
        _ => None,
    }
}

/// Parse an RSS `pubDate` (RFC 2822) or Atom `published`/`updated`
/// (ISO 8601) timestamp into unix seconds UTC; anything else yields None
/// (missing dates sort last in the Feeds tab "All" group).
fn parse_pub_date_secs(text: &str) -> Option<u64> {
    let t = text.trim();
    let t = t
        .strip_suffix('Z')
        .or_else(|| t.strip_suffix('z'))
        .unwrap_or(t);
    // ISO dates are Y-M-D based (at least two dashes); RFC 2822 dates are
    // month-name based. A plain `contains('T')` is not enough — "GMT"
    // zones and clock times contain "T"s too.
    if t.matches('-').count() >= 2 {
        parse_iso_date(t)
    } else {
        parse_rfc2822_date(t)
    }
}

/// RFC 2822: "Sat, 09 Aug 2026 12:00:00 +0000" (weekday optional).
fn parse_rfc2822_date(text: &str) -> Option<u64> {
    let rest = text.split_once(',').map(|(_, r)| r).unwrap_or(text).trim();
    let mut parts = rest.split_whitespace();
    let day: u32 = parts.next()?.parse().ok()?;
    let month: u32 = match parts.next()? {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => return None,
    };
    let year: i64 = parts.next()?.parse().ok()?;
    let mut clock = parts.next()?.split(':');
    let hour: u32 = clock.next()?.parse().ok()?;
    let minute: u32 = clock.next()?.parse().ok()?;
    let second: u32 = clock.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    if year < 1970 || month > 12 || day == 0 || day > 31 || hour > 23 || minute > 59 || second > 60
    {
        return None;
    }
    let offset_secs = parts.next().map(parse_zone_offset).unwrap_or(0);
    Some(
        (days_from_civil(year, month, day) * 86_400
            + hour as i64 * 3600
            + minute as i64 * 60
            + second as i64
            - offset_secs) as u64,
    )
}

/// ISO 8601: "2026-08-09T12:00:00Z" / "…+02:00" / "…+0200", optional
/// fractional seconds.
fn parse_iso_date(text: &str) -> Option<u64> {
    let (date_part, time_part) = text.split_once('T')?;
    let mut dp = date_part.split('-');
    let year: i64 = dp.next()?.parse().ok()?;
    let month: u32 = dp.next()?.parse().ok()?;
    let day: u32 = dp.next()?.parse().ok()?;
    if year < 1970 || month == 0 || month > 12 || day == 0 || day > 31 {
        return None;
    }
    let (time, offset_secs) = match time_part.find(['+', '-']) {
        Some(idx) => {
            let (rest, offset) = time_part.split_at(idx);
            (rest, parse_iso_offset(offset)?)
        }
        None => (time_part, 0),
    };
    let mut clock = time.split('.').next()?.split(':');
    let hour: u32 = clock.next()?.parse().ok()?;
    let minute: u32 = clock.next()?.parse().ok()?;
    let second: u32 = clock.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    Some(
        (days_from_civil(year, month, day) * 86_400
            + hour as i64 * 3600
            + minute as i64 * 60
            + second as i64
            - offset_secs) as u64,
    )
}

/// "+02:00" / "-0500" style UTC offsets in seconds (east positive).
fn parse_iso_offset(s: &str) -> Option<i64> {
    let (sign, rest) = s.split_at(1);
    let (hour, minute): (i64, i64) = if let Some((h, m)) = rest.split_once(':') {
        (h.parse().ok()?, m.parse().ok()?)
    } else if rest.len() >= 4 {
        (rest[..2].parse().ok()?, rest[2..4].parse().ok()?)
    } else {
        (rest.parse().ok()?, 0)
    };
    let secs = hour * 3600 + minute * 60;
    if sign == "-" {
        Some(-secs)
    } else {
        Some(secs)
    }
}

/// RFC 2822 numeric ("+0500" / "-0700") and named zones ("GMT", "UTC",
/// "EST", …); unknown names are treated as UTC.
fn parse_zone_offset(s: &str) -> i64 {
    if let Some(hour) = s.strip_prefix('+') {
        let h: i64 = hour[..2].parse().unwrap_or(0);
        let m: i64 = hour.get(2..4).and_then(|m| m.parse().ok()).unwrap_or(0);
        return h * 3600 + m * 60;
    }
    if let Some(hour) = s.strip_prefix('-') {
        let h: i64 = hour[..2].parse().unwrap_or(0);
        let m: i64 = hour.get(2..4).and_then(|m| m.parse().ok()).unwrap_or(0);
        return -(h * 3600 + m * 60);
    }
    match s {
        "GMT" | "UTC" | "UT" | "Z" => 0,
        "EST" => -5 * 3600,
        "EDT" => -4 * 3600,
        "CST" => -6 * 3600,
        "CDT" => -5 * 3600,
        "MST" => -7 * 3600,
        "MDT" => -6 * 3600,
        "PST" => -8 * 3600,
        "PDT" => -7 * 3600,
        _ => 0,
    }
}

/// Days since 1970-01-01 for a proleptic-Gregorian date (Howard Hinnant's
/// civil algorithm).
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = ((month + 9) % 12) as i64;
    let doy = (153 * mp + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719_468
}

/// Extract the first `<tag>...</tag>` content from text.
fn extract_tag(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = text.find(&open)? + open.len();
    let end = text[start..].find(&close)?;
    let content = &text[start..start + end];
    // Strip any nested tags (e.g. CDATA wrappers), decode XML entities, then
    // strip control characters that entity-decoding could otherwise
    // reintroduce, before finally trimming.
    let stripped = strip_tags(content);
    let decoded = decode_xml_entities(&stripped);
    let sanitized = strip_control_chars(&decoded);
    Some(sanitized.trim().to_string())
}

/// Extract the `href` attribute from the first `<link` element in Atom format.
fn extract_atom_link(text: &str) -> Option<String> {
    let link_start = text.find("<link")?;
    let link_end = text[link_start..].find('>')?;
    let link_tag = &text[link_start..link_start + link_end + 1];
    let href_start = link_tag.find("href=\"")? + 6;
    let href_end = link_tag[href_start..].find('"')?;
    let href = &link_tag[href_start..href_start + href_end];
    let decoded = decode_xml_entities(href);
    let sanitized = strip_control_chars(&decoded);
    Some(sanitized.trim().to_string())
}

/// Strip XML/HTML tags from text, treating `<![CDATA[...]]>` sections as raw
/// text rather than markup (their contents are copied verbatim, without
/// further tag-stripping).
fn strip_tags(text: &str) -> String {
    const CDATA_OPEN: &str = "<![CDATA[";
    const CDATA_CLOSE: &str = "]]>";

    let mut result = String::new();
    let mut rest = text;
    while let Some(cdata_start) = rest.find(CDATA_OPEN) {
        result.push_str(&strip_tags_no_cdata(&rest[..cdata_start]));
        let after_open = &rest[cdata_start + CDATA_OPEN.len()..];
        match after_open.find(CDATA_CLOSE) {
            Some(cdata_end) => {
                result.push_str(&after_open[..cdata_end]);
                rest = &after_open[cdata_end + CDATA_CLOSE.len()..];
            }
            None => {
                // Unterminated CDATA: treat the rest as raw content.
                result.push_str(after_open);
                rest = "";
                break;
            }
        }
    }
    result.push_str(&strip_tags_no_cdata(rest));
    result
}

/// Strip ordinary `<...>` tags from text (no CDATA awareness).
fn strip_tags_no_cdata(text: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result
}

/// Decode common XML entities (`&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;`)
/// and numeric character references (`&#NNN;` / `&#xHHHH;`) in a single
/// left-to-right scan. Anything unrecognized (e.g. a stray `&`) is left
/// untouched rather than erroring.
fn decode_xml_entities(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp_idx) = rest.find('&') {
        result.push_str(&rest[..amp_idx]);
        let tail = &rest[amp_idx..];
        let Some(semi_idx) = tail.find(';') else {
            result.push('&');
            rest = &tail[1..];
            continue;
        };
        let entity = &tail[1..semi_idx];
        let decoded_char = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ if entity.starts_with('#') => {
                let num_part = &entity[1..];
                let code_point = if let Some(hex) = num_part
                    .strip_prefix('x')
                    .or_else(|| num_part.strip_prefix('X'))
                {
                    u32::from_str_radix(hex, 16).ok()
                } else {
                    num_part.parse::<u32>().ok()
                };
                code_point.and_then(char::from_u32)
            }
            _ => None,
        };
        match decoded_char {
            Some(ch) => {
                result.push(ch);
                rest = &tail[semi_idx + 1..];
            }
            None => {
                // Unrecognized entity: leave the leading '&' untouched and
                // keep scanning from just after it.
                result.push('&');
                rest = &tail[1..];
            }
        }
    }
    result.push_str(rest);
    result
}

/// Filter out control characters (C0/C1, including ESC and BEL) from feed
/// text before it can reach the terminal, e.g. via an OSC 8 escape sequence.
fn strip_control_chars(text: &str) -> String {
    text.chars().filter(|ch| !ch.is_control()).collect()
}

#[cfg(test)]
mod tests {
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
        let entries = parse_rss_entries(&format!("<channel>{item}</channel>"), FeedKind::Audio);
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
        let entries = parse_rss_entries(&format!("<channel>{item}</channel>"), FeedKind::Audio);
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
        let entries = parse_rss_entries(&format!("<channel>{item}</channel>"), FeedKind::Video);
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
        let entries = parse_atom_entries(&format!("<feed>{entry}</feed>"), FeedKind::Audio);
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
        let entries = parse_rss_entries(&format!("<channel>{item}</channel>"), FeedKind::Video);
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
}
