/// Decode common XML/HTML entities (`&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;`)
/// and numeric character references (`&#NNN;` / `&#xHHHH;`) in a single
/// left-to-right scan. Anything unrecognized (e.g. a stray `&` or an
/// unsupported named entity) is left untouched rather than erroring.
pub fn decode_entities(text: &str) -> String {
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
        if let Some(ch) = decoded_char {
            result.push(ch);
            rest = &tail[semi_idx + 1..];
        } else {
            // Unrecognized entity: leave the leading '&' untouched and
            // keep scanning from just after it.
            result.push('&');
            rest = &tail[1..];
        }
    }
    result.push_str(rest);
    result
}

/// Convert a snippet of HTML (common in Audiobookshelf episode/podcast
/// descriptions) into plain terminal text: block tags become paragraph
/// breaks, links keep their visible text plus the URL as `text (URL)`,
/// and entities are decoded. Inline styling/formatting tags are dropped.
#[must_use]
pub fn html_to_text(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut rest = html;

    // Href of the open <a> tag; text between `<a ...>` and `</a>` is kept,
    // then the href follows in parentheses on closing.
    let mut pending_link: Option<String> = None;

    while let Some(lt) = rest.find('<') {
        result.push_str(&rest[..lt]);
        let after = &rest[lt..];
        let Some(gt) = after.find('>') else {
            result.push_str(after);
            break;
        };
        let tag = &after[1..gt];
        let lower = tag.trim().to_ascii_lowercase();

        if let Some(name) = lower.strip_prefix('/') {
            if is_block_tag(name.trim()) {
                result.push('\n');
            } else if name.trim() == "a" {
                if let Some(href) = pending_link.take() {
                    result.extend((!result.is_empty() && !result.ends_with(' ')).then_some(' '));
                    result.push('(');
                    result.push_str(&href);
                    result.push(')');
                }
            }
        } else {
            let name = lower
                .trim_end_matches('/')
                .split_whitespace()
                .next()
                .unwrap_or("");
            if name == "a" {
                pending_link = extract_href(&lower);
            } else if is_block_tag(name) {
                result.push('\n');
            }
        }
        rest = &after[gt + 1..];
    }
    result.push_str(rest);

    result = decode_entities(&result);
    result = trim_blank_lines(&result);
    result.trim().to_string()
}

fn is_block_tag(name: &str) -> bool {
    matches!(name, "p" | "div" | "li" | "ul" | "ol" | "br")
}

/// Collapse runs of blank lines (and trailing spaces) down to single
/// newlines, trimming each line.
fn trim_blank_lines(text: &str) -> String {
    text.split('\n')
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Extract the `href="..."` value from an `<a ...>` tag body.
fn extract_href(tag_body: &str) -> Option<String> {
    let key = "href=\"";
    let start = tag_body.find(key)? + key.len();
    let end = tag_body[start..].find('\"')?;
    Some(decode_entities(&tag_body[start..start + end]))
}

#[cfg(test)]
mod tests;
