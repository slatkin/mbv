use mbv_ui_model::feed_age::{AgeGroupedRow, FeedAgeGroup, age_grouped_rows};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FeedDisplayRow {
    Spacer,
    Heading(FeedAgeGroup),
    Entry(usize),
}

/// The Feeds tab's rows: `visible_entries` grouped under the age-group
/// headings keyed on each entry's publish date, entry index as the item
/// payload. Shared projection: `age_grouped_rows`.
#[must_use]
pub fn feed_display_rows(entries: &[mbv_queue::FeedEntry], now_secs: u64) -> Vec<FeedDisplayRow> {
    age_grouped_rows(
        entries,
        now_secs,
        |entry| entry.pub_date_secs,
        |row| match row {
            AgeGroupedRow::Spacer => FeedDisplayRow::Spacer,
            AgeGroupedRow::Heading(group) => FeedDisplayRow::Heading(group),
            AgeGroupedRow::Item(index, _) => FeedDisplayRow::Entry(index),
        },
    )
}

#[must_use]
pub fn current_time_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
