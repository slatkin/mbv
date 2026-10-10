use crate::feed_age::{FeedAgeGroup, feed_age_group};
use mbv_queue::QueueItem;

#[cfg(test)]
mod tests;

/// One merged `Recently added` row (spec `home-latest-sections`, requirement
/// "Recently added combines every Latest-bearing destination"): the
/// Service-qualified item plus the Feeds subscription display name when the
/// row is a feed entry; Emby and Audiobookshelf rows carry none.
#[derive(Debug, Clone)]
pub struct HomeRecentRow {
    pub item: QueueItem,
    pub feed_name: Option<String>,
}

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

/// The Home age group of one provider timestamp, by the Feeds rule: `Some`
/// only for `New` and `Recent`. Older and undated items belong to no group.
#[must_use]
pub fn home_age_group(timestamp: Option<u64>, now_secs: u64) -> Option<FeedAgeGroup> {
    match feed_age_group(timestamp, now_secs) {
        group @ (FeedAgeGroup::New | FeedAgeGroup::Recent) => Some(group),
        FeedAgeGroup::OlderThanTwoWeeks | FeedAgeGroup::OlderThanMonth | FeedAgeGroup::Unknown => {
            None
        }
    }
}

/// Whether one provider timestamp is inside Home's `Recently added` cutoff.
/// Every Home cutoff asks this question, so the 14-day rule has one owner.
#[must_use]
pub fn is_home_recent(timestamp: Option<u64>, now_secs: u64) -> bool {
    home_age_group(timestamp, now_secs).is_some()
}
