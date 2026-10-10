use crate::targets::RecentAge;
use mbv_queue::QueueItem;

#[cfg(test)]
mod tests;

/// One merged `Recently added` row (design D2 of `home-recently-added-tree`):
/// the Service-qualified item plus the Feeds subscription display name when
/// the row is a feed entry; Emby and Audiobookshelf rows carry none.
#[derive(Debug, Clone)]
pub struct HomeRecentRow {
    pub item: QueueItem,
    pub feed_name: Option<String>,
}

/// The `Recently added` root's shared maximum age: items older than 14 days
/// before the current launch are cut, in both the shell's fetch filter
/// (design D2/D3) and the projection's branch assignment.
pub const RECENTLY_ADDED_CUTOFF_SECS: u64 = 14 * 24 * 60 * 60;

/// The launch-relative interval used by destination Latest markers.
/// `previous` is intentionally immutable and separate from exit-only UI state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HomeLatestLaunchWindow {
    pub previous: Option<u64>,
    pub current: u64,
}

/// Normalize the provider timestamp carried by a destination Latest item.
#[must_use]
pub fn provider_timestamp_secs(item: &QueueItem) -> Option<u64> {
    match item {
        QueueItem::Emby(item) => mbv_feed::parse_pub_date_secs(&item.date_added),
        QueueItem::Feed(entry) => entry.pub_date_secs,
        QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Episode(episode)) => {
            episode.pub_date_secs
        }
        QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Book(_)) => None,
    }
}

#[must_use]
pub fn timestamp_in_launch_window(timestamp: Option<u64>, window: HomeLatestLaunchWindow) -> bool {
    let Some(previous) = window.previous else {
        return false;
    };
    timestamp.is_some_and(|timestamp| previous < timestamp && timestamp <= window.current)
}

#[must_use]
pub fn is_new_in_launch_window(item: &QueueItem, window: HomeLatestLaunchWindow) -> bool {
    timestamp_in_launch_window(provider_timestamp_secs(item), window)
}

/// Assign the `Recently added` age branch for one provider timestamp.
///
/// Branch assignment follows the spec's three branches: `New` holds items
/// dated after the previous client launch, `Last 7 days` holds the remaining
/// items up to 7 days old, and `Last 14 days` holds the remaining items; an
/// item older than `RECENTLY_ADDED_CUTOFF_SECS` before the current launch
/// gets `None`, as does a missing timestamp. A first launch (`previous` is
/// `None`) never yields `New`.
#[must_use]
pub fn recent_age(timestamp: Option<u64>, window: HomeLatestLaunchWindow) -> Option<RecentAge> {
    let timestamp = timestamp?;
    if timestamp < window.current.saturating_sub(RECENTLY_ADDED_CUTOFF_SECS) {
        return None;
    }
    if window.previous.is_some_and(|previous| timestamp > previous) {
        return Some(RecentAge::New);
    }
    let last_7_days_cutoff = window.current.saturating_sub(7 * 24 * 60 * 60);
    if timestamp >= last_7_days_cutoff {
        Some(RecentAge::Last7Days)
    } else {
        RecentAge::Last14Days.into()
    }
}
