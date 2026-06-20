use textwrap::wrap;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use crate::api::MediaItem;
use super::palette;

pub fn natural_sort_key(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            let mut num = c.to_string();
            while chars.peek().is_some_and(|d| d.is_ascii_digit()) {
                num.push(chars.next().unwrap());
            }
            out.push_str(&format!("{:0>8}", num));
        } else {
            out.push(c.to_ascii_lowercase());
        }
    }
    out
}

pub fn is_playable(item: &MediaItem) -> bool {
    matches!(item.media_type.as_str(), "Video" | "Audio")
}

pub fn sort_episodes(items: &mut Vec<MediaItem>) {
    items.sort_by_key(|i| i.index_number);
}

pub fn sort_audio_tracks(items: &mut Vec<MediaItem>) {
    let has_track_nums = items.iter().any(|i| i.index_number > 0);
    if has_track_nums {
        items.sort_by_key(|i| {
            if i.index_number > 0 {
                (0i64, i.parent_index_number, i.index_number, String::new())
            } else {
                (1i64, 0, 0, natural_sort_key(i.sort_key()))
            }
        });
    } else {
        items.sort_by_key(|i| natural_sort_key(i.sort_key()));
    }
}

pub fn fmt_duration(s: i64) -> String {
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60) }
    else         { format!("{}:{:02}", s / 60, s % 60) }
}

pub fn trunc_overview(s: &str) -> String {
    let stripped = regex_strip_urls(s);
    trunc_str(stripped.trim(), 300)
}

pub fn regex_strip_urls(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == 'h' {
            let mut buf = String::from(c);
            for expected in "ttp".chars() {
                match chars.peek() {
                    Some(&nc) if nc == expected => { buf.push(chars.next().unwrap()); }
                    _ => { out.push_str(&buf); buf.clear(); break; }
                }
            }
            if buf == "http" {
                if chars.peek() == Some(&'s') { buf.push(chars.next().unwrap()); }
                let mut ok = true;
                for expected in "://".chars() {
                    match chars.peek() {
                        Some(&nc) if nc == expected => { buf.push(chars.next().unwrap()); }
                        _ => { ok = false; break; }
                    }
                }
                if ok {
                    while chars.peek().is_some_and(|&c| !c.is_whitespace()) {
                        chars.next();
                    }
                } else {
                    out.push_str(&buf);
                }
            } else if !buf.is_empty() {
                out.push_str(&buf);
            }
        } else {
            out.push(c);
        }
    }
    let mut result = String::with_capacity(out.len());
    let mut prev_space = false;
    for c in out.chars() {
        if c.is_whitespace() {
            if !prev_space { result.push(' '); }
            prev_space = true;
        } else {
            result.push(c);
            prev_space = false;
        }
    }
    result
}

pub fn trunc_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max { s.to_string() }
    else { format!("{}\u{2026}", s.chars().take(max.saturating_sub(1)).collect::<String>()) }
}

pub fn item_text_and_style(item: &MediaItem, selected: bool) -> (String, Style) {
    if item.is_folder {
        let text = if item.item_type == "Folder" && item.total_count > 0 {
            format!("{} \u{00b7} {} albums", item.display_name(), item.total_count)
        } else if item.unplayed_item_count > 0 {
            format!("{} [{}]", item.display_name(), item.unplayed_item_count)
        } else {
            item.display_name()
        };
        let style = if selected { Style::default() }
            else               { Style::default().fg(palette::WHITE) };
        return (text, style);
    }
    let mut suffix = String::new();
    if item.runtime_ticks > 0 {
        let s = item.runtime_seconds();
        let h = (s / 3600.0) as u64;
        let m = ((s % 3600.0) / 60.0) as u64;
        let dur = if h > 0 { format!("{h}h{m:02}m") } else { format!("{m}m") };
        suffix = format!(" ({dur})");
    }
    let text = format!("{}{}", item.display_name(), suffix);
    let style = if selected { Style::default() }
        else { Style::default().fg(palette::WHITE) };
    (text, style)
}

pub fn split_suffix(s: &str) -> (&str, &str) {
    if s.ends_with(')') {
        if let Some(pos) = s.rfind(" (") { return (&s[..pos], &s[pos..]); }
    }
    if s.ends_with(']') {
        if let Some(pos) = s.rfind(" [") { return (&s[..pos], &s[pos..]); }
    }
    if let Some(pos) = s.find(" \u{00b7} ") { return (&s[..pos], &s[pos..]); }
    (s, "")
}

