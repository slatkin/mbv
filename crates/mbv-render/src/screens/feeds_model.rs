use mbv_ui_model::feed_age::{FeedAgeGroup, feed_age_group};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FeedDisplayRow {
    Spacer,
    Heading(FeedAgeGroup),
    Entry(usize),
}

// At most one heading per feed age group plus one spacer between groups
// (5 groups today, so 9 extra rows max); adding a group without raising
// this bound only costs a reallocation, never correctness.
const MAX_GROUP_ROWS: usize = 9;

#[must_use]
pub fn feed_display_rows(entries: &[mbv_queue::FeedEntry], now_secs: u64) -> Vec<FeedDisplayRow> {
    let mut rows = Vec::with_capacity(entries.len() + MAX_GROUP_ROWS);
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

#[must_use]
pub fn current_time_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
