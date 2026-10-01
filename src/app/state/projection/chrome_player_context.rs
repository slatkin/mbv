use crate::app::App;
use mbv_queue::{FeedEntry, PlaybackTitleParts, QueueItem};

impl App {
    /// The now-playing title parts for one queue item: core's media-type
    /// mapping, with the App-resolved feed subscription display name passed
    /// in for feed items only. The parts carry closed roles (D6); the
    /// painter's `title_part_fg` is the one site that resolves them to
    /// colours.
    pub(in crate::app) fn playback_title_parts(&self, item: &QueueItem) -> PlaybackTitleParts {
        let feed_subscription_name = item
            .as_feed()
            .and_then(|entry| self.feed_subscription_display_name(entry));
        item.playback_title_parts(feed_subscription_name.as_deref())
    }

    /// The display name of the configured subscription whose url matches the
    /// entry's `feed_id` (the subscription url recorded at fetch time, the
    /// same match the feed tab uses). `None` when the entry carries no
    /// `feed_id` or no configured subscription matches (task 3.2).
    pub(in crate::app) fn feed_subscription_display_name(
        &self,
        entry: &FeedEntry,
    ) -> Option<String> {
        let feed_id = entry.feed_id.as_deref()?;
        self.config
            .lock()
            .unwrap()
            .feeds
            .iter()
            .find(|subscription| subscription.url == feed_id)
            .map(|subscription| subscription.name.clone())
    }
}
