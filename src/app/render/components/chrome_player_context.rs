use crate::app::App;
use mbv_core::playback_queue::{FeedEntry, PlaybackTitleParts, QueueItem};

impl App {
    /// Whether the queue column's transport projects a two-part now-playing
    /// title (a context part): the expanded title band's geometry condition.
    /// Same displayed-state lookup `transport_projection` uses for the parts,
    /// so the root placements' row count and the panel's projection cannot
    /// disagree about the band's height.
    pub(in crate::app) fn transport_title_expanded(&self) -> bool {
        let state = self.displayed_playback_state();
        let item = state
            .active_idx
            .filter(|_| state.active)
            .and_then(|idx| self.playback_queue().item_at(idx));
        item.is_some_and(|item| self.playback_title_parts(item).context.is_some())
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::make_app_stub;
    use mbv_core::config::{FeedKind, FeedSubscription};

    fn feed_entry(feed_id: Option<&str>) -> FeedEntry {
        FeedEntry {
            guid: "g1".into(),
            title: "Entry Title".into(),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: None,
            pub_date_secs: None,
            feed_kind: None,
            feed_id: feed_id.map(str::to_string),
            position_ticks: 0,
            played: false,
        }
    }

    /// The feed subscription display name (task 3.2): only an entry whose
    /// `feed_id` matches a configured subscription's url resolves a context
    /// part; a non-matching entry and a `None` `feed_id` resolve `None`.
    #[test]
    fn feed_subscription_display_name_resolves_only_a_matching_entry() {
        let app = make_app_stub();
        app.config.lock().unwrap().feeds.push(FeedSubscription {
            name: "Example Daily".into(),
            url: "https://example.com/feed.xml".into(),
            kind: FeedKind::Audio,
        });

        let matching = QueueItem::Feed(feed_entry(Some("https://example.com/feed.xml")));
        let parts = app.playback_title_parts(&matching);
        assert_eq!(
            parts.context.as_ref().map(|part| part.text.as_str()),
            Some("Example Daily"),
            "a matching subscription resolves to its display name"
        );

        let non_matching = QueueItem::Feed(feed_entry(Some("https://other.example.org/rss")));
        let parts = app.playback_title_parts(&non_matching);
        assert!(
            parts.context.is_none(),
            "a non-matching entry degrades to the title part alone"
        );

        let identity_less = QueueItem::Feed(feed_entry(None));
        let parts = app.playback_title_parts(&identity_less);
        assert!(
            parts.context.is_none(),
            "a `None` feed_id degrades to the title part alone"
        );
    }
}
