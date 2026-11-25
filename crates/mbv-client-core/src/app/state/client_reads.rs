//! The core facts only one Client reads.
//!
//! A Client reaches no `App` field (change `split-tui-gui-clients`, design
//! D4/D4b rule 1). The facts here are core state that only one Client's
//! production code asks for — the window Client, whose own core holds no
//! mirror of them: the shown queue entry, the Named-queue list, the
//! queue-shuffle and Audiobookshelf-admission capabilities, the queue
//! filter's held bit, the peer queue's length, the configuration's autoload
//! rule, the TV series-detail and Upcoming facts, and — in `browse` — the
//! Emby-library and Audiobookshelf catalog facts. A fact both Clients read
//! lives in `shell_reads` instead; the state itself stays in the core, and a
//! write to it goes through `client_writes` or the shared write module.

mod browse;

use crate::app::App;
use crate::app::state::playback::ShownEntry;
use mbv_ctrl::NamedQueueEntry;
use mbv_emby_model::EmbyItem;
use mbv_queue::NamedQueueId;

impl App {
    /// The entry the Queue panel shows: a local Named queue or the connected
    /// peer entry, `None` while this Client shows neither.
    #[must_use]
    pub fn shown_entry(&self) -> Option<ShownEntry> {
        self.shown_entry
    }

    /// The local Owner's Named queues in queue order, as the last published
    /// list holds them.
    #[must_use]
    pub fn named_queue_entries(&self) -> &[NamedQueueEntry] {
        self.named_queues.entries()
    }

    /// How many Named queues the local Owner's last published list holds.
    #[must_use]
    pub fn named_queue_count(&self) -> usize {
        self.named_queues.entries().len()
    }

    /// The current Named queue: the one that holds the playhead.
    #[must_use]
    pub fn current_named_queue(&self) -> Option<NamedQueueId> {
        self.named_queues.current()
    }

    /// Whether the local Owner still holds `queue`.
    #[must_use]
    pub fn named_queue_holds(&self, queue: NamedQueueId) -> bool {
        self.named_queues.holds(queue)
    }

    /// The connected peer Player owner's queue length, or `0` while this
    /// Client holds no peer view.
    #[must_use]
    pub fn remote_queue_len(&self) -> usize {
        self.remote_view
            .as_ref()
            .map_or(0, crate::app::QueueView::total_queue_len)
    }

    /// Whether the Player owner the queue plays accepts a whole-queue
    /// reorder.
    #[must_use]
    pub fn supports_queue_shuffle(&self) -> bool {
        self.player.supports_queue_shuffle()
    }

    /// Whether the Player owner the queue plays accepts Audiobookshelf items.
    #[must_use]
    pub fn can_admit_audiobookshelf(&self) -> bool {
        self.player.can_admit_audiobookshelf()
    }

    /// Whether the queue filter's text entry holds the keyboard.
    #[must_use]
    pub fn queue_filter_active(&self) -> bool {
        self.queue_filter_active
    }

    /// The text the queue filter's entry holds, as the user typed it: the
    /// entry's own draft, which only a test reads back.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn queue_filter_text_for_test(&self) -> &str {
        &self.queue_filter
    }

    /// Whether a played album expands to its full tracklist.
    #[must_use]
    pub fn autoload_enabled(&self) -> bool {
        self.with_config(|config| config.autoload)
    }

    /// Whether the series detail for `series_id` is being fetched.
    #[must_use]
    pub fn series_detail_loading(&self, series_id: &str) -> bool {
        self.series_detail_loading.contains(series_id)
    }

    /// Whether the episode list of `series_id`'s season `season_id` is being
    /// fetched.
    #[must_use]
    pub fn series_season_loading(&self, series_id: &str, season_id: &str) -> bool {
        self.series_season_loading
            .contains(&(series_id.to_string(), season_id.to_string()))
    }

    /// Whether the cached detail of `series_id` waits for an in-view refresh to
    /// replace it (design D9).
    #[must_use]
    pub fn series_detail_stale(&self, series_id: &str) -> bool {
        self.series_detail_stale.contains(series_id)
    }

    /// Whether the cached episode list of `series_id`'s season `season_id` waits
    /// for an in-view refresh to replace it (design D9).
    #[must_use]
    pub fn series_season_stale(&self, series_id: &str, season_id: &str) -> bool {
        self.series_season_stale
            .contains(&(series_id.to_string(), season_id.to_string()))
    }

    /// The cached series detail's season or episode with `item_id`: the item a
    /// Shows tree row names, from whichever cached detail holds it.
    #[must_use]
    pub fn series_detail_item(&self, item_id: &str) -> Option<EmbyItem> {
        self.series_detail_cache.values().find_map(|detail| {
            detail
                .seasons
                .iter()
                .chain(detail.episodes.values().flatten())
                .find(|item| item.id == item_id)
                .cloned()
        })
    }

    /// Every TV library's Upcoming premieres, as last fetched: the merged rows
    /// of the Upcoming list, in no particular library order.
    pub fn tv_upcoming_premieres(&self) -> impl Iterator<Item = &mbv_emby_model::UpcomingPremiere> {
        self.tv_upcoming_snapshots.values().flatten()
    }
}
