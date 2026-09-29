use std::borrow::Cow;

pub(crate) struct Line<'a> {
    pub ts: &'a str,
    pub level: tracing::Level,
    pub source: &'a str,
    pub event: Option<&'a str>,
    pub fields: &'a [(String, String)],
    pub spans: &'a [String],
    pub message: &'a str,
}

pub(crate) fn format_line(line: &Line<'_>) -> String {
    let mut output = String::from("ts=");
    push_value(&mut output, line.ts);
    output.push_str(" level=");
    push_value(&mut output, level_name(line.level));
    output.push_str(" source=");
    push_value(&mut output, line.source);
    if let Some(event) = line.event {
        output.push_str(" event=");
        push_value(&mut output, event);
    }
    let fields = format_fields(line.fields);
    if !fields.is_empty() {
        output.push(' ');
        output.push_str(&fields);
    }
    for span in line.spans {
        if !span.is_empty() {
            output.push(' ');
            output.push_str(span);
        }
    }
    output.push_str(" msg=");
    push_value(&mut output, &redact(line.message));
    output
}

pub(super) fn level_name(level: tracing::Level) -> &'static str {
    match level {
        tracing::Level::ERROR => "error",
        tracing::Level::WARN => "warn",
        tracing::Level::INFO => "info",
        tracing::Level::DEBUG => "debug",
        tracing::Level::TRACE => "trace",
    }
}

pub(crate) fn format_fields(fields: &[(String, String)]) -> String {
    let mut output = String::new();
    for (key, value) in fields {
        if !output.is_empty() {
            output.push(' ');
        }
        output.push_str(key);
        output.push('=');
        push_value(&mut output, &redact(value));
    }
    output
}

/// Appends `value`, quoted and escaped when it needs it. Does not redact.
fn push_value(output: &mut String, value: &str) {
    let quoted = value.chars().any(|character| {
        character.is_whitespace() || character == '=' || character == '"' || character.is_control()
    });
    if !quoted {
        output.push_str(value);
        return;
    }

    output.push('"');
    for character in value.chars() {
        match character {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            control if control.is_control() => {
                use std::fmt::Write;
                write!(output, "\\u{{{:x}}}", u32::from(control))
                    .expect("writing a control escape to a String should not fail");
            }
            character => output.push(character),
        }
    }
    output.push('"');
}

fn redact(value: &str) -> Cow<'_, str> {
    if !value.contains("Bearer ") && !value.contains("://") {
        return Cow::Borrowed(value);
    }
    let mut output = String::with_capacity(value.len());
    let mut index = 0;
    while index < value.len() {
        let remaining = &value[index..];
        if remaining.starts_with("Bearer ") {
            output.push_str("Bearer REDACTED");
            index += "Bearer ".len();
            while let Some(character) = value[index..].chars().next() {
                if character.is_whitespace() || character == '"' || character == '\'' {
                    break;
                }
                index += character.len_utf8();
            }
        } else if let Some(url_end) = url_end(value, index) {
            output.push_str(&redact_url(&value[index..url_end]));
            index = url_end;
        } else if let Some(character) = remaining.chars().next() {
            output.push(character);
            index += character.len_utf8();
        }
    }
    Cow::Owned(output)
}

fn url_end(value: &str, start: usize) -> Option<usize> {
    let mut characters = value[start..].char_indices();
    let (_, first) = characters.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }

    let mut colon = None;
    for (offset, character) in characters {
        if character == ':' {
            colon = Some(start + offset);
            break;
        }
        if !(character.is_ascii_alphanumeric() || matches!(character, '+' | '.' | '-')) {
            return None;
        }
    }
    let colon = colon?;
    if !value[colon..].starts_with("://") {
        return None;
    }

    let end = value[colon + 3..]
        .char_indices()
        .find_map(|(offset, character)| {
            (character.is_whitespace() || character == '"' || character == '\'')
                .then_some(colon + 3 + offset)
        })
        .unwrap_or(value.len());
    Some(end)
}

fn redact_url(url: &str) -> String {
    let suffix_start = url.find(['?', '#']).unwrap_or(url.len());
    let url = &url[..suffix_start];
    let authority_start = url.find("://").map_or(url.len(), |index| index + 3);
    let authority_end = url[authority_start..]
        .find('/')
        .map_or(url.len(), |index| authority_start + index);
    let userinfo_end = url[authority_start..authority_end]
        .rfind('@')
        .map(|index| authority_start + index + 1);
    if let Some(userinfo_end) = userinfo_end {
        format!("{}{}", &url[..authority_start], &url[userinfo_end..])
    } else {
        url.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::{Line, format_fields, format_line};

    #[test]
    fn line_preserves_key_order_and_outer_to_inner_span_order() {
        let fields = vec![("slot".to_owned(), "12".to_owned())];
        let spans = vec!["outer=yes".to_owned(), "inner=ok".to_owned()];
        assert_eq!(
            format_line(&Line {
                ts: "2026-09-28T14:03:12.345+02:00",
                level: tracing::Level::INFO,
                source: "player",
                event: Some("player.load.failed"),
                fields: &fields,
                spans: &spans,
                message: "load failed",
            }),
            "ts=2026-09-28T14:03:12.345+02:00 level=info source=player event=player.load.failed slot=12 outer=yes inner=ok msg=\"load failed\""
        );
    }

    #[test]
    fn line_quotes_and_escapes_values() {
        let fields = vec![("detail".to_owned(), "has space=\"x\"\\\n\r\t".to_owned())];
        assert_eq!(
            format_fields(&fields),
            "detail=\"has space=\\\"x\\\"\\\\\\n\\r\\t\""
        );
    }

    #[test]
    fn line_redacts_known_and_unlisted_url_credentials() {
        let fields = vec![
            (
                "stream".to_owned(),
                "https://host/media.mp3?api_key=secret".to_owned(),
            ),
            (
                "enclosure".to_owned(),
                "https://host/media.mp3?password=secret".to_owned(),
            ),
        ];
        let output = format_line(&Line {
            ts: "now",
            level: tracing::Level::INFO,
            source: "feed",
            event: None,
            fields: &fields,
            spans: &[],
            message: "fetch failed at https://user:pass@host/feed?password=secret",
        });
        assert_eq!(
            output,
            "ts=now level=info source=feed stream=https://host/media.mp3 enclosure=https://host/media.mp3 msg=\"fetch failed at https://host/feed\""
        );
    }

    #[test]
    fn line_redacts_bearer_token() {
        let fields = vec![("authorization".to_owned(), "Bearer secret".to_owned())];
        assert_eq!(format_fields(&fields), "authorization=\"Bearer REDACTED\"");
    }
}
