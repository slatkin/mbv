use super::types_feed::IdleFeedItem;

/// Fetch an RSS/Atom feed and parse `<item>`/`<entry>` titles and links.
pub(super) fn fetch_and_parse_rss(url: &str) -> Result<Vec<IdleFeedItem>, String> {
    let body = ureq::get(url)
        .call()
        .map_err(|e| format!("HTTP request failed: {e}"))?
        .into_string()
        .map_err(|e| format!("Failed to read response body: {e}"))?;

    let mut items = Vec::new();

    // Try RSS `<item>` blocks first
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

    // If no RSS items found, try Atom `<entry>` blocks
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
    use super::extract_tag;

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
}
