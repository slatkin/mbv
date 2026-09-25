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

/// One Feeds age group for `pub_date_secs` against `now_secs`: the shared
/// day-boundary criteria the Feeds tab and the podcast tab's episode
/// grouping both use (one implementation; the podcast tab consumes it
/// through `podcast_display_rows`).
pub(in crate::app) fn feed_age_group(pub_date_secs: Option<u64>, now_secs: u64) -> FeedAgeGroup {
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
