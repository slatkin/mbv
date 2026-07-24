use mbv_core::api::MediaItem;
use unicode_width::UnicodeWidthStr;

/// Advance subtitle mode through the standard cycle.
pub(super) fn next_subtitle_mode(current: &str) -> &'static str {
    match current {
        "Default" | "" => "Always",
        "Always" => "Smart",
        "Smart" => "OnlyForced",
        "OnlyForced" => "None",
        "None" => "HearingImpaired",
        _ => "Default",
    }
}

/// Advance a language preference through `["" (any)] + my_languages`.
pub(super) fn cycle_lang(my_languages: &[String], current: &str) -> String {
    let cycle: Vec<&str> = std::iter::once("")
        .chain(my_languages.iter().map(String::as_str))
        .collect();
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
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// Format duration without seconds — for video items in the queue.
/// Examples: "<1m", "37m", "1h05m", "2h03m".
pub fn fmt_duration_approx(s: i64) -> String {
    let total_mins = s / 60;
    let h = total_mins / 60;
    let m = total_mins % 60;
    if h > 0 {
        format!("{}h{:02}m", h, m)
    } else if m > 0 {
        format!("{}m", m)
    } else if s > 0 {
        "<1m".to_string()
    } else {
        "0m".to_string()
    }
}

pub fn trunc_overview(s: &str) -> String {
    let stripped = regex_strip_urls(s);
    trunc_str(stripped.trim(), 400)
}

/// URL-stripped, trimmed overview text with no length cap. Used by the power
/// view's compact movie-detail banner, which grows to fit its full content
/// instead of truncating (#204, #263) -- unlike `trunc_overview`, still used
/// by the legacy library table row and the power-view home-video list, which
/// render through a fixed-height surface.
pub fn clean_overview(s: &str) -> String {
    regex_strip_urls(s).trim().to_string()
}

pub fn regex_strip_urls(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == 'h' {
            let mut buf = String::from(c);
            for expected in "ttp".chars() {
                match chars.peek() {
                    Some(&nc) if nc == expected => {
                        buf.push(chars.next().unwrap());
                    }
                    _ => {
                        out.push_str(&buf);
                        buf.clear();
                        break;
                    }
                }
            }
            if buf == "http" {
                if chars.peek() == Some(&'s') {
                    buf.push(chars.next().unwrap());
                }
                let mut ok = true;
                for expected in "://".chars() {
                    match chars.peek() {
                        Some(&nc) if nc == expected => {
                            buf.push(chars.next().unwrap());
                        }
                        _ => {
                            ok = false;
                            break;
                        }
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
            if !prev_space {
                result.push(' ');
            }
            prev_space = true;
        } else {
            result.push(c);
            prev_space = false;
        }
    }
    result
}

/// First `n` chars of `s`, with no ellipsis — for fixed-width abbreviations
/// like language codes ("en", "eng"), not for display truncation of
/// arbitrary text (see `trunc_str` for that).
pub fn take_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

pub fn trunc_str(s: &str, max: usize) -> String {
    if s.width() <= max {
        s.to_string()
    } else {
        let mut out = String::new();
        let mut w = 0;
        for c in s.chars() {
            let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
            if w + cw + 1 > max {
                break;
            }
            out.push(c);
            w += cw;
        }
        out.push('\u{2026}');
        out
    }
}

/// A visual row in the queue: a group header, a blank spacer between groups, or a
/// track (item index into the underlying queue).
#[derive(Clone)]
pub(super) enum QueueRow {
    Header,
    Spacer,
    Track { idx: usize },
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
        display.extend((0..items.len()).map(|idx| QueueRow::Track { idx }));
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
        display.push(QueueRow::Track { idx: i });
    }
    (display, group_for_header)
}
