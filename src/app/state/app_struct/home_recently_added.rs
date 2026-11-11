//! The App's Home `Recently added` store and its Feeds-derived rows
//! (add-gui-library-panel row 6.2): the state a fetched event lands in and
//! the functions that maintain and read it. Live on the `App` so both
//! Clients — the TUI shell's pushes and the GUI Library's projection — read
//! the same facts; the shell's acknowledgement and projection halves stay
//! shell code.

use crate::app::state::app_struct::App;
use crate::app::state::playback::DestinationLatestSource;
use mbv_queue::QueueItem;

impl App {
    /// Evict fetched `Recently added` sources whose Service library no
    /// longer exists: an Emby source without a surviving library tab (a
    /// rebuilt or replaced setup dropped it), or an Audiobookshelf source
    /// without a surviving podcast library (a teardown or replacement
    /// empties the catalog). Feeds is derived at merge time and never stored.
    /// Returns whether anything was evicted; the caller re-projects.
    pub(in crate::app) fn evict_gone_home_recently_added(&mut self) -> bool {
        let before = self.home_recently_added.len();
        self.home_recently_added.retain(|source, _| match source {
            DestinationLatestSource::Emby(library_id) => self
                .libs
                .iter()
                .any(|library| library.library.id == *library_id),
            DestinationLatestSource::Audiobookshelf(library_id) => self
                .audiobookshelf_libraries
                .iter()
                .any(|library| library.id == *library_id),
            DestinationLatestSource::Feeds => true,
        });
        self.home_recently_added.len() != before
    }

    /// Accepts one fetched library's Home Recently added items (`openspec/specs/home-latest-sections`):
    /// the stale-generation drop matches the `ShelfFetched` App arm — a
    /// completion from a removed or re-setup Service never reaches the
    /// store — and the live-generation items are stored under their source
    /// filtered to the 14-day cutoff. Fetches never produce a Feeds source
    /// (Feeds rows are derived at merge time), so a Feeds event stores
    /// nothing.
    pub(in crate::app) fn accept_home_recently_added(
        &mut self,
        source: DestinationLatestSource,
        generation: mbv_core::service_runtime::SetupGeneration,
        items: Vec<QueueItem>,
    ) {
        let accepts = match &source {
            DestinationLatestSource::Emby(_) => self.emby_runtime.accepts(generation),
            DestinationLatestSource::Audiobookshelf(_) => {
                self.audiobookshelf_runtime.accepts(generation)
            }
            DestinationLatestSource::Feeds => false,
        };
        if accepts {
            self.store_home_recently_added(source, items);
        }
    }

    /// Stores one fetched library's Home Recently added items (`openspec/specs/home-latest-sections`):
    /// filtered to the 14-day cutoff on arrival, keyed by the source. An
    /// item without a valid date gets no age branch and is omitted.
    pub(in crate::app) fn store_home_recently_added(
        &mut self,
        source: DestinationLatestSource,
        items: Vec<QueueItem>,
    ) {
        let now_secs = mbv_render::screens::feeds_model::current_time_secs();
        let items = items
            .into_iter()
            .filter(|item| {
                mbv_ui_model::home_latest::is_home_recent(
                    mbv_ui_model::home_latest::provider_timestamp_secs(item),
                    now_secs,
                )
            })
            .collect();
        self.home_recently_added.insert(source, items);
    }

    /// The merged, newest-first `Recently added` rows, built at merge time
    /// from the per-source store plus Feeds (`openspec/specs/home-latest-sections`). Feeds has no store entry: its rows derive
    /// from `all_entries`, filtered to the same 14-day cutoff, with the
    /// subscription name resolved here. A merged row without a valid
    /// date has no age branch, so it can never appear.
    pub(in crate::app) fn home_recent_rows(
        &self,
        now_secs: u64,
    ) -> Vec<mbv_ui_model::home_latest::HomeRecentRow> {
        use mbv_ui_model::home_latest::{HomeRecentRow, is_home_recent, provider_timestamp_secs};
        use std::cmp::Reverse;
        let mut rows: Vec<HomeRecentRow> = self
            .home_recently_added
            .values()
            .flatten()
            .map(|item| HomeRecentRow {
                item: item.clone(),
                feed_name: None,
            })
            .collect();
        rows.extend(
            self.feed_tab
                .all_entries
                .iter()
                .filter(|entry| is_home_recent(entry.pub_date_secs, now_secs))
                .map(|entry| {
                    let feed_name = entry.feed_id.as_deref().and_then(|feed_id| {
                        self.feed_tab
                            .subscriptions
                            .iter()
                            .find(|subscription| subscription.url == feed_id)
                            .map(|subscription| subscription.name.clone())
                    });
                    HomeRecentRow {
                        item: QueueItem::Feed(entry.clone()),
                        feed_name,
                    }
                }),
        );
        rows.sort_by_cached_key(|row| Reverse(provider_timestamp_secs(&row.item)));
        rows
    }

    pub(in crate::app) fn home_stable_target(
        &self,
        target: &mbv_ui_model::targets::HomeItemTarget,
    ) -> Option<QueueItem> {
        // `Continue` is Emby-only, so it resolves through the Continue
        // Watching snapshot. `Recent` looks up one row by content id
        // in the stored sources and the Feeds entries, without building the
        // merged list (`openspec/specs/home-latest-sections`).
        match target {
            mbv_ui_model::targets::HomeItemTarget::Continue(item_id) => self
                .home_content
                .continue_items
                .iter()
                .find(|item| item.id == item_id.as_str())
                .cloned()
                .map(|item| QueueItem::Emby(Box::new(item))),
            mbv_ui_model::targets::HomeItemTarget::Recent(content_id) => self
                .home_recently_added
                .values()
                .flatten()
                .find(|item| item.content_id() == *content_id)
                .cloned()
                .or_else(|| self.home_recent_feed_entry(content_id)),
        }
    }

    /// The Feeds `Recently added` row with this content id, if its entry is
    /// inside the 14-day cutoff (Feeds rows are derived, never stored).
    fn home_recent_feed_entry(
        &self,
        content_id: &mbv_queue::QueueItemContentId,
    ) -> Option<QueueItem> {
        let now_secs = mbv_render::screens::feeds_model::current_time_secs();
        self.feed_tab
            .all_entries
            .iter()
            .find(|entry| {
                matches!(content_id, mbv_queue::QueueItemContentId::Feed(guid) if *guid == entry.guid)
                    && mbv_ui_model::home_latest::is_home_recent(entry.pub_date_secs, now_secs)
            })
            .cloned()
            .map(QueueItem::Feed)
    }
}
