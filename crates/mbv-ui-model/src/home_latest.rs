use mbv_queue::QueueItem;

#[cfg(test)]
mod tests;

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
