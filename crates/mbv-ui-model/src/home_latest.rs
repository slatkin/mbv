use mbv_queue::QueueItem;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
mod tests;

/// The launch-relative interval used by destination Latest markers.
/// `previous` is intentionally immutable and separate from exit-only UI state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HomeLatestLaunchWindow {
    pub previous: Option<u64>,
    pub current: u64,
}

#[must_use]
pub fn current_launch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[must_use]
pub fn capture_launch_window(current: u64) -> HomeLatestLaunchWindow {
    if current == 0 {
        log::warn!(target: "home_latest", "invalid non-positive launch cutoff; markers disabled");
        return HomeLatestLaunchWindow {
            previous: None,
            current,
        };
    }

    let previous = mbv_config::load_home_latest_launch();
    if let Err(error) = mbv_config::save_home_latest_launch(current) {
        log::warn!(target: "home_latest", "could not save launch cutoff: {error}");
        return HomeLatestLaunchWindow {
            previous: None,
            current,
        };
    }
    HomeLatestLaunchWindow { previous, current }
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
pub fn is_new_in_launch_window(item: &QueueItem, window: HomeLatestLaunchWindow) -> bool {
    let Some(previous) = window.previous else {
        return false;
    };
    provider_timestamp_secs(item)
        .is_some_and(|timestamp| previous < timestamp && timestamp <= window.current)
}
