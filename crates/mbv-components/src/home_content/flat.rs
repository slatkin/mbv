//! The Home flat projection: the shell-owned Continue Watching items and the
//! shell-merged `Recently added` rows project into the shared
//! [`MediaListRow`] vocabulary. Headings are the only structural rows, and a
//! heading with no rows below it is omitted. Item rows use the flat lists'
//! split title-parts (`provider` context, item title as secondary);
//! `Recently added` item rows carry the provider date in the right gutter.
//! Age groups follow `openspec/specs/home-latest-sections` (the Feeds rule).

use mbv_queue::QueueItem;
use mbv_render::components::media_list::{
    MediaKind, MediaListRow, MediaListTrailing, MediaSemanticState,
};
use mbv_ui_model::feed_age::FeedAgeGroup;
use mbv_ui_model::home_latest::{HomeRecentRow, home_age_group, provider_timestamp_secs};
use mbv_ui_model::targets::HomeItemTarget;
use mbv_ui_model::ui_util::fmt_publish_date_short;

type HomeRow = MediaListRow<HomeItemTarget>;

fn heading(text: &str) -> HomeRow {
    MediaListRow::Heading { text: text.into() }
}

/// One item row; `Recently added` rows pass their gutter date.
fn item_row(
    target: HomeItemTarget,
    item: &QueueItem,
    feed_name: Option<&str>,
    trailing: Option<String>,
) -> HomeRow {
    let parts = item.playback_title_parts(feed_name);
    let (primary, secondary) = match parts.context {
        Some(context) => (context.text, Some(parts.title.text)),
        None => (parts.title.text, None),
    };
    MediaListRow::Item {
        target,
        primary,
        secondary,
        trailing: trailing.map(MediaListTrailing::Gutter),
        duration: None,
        kind: MediaKind::Media,
        semantic_state: MediaSemanticState::from_queue_item(item),
    }
}

/// The non-empty `Recently added` age groups, `New` then `Recent`, each with
/// its item rows in the order they arrive. One pass parses each date once.
fn grouped_rows(recent_rows: &[HomeRecentRow], now_secs: u64) -> Vec<(FeedAgeGroup, Vec<HomeRow>)> {
    let mut new_rows = Vec::new();
    let mut recent = Vec::new();
    for row in recent_rows {
        let timestamp = provider_timestamp_secs(&row.item);
        let bucket = match home_age_group(timestamp, now_secs) {
            Some(FeedAgeGroup::New) => &mut new_rows,
            Some(FeedAgeGroup::Recent) => &mut recent,
            None
            | Some(
                FeedAgeGroup::OlderThanTwoWeeks
                | FeedAgeGroup::OlderThanMonth
                | FeedAgeGroup::Unknown,
            ) => continue,
        };
        bucket.push(item_row(
            HomeItemTarget::Recent(row.item.content_id()),
            &row.item,
            row.feed_name.as_deref(),
            timestamp.map(fmt_publish_date_short),
        ));
    }
    [
        (FeedAgeGroup::New, new_rows),
        (FeedAgeGroup::Recent, recent),
    ]
    .into_iter()
    .filter(|(_, rows)| !rows.is_empty())
    .collect()
}

/// The complete Home projection: `Continue watching`, a spacer, then
/// `Recently added` with its non-empty `New` and `Recent` groups. Empty
/// inputs project no rows (the panel then renders its empty placeholder).
pub(super) fn flat_rows(
    continue_items: &[QueueItem],
    recent_rows: &[HomeRecentRow],
    now_secs: u64,
) -> Vec<HomeRow> {
    let mut rows = Vec::new();
    if !continue_items.is_empty() {
        rows.push(heading("Continue watching"));
        rows.extend(
            continue_items
                .iter()
                .map(|item| item_row(HomeItemTarget::continue_item(item.id()), item, None, None)),
        );
    }
    let groups = grouped_rows(recent_rows, now_secs);
    if !groups.is_empty() {
        if !rows.is_empty() {
            rows.push(MediaListRow::Spacer);
        }
        rows.push(heading("Recently added"));
        for (group, group_rows) in groups {
            rows.push(heading(group.label()));
            rows.extend(group_rows);
        }
    }
    rows
}
