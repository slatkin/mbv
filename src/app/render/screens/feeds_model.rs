use mbv_ui_model::feed_age::{feed_age_group, FeedAgeGroup};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::app) enum FeedDisplayRow {
    Spacer,
    Heading(FeedAgeGroup),
    Entry(usize),
}

pub(in crate::app) fn feed_display_rows(
    entries: &[mbv_queue::FeedEntry],
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
