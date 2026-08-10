use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::QueueSlot;
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

pub fn is_playable(item: &EmbyItem) -> bool {
    matches!(item.media_type.as_str(), "Video" | "Audio")
}

pub fn sort_episodes(items: &mut [EmbyItem]) {
    items.sort_by_key(|i| i.index_number);
}

pub fn sort_audio_tracks(items: &mut [EmbyItem]) {
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

/// Format duration as `H:MM:SS` (or `M:SS` when under an hour). Only the
/// first component drops zero-padding; every later component is zero-padded
/// to two digits so `2:02` and `2:02:02` read as intended, not `2:2`. Intended
/// for right-aligned list cells where a ragged left edge reads cleaner than
/// padded columns.
/// Examples: "0:00", "0:45", "3:05", "59:59", "1:00:00", "1:23:45".
pub fn fmt_duration_short(s: i64) -> String {
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// Format duration without seconds — for video items in the queue.
/// Examples: "<1′", "37′", "125′".
pub fn fmt_duration_approx(s: i64) -> String {
    let total_mins = s / 60;
    if total_mins > 0 {
        format!("{}′", total_mins)
    } else if s > 0 {
        "<1′".to_string()
    } else {
        "0′".to_string()
    }
}

/// Format duration as minutes:seconds — for music tracks.
/// Examples: "0:47", "3:47", "12:05".
pub fn fmt_duration_mmss(s: i64) -> String {
    let m = s / 60;
    let s = s % 60;
    format!("{}:{:02}", m, s)
}

/// Format playback progress as "N%", capped at 99% (100% reads as finished,
/// not "in progress"). Empty when there's no meaningful progress to show.
pub fn fmt_playback_pct(pos_ticks: i64, runtime_ticks: i64) -> String {
    if pos_ticks > 0 && runtime_ticks > 0 {
        format!("{}%", (pos_ticks * 100 / runtime_ticks.max(1)).min(99))
    } else {
        String::new()
    }
}

pub fn trunc_overview(s: &str) -> String {
    let stripped = regex_strip_urls(s);
    trunc_str(stripped.trim(), 400)
}

/// URL-stripped, trimmed overview text with no length cap. Used by the view's
/// compact movie-detail banner, which grows to fit its full content
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

/// A visual row in the queue. The queue list has no grouping/headers — every
/// row is a slot from the canonical `PlaybackQueue`, in order.  The cursor
/// is a slot index.
#[derive(Clone)]
pub(super) enum QueueRow {
    Slot { slot_idx: usize },
}

/// Build the flat visual rows for the queue: one row per canonical slot.
pub(super) fn build_queue_rows(slots: &[QueueSlot]) -> Vec<QueueRow> {
    (0..slots.len())
        .map(|slot_idx| QueueRow::Slot { slot_idx })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::make_item;
    use mbv_core::api::EmbyItem;
    use mbv_core::playback_queue::{FeedEntry, PlaybackQueue, QueueItem};

    fn make_audio_item(album: &str, album_id: &str, artist: &str) -> EmbyItem {
        let mut item = make_item(album, "Audio");
        item.album = album.to_string();
        item.album_id = album_id.to_string();
        item.artist = artist.to_string();
        item
    }

    fn make_slots(items: &[EmbyItem], feed_items: &[FeedEntry]) -> Vec<QueueSlot> {
        let mut queue = PlaybackQueue::from_items(items.to_vec(), None);
        for entry in feed_items {
            queue.append(QueueItem::Feed(entry.clone()));
        }
        queue.into_slots()
    }

    #[test]
    fn build_queue_rows_is_flat_in_item_order() {
        let items = vec![
            make_audio_item("Album A", "a1", "Artist"),
            make_audio_item("Album A", "a1", "Artist"),
            make_audio_item("Album A", "a1", "Artist"),
        ];
        let slots = make_slots(&items, &[]);
        let rows = build_queue_rows(&slots);

        assert_eq!(rows.len(), 3);
        assert!(matches!(rows[0], QueueRow::Slot { slot_idx: 0 }));
        assert!(matches!(rows[1], QueueRow::Slot { slot_idx: 1 }));
        assert!(matches!(rows[2], QueueRow::Slot { slot_idx: 2 }));
    }

    #[test]
    fn build_queue_rows_appends_feed_rows_after_tracks() {
        let items = vec![
            make_audio_item("Album A", "a1", "Artist"),
            make_audio_item("Album A", "a1", "Artist"),
        ];
        let feed_items = vec![
            FeedEntry {
                guid: "f1".to_string(),
                title: "Feed 1".to_string(),
                enclosure_url: None,
                link: None,
                mime_type: None,
                duration_ticks: None,
                pub_date_secs: None,
                feed_kind: Some(mbv_core::config::FeedKind::Audio),
                feed_id: None,
                position_ticks: 0,
                played: false,
            },
            FeedEntry {
                guid: "f2".to_string(),
                title: "Feed 2".to_string(),
                enclosure_url: None,
                link: None,
                mime_type: None,
                duration_ticks: None,
                pub_date_secs: None,
                feed_kind: Some(mbv_core::config::FeedKind::Audio),
                feed_id: None,
                position_ticks: 0,
                played: false,
            },
        ];
        let slots = make_slots(&items, &feed_items);
        let rows = build_queue_rows(&slots);

        assert_eq!(rows.len(), 4);
        assert!(matches!(rows[0], QueueRow::Slot { slot_idx: 0 }));
        assert!(matches!(rows[1], QueueRow::Slot { slot_idx: 1 }));
        assert!(matches!(rows[2], QueueRow::Slot { slot_idx: 2 }));
        assert!(matches!(rows[3], QueueRow::Slot { slot_idx: 3 }));
    }
}
