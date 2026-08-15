use super::types_feed::IdleFeedItem;
use mbv_core::api::{decode_entities, TICKS_PER_SECOND};
use mbv_core::config::FeedKind;
use mbv_core::playback_queue::FeedEntry;
use std::sync::Arc;

use super::feed_parse_date::parse_pub_date_secs;

fn fetch_feed_body(url: &str) -> Result<String, String> {
    tls_agent()?
        .get(url)
        .call()
        .map_err(|e| format!("HTTP request failed: {e}"))?
        .into_string()
        .map_err(|e| format!("Failed to read response body: {e}"))
}

/// `ureq::get` never picks up the `native-tls` feature on its own — it only
/// activates when a connector is configured explicitly on the agent — so
/// plain shortcut calls fail every `https://` request with "no TLS backend
/// is configured". Build an agent with the connector wired up instead.
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

    // Channel IDs are conventionally prefixed "UC" and appear directly in the
    // URL, so that form rewrites with no network call; `@handle`, `/c/`, and
    // `/user/` URLs don't carry the channel ID and need the channel page
    // scraped for its canonical feed link.
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    match segments.as_slice() {
        ["channel", id] if id.starts_with("UC") => {
            return Ok(format!(
                "https://www.youtube.com/feeds/videos.xml?channel_id={id}"
            ));
        }
        [handle] if handle.starts_with('@') && handle.len() > 1 => {}
        ["c" | "user", name] if !name.is_empty() => {}
        _ => return Err("URL is not a resolvable YouTube channel URL".to_string()),
    }

    let body = tls_agent()?
        .get(input)
        .call()
        .map_err(|e| format!("Failed to resolve YouTube channel: {e}"))?
        .into_string()
        .map_err(|e| format!("Failed to read YouTube channel page: {e}"))?;
    extract_rss_link(&body)
        .ok_or_else(|| "YouTube channel page did not contain an RSS feed URL".to_string())
}

fn url_authority_and_path(input: &str) -> Option<(&str, &str)> {
    let rest = input
        .strip_prefix("https://")
        .or_else(|| input.strip_prefix("http://"))?;
    let slash = rest.find('/')?;
    let host = rest[..slash].split('@').next()?.split(':').next()?;
    Some((host, &rest[slash..]))
}

/// Tags matching `<link ...>` in `text`, in order, as their inner text
/// (without the enclosing `<link`/`>`).
fn link_tags(text: &str) -> impl Iterator<Item = &str> {
    let mut rest = text;
    std::iter::from_fn(move || {
        let pos = rest.find("<link")?;
        let tag = &rest[pos..];
        let len = tag.find('>')?;
        let tag_text = &tag[..len];
        rest = &tag[len + 1..];
        Some(tag_text)
    })
}

fn extract_rss_link(body: &str) -> Option<String> {
    link_tags(body).find_map(|tag| {
        if extract_attribute(tag, "rel").as_deref() == Some("alternate")
            && extract_attribute(tag, "type").as_deref() == Some("application/rss+xml")
        {
            extract_attribute(tag, "href")
        } else {
            None
        }
    })
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
/// [`FeedEntry`] (guid, title, enclosure URL, MIME type, duration → ticks,
/// publish date). Sibling of `fetch_and_parse_rss` (#471); the idle-feed
/// path keeps its lean `IdleFeedItem` shape. Entries without any playable
/// source are skipped.
///
/// `subscription_kind` is the subscription's `FeedKind` used as the
/// canonical fallback when enclosure MIME is absent or unrecognized.
/// `feed_id` is the stable identity for the keyed feed-entry state store
/// (#492); typically the normalized subscription URL.
pub(super) fn fetch_and_parse_entries(
    url: &str,
    subscription_kind: FeedKind,
    feed_id: &str,
) -> Result<Vec<FeedEntry>, String> {
    let body = fetch_feed_body(url)?;
    let mut entries = parse_rss_entries(&body, subscription_kind, feed_id);
    if entries.is_empty() {
        entries = parse_atom_entries(&body, subscription_kind, feed_id);
    }
    Ok(entries)
}

fn parse_rss_entries(body: &str, subscription_kind: FeedKind, feed_id: &str) -> Vec<FeedEntry> {
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
            feed_id: Some(feed_id.to_string()),
            position_ticks: 0,
            played: false,
        });
    }
    entries
}

fn parse_atom_entries(body: &str, subscription_kind: FeedKind, feed_id: &str) -> Vec<FeedEntry> {
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
            feed_id: Some(feed_id.to_string()),
            position_ticks: 0,
            played: false,
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
    link_tags(text).find_map(|tag| {
        if extract_attribute(tag, "rel").as_deref() != Some("enclosure") {
            return None;
        }
        extract_attribute(tag, "href").map(|url| (url, extract_attribute(tag, "type")))
    })
}

/// Extract the value of an `attr="..."` attribute from a single tag's
/// text (entities decoded, control chars stripped, trimmed).
fn extract_attribute(text: &str, attr: &str) -> Option<String> {
    let needle = format!(r#"{attr}=""#);
    let start = text.find(&needle)? + needle.len();
    let end = text[start..].find('"')?;
    let value = &text[start..start + end];
    let decoded = decode_entities(value);
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
    let decoded = decode_entities(&stripped);
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
    let decoded = decode_entities(href);
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

/// Filter out control characters (C0/C1, including ESC and BEL) from feed
/// text before it can reach the terminal, e.g. via an OSC 8 escape sequence.
fn strip_control_chars(text: &str) -> String {
    text.chars().filter(|ch| !ch.is_control()).collect()
}

#[cfg(test)]
#[path = "feed_parse_tests.rs"]
mod tests;
