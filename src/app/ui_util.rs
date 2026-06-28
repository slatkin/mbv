use ratatui::style::{Modifier, Style};
use unicode_width::UnicodeWidthStr;
use ratatui::text::{Line, Span, Text};
use crate::api::MediaItem;
use super::palette;

/// Advance subtitle mode through the standard cycle.
pub(super) fn next_subtitle_mode(current: &str) -> &'static str {
    match current {
        "Default" | "" => "Always",
        "Always"       => "Smart",
        "Smart"        => "OnlyForced",
        "OnlyForced"   => "None",
        "None"         => "HearingImpaired",
        _              => "Default",
    }
}

/// Advance a language preference through `["" (any)] + my_languages`.
pub(super) fn cycle_lang(my_languages: &[String], current: &str) -> String {
    let cycle: Vec<&str> = std::iter::once("").chain(my_languages.iter().map(String::as_str)).collect();
    let idx = cycle.iter().position(|&l| l == current).unwrap_or(0);
    cycle[(idx + 1) % cycle.len()].to_string()
}

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

pub fn sort_episodes(items: &mut [MediaItem]) {
    items.sort_by_key(|i| i.index_number);
}

pub fn sort_audio_tracks(items: &mut [MediaItem]) {
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

/// Format duration without seconds — for video items in the queue.
/// Examples: "<1m", "37m", "1h05m", "2h03m".
pub fn fmt_duration_approx(s: i64) -> String {
    let total_mins = s / 60;
    let h = total_mins / 60;
    let m = total_mins % 60;
    if h > 0 { format!("{}h{:02}m", h, m) }
    else if m > 0 { format!("{}m", m) }
    else if s > 0 { "<1m".to_string() }
    else { "0m".to_string() }
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
    if s.width() <= max { s.to_string() }
    else {
        let mut out = String::new();
        let mut w = 0;
        for c in s.chars() {
            let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
            if w + cw + 1 > max { break; }
            out.push(c);
            w += cw;
        }
        out.push('\u{2026}');
        out
    }
}

pub fn item_text_and_style(item: &MediaItem, selected: bool) -> (String, Style) {
    if item.is_folder {
        let text = if item.item_type == "Folder" && item.total_count > 0 {
            format!("{} \u{00b7} {} items", item.display_name(), item.total_count)
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


pub fn fmt_item_wrapped(item: &MediaItem, width: usize, selected: bool) -> Text<'static> {
    let name_style = if selected {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(palette::WHITE).add_modifier(Modifier::BOLD)
    };
    let yellow = Style::default().fg(palette::YELLOW);
    let subtle = Style::default().fg(palette::SUBTLE);
    let count_style = Style::default().fg(palette::IRIS).add_modifier(Modifier::BOLD);

    let in_progress = !item.is_folder && item.playback_position_ticks > 0;

    let dur_str: String = if item.runtime_ticks > 0 {
        let s = item.runtime_seconds();
        let h = (s / 3600.0) as u64;
        let m = ((s % 3600.0) / 60.0) as u64;
        if h > 0 { format!("{h}h{m:02}m") } else { format!("{m}m") }
    } else { String::new() };

    // Subtitle line: context metadata
    let subtitle: String = if item.is_folder {
        String::new()
    } else if item.item_type == "Episode" && item.parent_index_number > 0 {
        let tag = format!("S{:02}E{:02}", item.parent_index_number, item.index_number);
        if !item.series_name.is_empty() {
            format!("{tag}  {}", item.series_name)
        } else { tag }
    } else if item.item_type == "Audio" {
        if !item.album.is_empty() && !item.artist.is_empty() {
            format!("{}  {}", item.artist, item.album)
        } else if !item.artist.is_empty() {
            item.artist.clone()
        } else { String::new() }
    } else if item.item_type == "MusicAlbum" && !item.artist.is_empty() {
        item.artist.clone()
    } else {
        String::new()
    };

    // Duration goes on line 1 as right-aligned suffix when there's subtitle content,
    // otherwise drops to line 2 so the second row isn't wasted blank.
    let has_subtitle = !subtitle.is_empty();

    let suffix: String = if item.is_folder {
        if item.unplayed_item_count > 0 {
            format!("[{}]", item.unplayed_item_count)
        } else if item.total_count > 0 {
            format!("{}", item.total_count)
        } else {
            String::new()
        }
    } else if in_progress {
        if item.runtime_ticks > 0 {
            let pct = (item.playback_position_ticks * 100 / item.runtime_ticks.max(1)) as u64;
            format!("{pct}%")
        } else { String::new() }
    } else if has_subtitle {
        dur_str.clone()
    } else {
        String::new()
    };

    let name = trunc_str(&item.name, width.saturating_sub(suffix.width() + 2).max(1));
    let gap = width.saturating_sub(name.width() + suffix.width());
    let suffix_style = if in_progress { yellow } else if item.is_folder { count_style } else { subtle };
    let mut line1_spans = vec![Span::styled(name, name_style)];
    if !suffix.is_empty() {
        line1_spans.push(Span::styled(" ".repeat(gap.max(1)), Style::default()));
        line1_spans.push(Span::styled(suffix, suffix_style));
    }

    let sub_style = subtle;
    let line2_content = if has_subtitle {
        trunc_str(&subtitle, width.max(1))
    } else {
        dur_str
    };
    let line2 = Line::from(Span::styled(line2_content, sub_style));

    Text::from(vec![Line::from(line1_spans), line2])
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
    let name_style = if selected {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(palette::WHITE).add_modifier(Modifier::BOLD)
    };
    let yellow = Style::default().fg(palette::YELLOW);
    let subtle = Style::default().fg(palette::SUBTLE);

    // Fixed right-side columns (widths include 1 leading space each)
    let dur_str = if item.runtime_ticks > 0 {
        let s = item.runtime_seconds();
        let h = (s / 3600.0) as u64;
        let m = ((s % 3600.0) / 60.0) as u64;
        if h > 0 { format!("{h}h{m:02}m") } else { format!("{m}m") }
    } else { String::new() };

    let pct_str = if item.runtime_ticks > 0 {
        let pct = (item.playback_position_ticks * 100 / item.runtime_ticks.max(1)) as u64;
        format!("{pct}%")
    } else { String::new() };

    let ep_tag = if item.item_type == "Episode" && item.parent_index_number > 0 {
        format!("S{}E{:02}", item.parent_index_number, item.index_number)
    } else { String::new() };

    let context = if item.item_type == "Episode" {
        item.series_name.clone()
    } else if item.item_type == "Audio" {
        item.artist.clone()
    } else if item.production_year > 0 {
        item.production_year.to_string()
    } else { String::new() };

    // Column layout (left to right):
    //   episode title | context (series/year/artist) | ep tag | pct | dur
    const CTX_W: usize = 20; // series name / year / artist
    const TAG_W: usize =  9; // "S2025E31 "
    const PCT_W: usize =  5; // " 100%"
    const DUR_W: usize =  6; // " 1h23m"

    let ctx_used = if context.is_empty() { 0 } else { CTX_W };
    let tag_used = if ep_tag.is_empty() { 0 } else { TAG_W };
    let fixed_w = ctx_used + tag_used + PCT_W + DUR_W;
    let name_w = width.saturating_sub(fixed_w).max(4);

    let ctx_col = if context.is_empty() { String::new() } else { format!("{:<width$}", trunc_str(&context, CTX_W - 1), width = CTX_W) };
    let tag_col = if ep_tag.is_empty() { String::new() } else { format!("{:<width$}", ep_tag, width = TAG_W) };
    let pct_col = if pct_str.is_empty() { " ".repeat(PCT_W) } else { format!("{:>width$}", pct_str, width = PCT_W) };
    let dur_col = if dur_str.is_empty() { " ".repeat(DUR_W) } else { format!("{:>width$}", dur_str, width = DUR_W) };

    let col_style = subtle;

    let name_trunc = trunc_str(&item.name, name_w);
    let name_pad = name_w.saturating_sub(name_trunc.width());
    let line = Line::from(vec![
        Span::styled(name_trunc, name_style),
        Span::raw(" ".repeat(name_pad)),
        Span::styled(ctx_col, col_style),
        Span::styled(tag_col, col_style),
        Span::styled(pct_col, yellow),
        Span::styled(dur_col, col_style),
    ]);
    Text::from(vec![line])
}

pub fn highlight_style_continue(_item: &MediaItem) -> Style {
    Style::default().bg(palette::FOCUSED)
}

/// A visual row in the queue: a group header, a blank spacer between groups, or a
/// track (item index + whether it sits under a group header, which drives the indent).
#[derive(Clone)]
pub(super) enum QueueRow {
    Header,
    Spacer,
    Track { idx: usize, in_group: bool },
}

/// Build the visual rows for the queue.
///
/// When `group` is true, audio items are grouped by album ("Artist: Album") and
/// episodes by series name, with a `Header` before each group and a `Spacer` between
/// consecutive groups; movies and everything else stay ungrouped. When `group` is
/// false, every item is a flat `Track` with no headers. The returned `Vec<String>`
/// holds the label for the i-th `Header`.
pub(super) fn build_queue_rows(items: &[MediaItem], group: bool) -> (Vec<QueueRow>, Vec<String>) {
    let mut display: Vec<QueueRow> = Vec::new();
    let mut group_for_header: Vec<String> = Vec::new();
    if !group {
        display.extend((0..items.len()).map(|idx| QueueRow::Track { idx, in_group: false }));
        return (display, group_for_header);
    }
    let mut last_group_key: Option<String> = None;
    for (i, item) in items.iter().enumerate() {
        let group = if item.is_audio() && !item.album.is_empty() {
            let key = format!("a:{}", item.album_id);
            let label = if item.artist.is_empty() {
                item.album.clone()
            } else {
                format!("{}: {}", item.artist, item.album)
            };
            Some((key, label))
        } else if item.item_type == "Episode" && !item.series_name.is_empty() {
            Some((format!("e:{}", item.series_name), item.series_name.clone()))
        } else {
            None
        };

        let in_group = group.is_some();
        if let Some((key, label)) = group {
            if last_group_key.as_deref() != Some(key.as_str()) {
                if last_group_key.is_some() {
                    display.push(QueueRow::Spacer);
                }
                display.push(QueueRow::Header);
                group_for_header.push(label);
                last_group_key = Some(key);
            }
        } else {
            last_group_key = None;
        }
        display.push(QueueRow::Track { idx: i, in_group });
    }
    (display, group_for_header)
}
