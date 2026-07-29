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

/// Format duration as minutes:seconds — for music tracks.
/// Examples: "0:47", "3:47", "12:05".
pub fn fmt_duration_mmss(s: i64) -> String {
    let m = s / 60;
    let s = s % 60;
    format!("{}:{:02}", m, s)
}

pub fn trunc_overview(s: &str) -> String {
    let stripped = regex_strip_urls(s);
    trunc_str(stripped.trim(), 400)
}

/// URL-stripped, trimmed overview text with no length cap. Used by the power
/// view's compact movie-detail banner, which grows to fit its full content
/// instead of truncating (#204, #263) -- unlike `trunc_overview`, still used
/// by the legacy library table row and the home-video list, which
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
/// false, every item is a flat `Track` with no headers. A header is only emitted for
/// runs of 3 or more consecutive same-key items; runs of 1-2 render as plain tracks.
/// The returned `Vec<String>` holds the label for the i-th `Header`.
pub(super) fn build_queue_rows(items: &[MediaItem], group: bool) -> (Vec<QueueRow>, Vec<String>) {
    let mut display: Vec<QueueRow> = Vec::new();
    let mut group_for_header: Vec<String> = Vec::new();
    if !group {
        display.extend((0..items.len()).map(|idx| QueueRow::Track { idx }));
        return (display, group_for_header);
    }

    // Grouping key/label for each item, or `None` for ungrouped items.
    let keys: Vec<Option<(String, String)>> = items
        .iter()
        .map(|item| {
            if item.is_audio() && !item.album.is_empty() {
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
            }
        })
        .collect();

    let mut last_group_key: Option<String> = None;
    let mut i = 0;
    while i < items.len() {
        match &keys[i] {
            Some((key, label)) => {
                // Find the end of this run of consecutive same-key items.
                let mut end = i;
                while end + 1 < items.len()
                    && keys[end + 1].as_ref().map(|(k, _)| k.as_str()) == Some(key.as_str())
                {
                    end += 1;
                }
                let run_len = end - i + 1;
                if run_len >= 3 {
                    if last_group_key.is_some() {
                        display.push(QueueRow::Spacer);
                    }
                    display.push(QueueRow::Header);
                    group_for_header.push(label.clone());
                    last_group_key = Some(key.clone());
                } else {
                    last_group_key = None;
                }
                for idx in i..=end {
                    display.push(QueueRow::Track { idx });
                }
                i = end + 1;
            }
            None => {
                last_group_key = None;
                display.push(QueueRow::Track { idx: i });
                i += 1;
            }
        }
    }

    (display, group_for_header)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::make_item;

    fn make_audio_item(album: &str, album_id: &str, artist: &str) -> MediaItem {
        let mut item = make_item(album, "Audio");
        item.album = album.to_string();
        item.album_id = album_id.to_string();
        item.artist = artist.to_string();
        item
    }

    fn make_episode_item(series_name: &str) -> MediaItem {
        let mut item = make_item(series_name, "Episode");
        item.series_name = series_name.to_string();
        item
    }

    fn make_movie_item() -> MediaItem {
        make_item("Movie", "Movie")
    }

    #[test]
    fn build_queue_rows_single_audio_item_no_header() {
        let items = vec![make_audio_item("Album A", "a1", "Artist")];
        let (rows, headers) = build_queue_rows(&items, true);

        // Single item should have no header
        assert_eq!(headers.len(), 0);
        assert_eq!(rows.len(), 1);
        assert!(matches!(rows[0], QueueRow::Track { idx: 0 }));
    }

    #[test]
    fn build_queue_rows_two_same_album_items_no_header() {
        let items = vec![
            make_audio_item("Album A", "a1", "Artist"),
            make_audio_item("Album A", "a1", "Artist"),
        ];
        let (rows, headers) = build_queue_rows(&items, true);

        // Two items with same album should have no header
        assert_eq!(headers.len(), 0);
        assert_eq!(rows.len(), 2);
        assert!(matches!(rows[0], QueueRow::Track { idx: 0 }));
        assert!(matches!(rows[1], QueueRow::Track { idx: 1 }));
    }

    #[test]
    fn build_queue_rows_three_same_album_items_has_header() {
        let items = vec![
            make_audio_item("Album A", "a1", "Artist"),
            make_audio_item("Album A", "a1", "Artist"),
            make_audio_item("Album A", "a1", "Artist"),
        ];
        let (rows, headers) = build_queue_rows(&items, true);

        // Three items with same album should have one header
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0], "Artist: Album A");
        assert_eq!(rows.len(), 4); // Header + 3 tracks
        assert!(matches!(rows[0], QueueRow::Header));
        assert!(matches!(rows[1], QueueRow::Track { idx: 0 }));
        assert!(matches!(rows[2], QueueRow::Track { idx: 1 }));
        assert!(matches!(rows[3], QueueRow::Track { idx: 2 }));
    }

    #[test]
    fn build_queue_rows_four_same_album_items_has_header() {
        let items = vec![
            make_audio_item("Album A", "a1", "Artist"),
            make_audio_item("Album A", "a1", "Artist"),
            make_audio_item("Album A", "a1", "Artist"),
            make_audio_item("Album A", "a1", "Artist"),
        ];
        let (rows, headers) = build_queue_rows(&items, true);

        // Four items with same album should have one header
        assert_eq!(headers.len(), 1);
        assert_eq!(rows.len(), 5); // Header + 4 tracks
        assert!(matches!(rows[0], QueueRow::Header));
    }

    #[test]
    fn build_queue_rows_three_episode_items_has_header() {
        let items = vec![
            make_episode_item("Series A"),
            make_episode_item("Series A"),
            make_episode_item("Series A"),
        ];
        let (rows, headers) = build_queue_rows(&items, true);

        // Three episodes with same series should have one header
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0], "Series A");
        assert_eq!(rows.len(), 4); // Header + 3 tracks
        assert!(matches!(rows[0], QueueRow::Header));
    }

    #[test]
    fn build_queue_rows_mixed_run_lengths() {
        let items = vec![
            make_audio_item("Album A", "a1", "Artist A"),
            make_audio_item("Album A", "a1", "Artist A"),
            // Run of 2: no header
            make_audio_item("Album B", "a2", "Artist B"),
            // Run of 1: no header
            make_audio_item("Album C", "a3", "Artist C"),
            make_audio_item("Album C", "a3", "Artist C"),
            make_audio_item("Album C", "a3", "Artist C"),
            // Run of 3: has header
            make_movie_item(),
            // Ungrouped: no header
        ];
        let (rows, headers) = build_queue_rows(&items, true);

        // Only the run of 3 should have a header
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0], "Artist C: Album C");

        // Expected rows: 2 tracks + 1 track + header + 3 tracks + 1 track = 8 rows
        assert_eq!(rows.len(), 8);
    }

    #[test]
    fn build_queue_rows_consecutive_groups_have_spacer() {
        let items = vec![
            make_audio_item("Album A", "a1", "Artist A"),
            make_audio_item("Album A", "a1", "Artist A"),
            make_audio_item("Album A", "a1", "Artist A"),
            // Header + Spacer expected here before Album B
            make_audio_item("Album B", "a2", "Artist B"),
            make_audio_item("Album B", "a2", "Artist B"),
            make_audio_item("Album B", "a2", "Artist B"),
        ];
        let (rows, headers) = build_queue_rows(&items, true);

        // Both groups should have headers
        assert_eq!(headers.len(), 2);

        // Expected: Header, 3 tracks, Spacer, Header, 3 tracks = 9 rows
        assert_eq!(rows.len(), 9);
        assert!(matches!(rows[0], QueueRow::Header));
        assert!(matches!(rows[1], QueueRow::Track { .. }));
        assert!(matches!(rows[2], QueueRow::Track { .. }));
        assert!(matches!(rows[3], QueueRow::Track { .. }));
        assert!(matches!(rows[4], QueueRow::Spacer));
        assert!(matches!(rows[5], QueueRow::Header));
    }

    #[test]
    fn build_queue_rows_no_grouping() {
        let items = vec![
            make_audio_item("Album A", "a1", "Artist A"),
            make_audio_item("Album A", "a1", "Artist A"),
            make_audio_item("Album A", "a1", "Artist A"),
        ];
        let (rows, headers) = build_queue_rows(&items, false);

        // With grouping disabled, no headers even for 3+ items
        assert_eq!(headers.len(), 0);
        assert_eq!(rows.len(), 3);
        assert!(matches!(rows[0], QueueRow::Track { idx: 0 }));
        assert!(matches!(rows[1], QueueRow::Track { idx: 1 }));
        assert!(matches!(rows[2], QueueRow::Track { idx: 2 }));
    }
}