pub fn fmt_item_wrapped(item: &MediaItem, width: usize, selected: bool) -> Text<'static> {
    let (full_text, style) = item_text_and_style(item, selected);
    let in_progress = !item.is_folder && item.playback_position_ticks > 0;
    let yellow = Style::default().fg(palette::YELLOW);
    let subtle = Style::default().fg(palette::SUBTLE);
    let count_style = Style::default().fg(palette::IRIS).add_modifier(Modifier::BOLD);
    let label_style = Style::default().fg(palette::YELLOW);
    let w = width.max(1);
    // Returns extra spans for a suffix: dot-count suffixes get green+bold number + white label.
    let suf_spans = |suf: &str| -> Vec<Span<'static>> {
        if let Some(rest) = suf.strip_prefix(" \u{00b7} ") {
            // rest is e.g. "49 albums" — split at first space
            if let Some(sp) = rest.find(' ') {
                return vec![
                    Span::styled(format!(" \u{00b7} {}", &rest[..sp]), count_style),
                    Span::styled(rest[sp..].to_string(), label_style),
                ];
            }
            return vec![Span::styled(suf.to_string(), count_style)];
        }
        vec![Span::styled(suf.to_string(), subtle)]
    };
    let lines: Vec<Line<'static>> = wrap(&full_text, w).into_iter().enumerate()
        .map(|(i, s)| {
            let s = s.into_owned();
            if i == 0 && in_progress {
                let pct_str = if item.runtime_ticks > 0 {
                    let pct = (item.playback_position_ticks * 100 / item.runtime_ticks.max(1)) as u64;
                    format!(" {pct}%")
                } else { String::new() };
                let (name, suf) = split_suffix(&s);
                let mut spans = vec![Span::styled(name.to_string(), style)];
                if !suf.is_empty() { spans.extend(suf_spans(suf)); }
                if !pct_str.is_empty() { spans.push(Span::styled(pct_str, yellow)); }
                Line::from(spans)
            } else {
                let (name, suf) = split_suffix(&s);
                if suf.is_empty() {
                    Line::from(Span::styled(s, style))
                } else {
                    let mut spans = vec![Span::styled(name.to_string(), style)];
                    spans.extend(suf_spans(suf));
                    Line::from(spans)
                }
            }
        })
        .collect();
    if lines.is_empty() { Text::from("") } else { Text::from(lines) }
}

pub fn highlight_style(item: &MediaItem) -> Style {
    if item.is_folder && item.item_type != "Series" && item.item_type != "MusicAlbum" && item.item_type != "MusicArtist" {
        Style::default().fg(palette::BASE).bg(palette::PINE)
    } else if item.playback_position_ticks > 0 {
        Style::default().fg(palette::BASE).bg(palette::YELLOW)
    } else {
        Style::default().fg(palette::WHITE).bg(palette::FOCUSED)
    }
}

pub fn fmt_item_continue(item: &MediaItem, width: usize, selected: bool) -> Text<'static> {
    let (full_text, _) = item_text_and_style(item, selected);
    let in_progress = item.playback_position_ticks > 0;
    let span_style = if selected { Style::default() } else { Style::default().fg(palette::WHITE) };
    let yellow = Style::default().fg(palette::YELLOW);
    let subtle = Style::default().fg(palette::SUBTLE);
    let w = width.max(1);
    let lines: Vec<Line<'static>> = wrap(&full_text, w).into_iter().enumerate()
        .map(|(i, s)| {
            let s = s.into_owned();
            if i == 0 && in_progress {
                let pct_str = if item.runtime_ticks > 0 {
                    let pct = (item.playback_position_ticks * 100 / item.runtime_ticks.max(1)) as u64;
                    format!(" {pct}%")
                } else { String::new() };
                let (name, suf) = split_suffix(&s);
                let mut spans = vec![Span::styled(name.to_string(), span_style)];
                if !suf.is_empty() { spans.push(Span::styled(suf.to_string(), subtle)); }
                if !pct_str.is_empty() { spans.push(Span::styled(pct_str, yellow)); }
                Line::from(spans)
            } else {
                let (name, suf) = split_suffix(&s);
                if suf.is_empty() {
                    Line::from(Span::styled(s, span_style))
                } else {
                    Line::from(vec![Span::styled(name.to_string(), span_style), Span::styled(suf.to_string(), subtle)])
                }
            }
        })
        .collect();
    if lines.is_empty() { Text::from("") } else { Text::from(lines) }
}

pub fn highlight_style_continue(_item: &MediaItem) -> Style {
    Style::default().bg(palette::FOCUSED)
}
