use crate::app::render::components::hero::HERO_TITLE_ROWS;
use crate::app::ui_util::list_duration_secs;
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::playback_queue::FeedEntry;
use std::time::{SystemTime, UNIX_EPOCH};

const SECONDS_PER_DAY: u64 = 24 * 60 * 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum FeedAgeGroup {
    New,
    Recent,
    OlderThanTwoWeeks,
    OlderThanMonth,
    Unknown,
}

impl FeedAgeGroup {
    pub(in crate::app) fn label(self) -> &'static str {
        match self {
            Self::New => "New",
            Self::Recent => "Recent",
            Self::OlderThanTwoWeeks => "Older than two weeks",
            Self::OlderThanMonth => "Older than a month",
            Self::Unknown => "Unknown date",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::app) enum FeedDisplayRow {
    Spacer,
    Heading(FeedAgeGroup),
    Entry(usize),
}

fn feed_age_group(pub_date_secs: Option<u64>, now_secs: u64) -> FeedAgeGroup {
    let Some(pub_date_secs) = pub_date_secs else {
        return FeedAgeGroup::Unknown;
    };

    match now_secs.saturating_sub(pub_date_secs) / SECONDS_PER_DAY {
        0..=1 => FeedAgeGroup::New,
        2..=13 => FeedAgeGroup::Recent,
        14..=29 => FeedAgeGroup::OlderThanTwoWeeks,
        _ => FeedAgeGroup::OlderThanMonth,
    }
}

pub(in crate::app) fn feed_display_rows(
    entries: &[mbv_core::playback_queue::FeedEntry],
    now_secs: u64,
) -> Vec<FeedDisplayRow> {
    let mut rows = Vec::new();
    let mut last_group = None;

    for (idx, entry) in entries.iter().enumerate() {
        let group = feed_age_group(entry.pub_date_secs, now_secs);
        if last_group != Some(group) {
            if last_group.is_some() {
                rows.push(FeedDisplayRow::Spacer);
            }
            rows.push(FeedDisplayRow::Heading(group));
            last_group = Some(group);
        }
        rows.push(FeedDisplayRow::Entry(idx));
    }

    rows
}

pub(in crate::app) fn current_time_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Convert an optional feed tick count to canonical list duration text.
pub(in crate::app) fn feed_duration_text(ticks: Option<u64>) -> Option<String> {
    ticks
        .map(|t| (t / TICKS_PER_SECOND as u64) as i64)
        .and_then(list_duration_secs)
}

/// Format a pub_date_secs value into a short date string for display.
pub(super) fn format_pub_date(secs: Option<u64>) -> String {
    match secs {
        Some(s) => {
            // Simple YYYY-MM-DD from unix seconds
            let days = (s / 86400) as i64 + 719468;
            let era = if days >= 0 { days } else { days - 146096 } / 146097;
            let doe = days - era * 146097;
            let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
            let y = yoe + era * 400;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let d = doy - (153 * mp + 2) / 5 + 1;
            let m = if mp < 10 { mp + 3 } else { mp - 9 };
            let yr = if m <= 2 { y + 1 } else { y };
            format!("{yr:04}-{m:02}-{d:02}")
        }
        None => String::new(),
    }
}

/// Row budget for the feeds hero's text content (design.md decision 6:
/// element presence -- no image, since feed entries carry no artwork), a
/// title row (two-column lists only) plus a single metadata line and its
/// trailing spacer.
pub(in crate::app::render) fn feed_hero_content_rows(show_title: bool) -> u16 {
    let title_rows = if show_title { HERO_TITLE_ROWS } else { 0 };
    title_rows + 2
}

/// The feeds hero's one metadata line: duration, publish date, MIME type,
/// and watched state, in that order -- feeds' declared metadata set
/// (design.md decision 6).
pub(in crate::app::render) fn feed_entry_meta_line(entry: &FeedEntry) -> String {
    let mut parts = Vec::new();
    if let Some(duration) = feed_duration_text(entry.duration_ticks) {
        parts.push(duration);
    }
    let date = format_pub_date(entry.pub_date_secs);
    if !date.is_empty() {
        parts.push(date);
    }
    if let Some(mime) = entry.mime_type.as_deref() {
        if !mime.is_empty() {
            parts.push(mime.to_string());
        }
    }
    if entry.played {
        parts.push("Watched".to_string());
    }
    parts.join("   ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use mbv_core::config::FeedKind;
    use mbv_core::playback_queue::FeedEntry;

    fn entry(title: &str, pub_date_secs: Option<u64>) -> FeedEntry {
        FeedEntry {
            guid: title.to_string(),
            title: title.to_string(),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: None,
            pub_date_secs,
            feed_kind: Some(FeedKind::Video),
            feed_id: None,
            position_ticks: 0,
            played: false,
        }
    }

    #[test]
    fn age_groups_cover_boundaries_and_unknown_dates() {
        let now = 30 * SECONDS_PER_DAY;
        let cases = [
            (Some(now + SECONDS_PER_DAY), FeedAgeGroup::New),
            (Some(now), FeedAgeGroup::New),
            (Some(now - SECONDS_PER_DAY), FeedAgeGroup::New),
            (Some(now - 2 * SECONDS_PER_DAY), FeedAgeGroup::Recent),
            (Some(now - 13 * SECONDS_PER_DAY), FeedAgeGroup::Recent),
            (
                Some(now - 14 * SECONDS_PER_DAY),
                FeedAgeGroup::OlderThanTwoWeeks,
            ),
            (
                Some(now - 29 * SECONDS_PER_DAY),
                FeedAgeGroup::OlderThanTwoWeeks,
            ),
            (
                Some(now - 30 * SECONDS_PER_DAY),
                FeedAgeGroup::OlderThanMonth,
            ),
            (None, FeedAgeGroup::Unknown),
        ];

        for (date, expected) in cases {
            assert_eq!(feed_age_group(date, now), expected);
        }
    }

    #[test]
    fn display_rows_insert_non_selectable_groups_without_changing_indices() {
        let now = 30 * SECONDS_PER_DAY;
        let entries = vec![
            entry("new", Some(now)),
            entry("recent", Some(now - 2 * SECONDS_PER_DAY)),
            entry("two weeks", Some(now - 14 * SECONDS_PER_DAY)),
            entry("month", Some(now - 30 * SECONDS_PER_DAY)),
            entry("unknown", None),
        ];

        assert_eq!(
            feed_display_rows(&entries, now),
            vec![
                FeedDisplayRow::Heading(FeedAgeGroup::New),
                FeedDisplayRow::Entry(0),
                FeedDisplayRow::Spacer,
                FeedDisplayRow::Heading(FeedAgeGroup::Recent),
                FeedDisplayRow::Entry(1),
                FeedDisplayRow::Spacer,
                FeedDisplayRow::Heading(FeedAgeGroup::OlderThanTwoWeeks),
                FeedDisplayRow::Entry(2),
                FeedDisplayRow::Spacer,
                FeedDisplayRow::Heading(FeedAgeGroup::OlderThanMonth),
                FeedDisplayRow::Entry(3),
                FeedDisplayRow::Spacer,
                FeedDisplayRow::Heading(FeedAgeGroup::Unknown),
                FeedDisplayRow::Entry(4),
            ]
        );
    }
}
