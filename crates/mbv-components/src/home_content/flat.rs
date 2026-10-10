//! The Home flat projection: the shell-owned Continue Watching items and the
//! shell-merged `Recently added` rows project into the shared
//! [`MediaListRow`] vocabulary. Headings are the only structural rows, and a
//! heading with no rows below it is omitted. Item rows use the flat lists'
//! split title-parts (`provider` context, item title as secondary);
//! `Recently added` item rows carry the provider date in the right gutter.
//! `Recently added` is one flat list with no age sub-headings
//! (`openspec/specs/home-latest-sections`); the 14-day cutoff still applies.

use mbv_queue::QueueItem;
use mbv_render::components::media_list::{
    MediaKind, MediaListRow, MediaListTrailing, MediaSemanticState,
};
use mbv_ui_model::home_latest::{HomeRecentRow, is_home_recent, provider_timestamp_secs};
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

/// The `Recently added` item rows inside the 14-day cutoff, in the order they
/// arrive. One date parse per row.
fn recent_item_rows(recent_rows: &[HomeRecentRow], now_secs: u64) -> Vec<HomeRow> {
    recent_rows
        .iter()
        .filter_map(|row| {
            let timestamp = provider_timestamp_secs(&row.item);
            is_home_recent(timestamp, now_secs).then(|| {
                item_row(
                    HomeItemTarget::Recent(row.item.content_id()),
                    &row.item,
                    row.feed_name.as_deref(),
                    timestamp.map(fmt_publish_date_short),
                )
            })
        })
        .collect()
}

/// The complete Home projection: `Continue watching`, a spacer, then
/// `Recently added` as one flat list. Empty inputs project no rows (the panel
/// then renders its empty placeholder).
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
    let recent = recent_item_rows(recent_rows, now_secs);
    if !recent.is_empty() {
        if !rows.is_empty() {
            rows.push(MediaListRow::Spacer);
        }
        rows.push(heading("Recently added"));
        rows.extend(recent);
    }
    rows
}
