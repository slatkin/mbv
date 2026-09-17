use mbv_core::api::EmbyItem;
use unicode_width::UnicodeWidthStr;

/// Three-letter month names, indexed 0 = January (the same abbreviation
/// style `render::components::home_video::format_release_date` renders).
const MONTH_ABBREVIATIONS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Format a publish date given as unix seconds (UTC) as a hero meta row:
/// "19 Jun 2015" — the same shape the Emby hero's release date uses.
/// Saturation-free: a seconds value beyond `time`'s range renders as the
/// empty string rather than panicking.
pub fn fmt_publish_date(secs: u64) -> String {
    let Ok(date) = time::OffsetDateTime::from_unix_timestamp(i64::try_from(secs).unwrap_or(0))
    else {
        return String::new();
    };
    let month = u8::from(date.month());
    let month = MONTH_ABBREVIATIONS
        .get(usize::from(month - 1))
        .copied()
        .unwrap_or("");
    format!("{} {month} {}", date.day(), date.year())
}

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

/// Move a list cursor by `delta` rows (signed), clamped to `[0, len-1]`.
/// Handles the empty-list case by returning 0.
pub(crate) fn move_cursor(cur: usize, delta: i64, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    (cur as i64 + delta).clamp(0, len as i64 - 1) as usize
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

/// Returns the letter-group bucket label for the sort key `key` given `total`
/// items in the list. "#" for keys starting with a digit or non-ASCII-alphabetic
/// character; individual letters for 250+ items; three-letter ranges below that.
///
/// KNOWN LIMITATION: any non-ASCII-alphabetic first character (accented letters
/// like "Æon"/"Élan" included, codepoint > 'Z') buckets here as "#". But the "#"
/// *pill*'s Emby fetch bounds are `NameLessThan("A")` -- only titles that SORT
/// BEFORE "A" -- so an accented title with a codepoint after 'Z' is actually
/// fetched by the `V–Z` pill (`name_ge = "V"`, no upper bound) yet renders under
/// this "#" header, making it unreachable from the "#" pill's scoped fetch.
/// Left as-is; flagged for a follow-up.
pub fn letter_bucket_label(key: &str, total: usize) -> String {
    let first = key
        .chars()
        .next()
        .map(|c| c.to_ascii_uppercase())
        .unwrap_or('\0');
    if !first.is_ascii_alphabetic() {
        return "#".to_string();
    }
    if total >= 250 {
        return first.to_string();
    }
    match first {
        'A'..='C' => "A\u{2013}C",
        'D'..='F' => "D\u{2013}F",
        'G'..='I' => "G\u{2013}I",
        'J'..='L' => "J\u{2013}L",
        'M'..='O' => "M\u{2013}O",
        'P'..='R' => "P\u{2013}R",
        'S'..='U' => "S\u{2013}U",
        _ => "V\u{2013}Z",
    }
    .to_string()
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

/// Format duration without seconds — for video items in the queue and the
/// hero meta row.
/// Examples: "<1m", "37m", "1h12m", "2h15m".
pub fn fmt_duration_approx(s: i64) -> String {
    let h = s / 3600;
    let m = (s % 3600) / 60;
    if h > 0 {
        if m > 0 {
            format!("{}h{}m", h, m)
        } else {
            format!("{}h", h)
        }
    } else if m > 0 {
        format!("{}m", m)
    } else if s > 0 {
        "<1m".to_string()
    } else {
        "0m".to_string()
    }
}

/// URL-stripped, trimmed overview text with no length cap. Used by the view's
/// compact movie-detail banner, which grows to fit its full content
/// instead of truncating (#204, #263).
pub fn clean_overview(s: &str) -> String {
    regex_strip_urls(s).trim().to_string()
}

pub fn regex_strip_urls(s: &str) -> String {
    s.split_whitespace()
        .filter(|token| !token.contains("http://") && !token.contains("https://"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// First `n` chars of `s`, with no ellipsis — for fixed-width abbreviations
/// like language codes ("en", "eng"), not for display truncation of
/// arbitrary text (see `trunc_str` for that).
pub fn take_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

pub fn trunc_str(s: &str, max: usize) -> String {
    if max == 0 {
        String::new()
    } else if s.width() <= max {
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
