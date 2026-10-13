//! Shell-side Continue Watching projection and destination Latest marker state.

use super::Model;
use crate::app::dispatch::notify::ToastSeverity;
use crate::app::state::playback::{
    DestinationLatestSnapshot, DestinationLatestSource, HomeContent,
};
#[cfg(test)]
use mbv_components::home_content::HomeContent as HomeOwner;
#[cfg(test)]
use mbv_components::library_panel::LibraryPanel;
use mbv_core::service_runtime::SetupGeneration;
use mbv_queue::QueueItem;
use mbv_ui_model::browse::ServerRows;
#[cfg(test)]
use mbv_ui_model::library::LibraryKey;
#[cfg(test)]
use mbv_ui_msg::ComponentId;
use std::time::Instant;

impl Model {
    pub(in crate::app) fn assign_home_content(&mut self, content: HomeContent) {
        self.home_content = content;
        // A bootstrap or refresh may carry a rebuilt library set that dropped
        // a previously fetched source's library; evict it before projecting
        // (see `openspec/specs/home-latest-sections`).
        self.evict_gone_home_recently_added();
        self.push_home_content();
    }

    /// Evict fetched `Recently added` sources whose Service library no
    /// longer exists: an Emby source without a surviving library tab (a
    /// rebuilt or replaced setup dropped it), or an Audiobookshelf source
    /// without a surviving podcast library (a teardown or replacement
    /// empties the catalog). Feeds is derived at push time and never stored.
    /// Returns whether anything was evicted; the caller re-projects.
    pub(in crate::app) fn evict_gone_home_recently_added(&mut self) -> bool {
        let before = self.home_recently_added.len();
        let app = &self.app;
        self.home_recently_added.retain(|source, _| match source {
            DestinationLatestSource::Emby(library_id) => app
                .libs
                .iter()
                .any(|library| library.library.id == *library_id),
            DestinationLatestSource::Audiobookshelf(library_id) => app
                .audiobookshelf_libraries
                .iter()
                .any(|library| library.id == *library_id),
            DestinationLatestSource::Feeds => true,
        });
        self.home_recently_added.len() != before
    }

    pub(in crate::app) fn update_emby_latest_snapshot(
        &mut self,
        library_id: &str,
        items: Vec<QueueItem>,
    ) {
        let emby_items = items
            .iter()
            .filter_map(QueueItem::as_emby)
            .cloned()
            .collect::<Vec<_>>();
        self.tv_latest_snapshots.insert(
            library_id.to_string(),
            DestinationLatestSnapshot::new(items),
        );
        for library in &mut self.app.libs {
            if library.library.id != library_id || library.library.collection_type != "tvshows" {
                continue;
            }
            let Some(level) = library.nav_stack.last_mut() else {
                continue;
            };
            if level.tv_content_mode != Some(mbv_queue::TvContentMode::Latest) {
                continue;
            }
            level.items.clone_from(&emby_items);
            level.rows = ServerRows::complete(emby_items.len());
            level.loading = false;
        }
        self.push_active_emby_library_owner_content();
    }

    pub(in crate::app) fn clear_home_content(&mut self) {
        self.home_content.continue_items.clear();
        // `HomeContentCleared` is the Service teardown arm: a removed or
        // replaced setup also drops its fetched `Recently added` sources
        // (review P2).
        self.evict_gone_home_recently_added();
        self.push_home_content();
    }

    /// Accepts one fetched library's Home Recently added items (`openspec/specs/home-latest-sections`):
    /// the stale-generation drop matches the `ShelfFetched` App arm — a
    /// completion from a removed or re-setup Service never reaches the
    /// store — and the live-generation items are stored under their source
    /// filtered to the 14-day cutoff. Fetches never produce a Feeds source
    /// (Feeds rows are derived at push time), so a Feeds event stores
    /// nothing.
    pub(in crate::app) fn accept_home_recently_added(
        &mut self,
        source: DestinationLatestSource,
        generation: SetupGeneration,
        items: Vec<QueueItem>,
    ) {
        let accepts = match &source {
            DestinationLatestSource::Emby(_) => self.app.emby_runtime.accepts(generation),
            DestinationLatestSource::Audiobookshelf(_) => {
                self.app.audiobookshelf_runtime.accepts(generation)
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

    pub(in crate::app) fn home_stable_target(
        &self,
        target: &mbv_ui_model::targets::HomeItemTarget,
    ) -> Option<QueueItem> {
        // `Continue` is Emby-only, so it resolves through the shell-owned
        // Continue Watching snapshot. `Recent` looks up one row by content id
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
        self.app
            .feed_tab
            .all_entries
            .iter()
            .find(|entry| {
                matches!(content_id, mbv_queue::QueueItemContentId::Feed(guid) if *guid == entry.guid)
                    && mbv_ui_model::home_latest::is_home_recent(entry.pub_date_secs, now_secs)
            })
            .cloned()
            .map(QueueItem::Feed)
    }

    pub(in crate::app) fn fetch_home_at_startup(&mut self) {
        let fetched_home = self.app.fetch_home();
        self.home_content.loading = false;
        match fetched_home {
            Ok(content) => {
                // A launch with a cached Emby client brings the Service to
                // Ready at construction, so `apply_emby_bootstrap` -- the
                // async-startup seam that re-arms the Home Recently added
                // fetch -- never runs on this path. This is that launch's
                // one Home load with the eligible libraries already known,
                // so it arms the fetch here (spec `home-latest-sections`,
                // "Recently added loads at launch independent of
                // destinations": "when that Service becomes ready"). The
                // spawn is a no-op without an Emby client, which keeps the
                // async-startup launch (still Connecting here) on its own
                // bootstrap seam.
                self.app
                    .spawn_home_recently_added(mbv_queue::ServiceKind::Emby);
                let has_live_flash = self.app.status_expires.is_some_and(|t| t > Instant::now());
                if !has_live_flash {
                    self.app.status.clear();
                }
                self.assign_home_content(content);
            }
            Err(error) => {
                self.app.flash(
                    format!("Couldn't load home: {error}"),
                    ToastSeverity::Warning,
                );
                self.push_home_content();
            }
        }
    }

    pub(in crate::app) fn apply_emby_completion_drain(
        &mut self,
        completion: crate::app::dispatch::session::service_startup::Completion,
    ) {
        if let Some(content) = self.app.apply_emby_completion(completion) {
            self.assign_home_content(content);
        }
    }

    pub(in crate::app) fn apply_emby_setup_completion_drain(
        &mut self,
        completion: crate::app::dispatch::session::service_startup::SetupCompletion,
    ) {
        if let Some(content) = self.app.apply_emby_setup_completion(completion) {
            self.assign_home_content(content);
        }
    }

    pub(in crate::app) fn home_continue_watching_selected() -> bool {
        true
    }

    #[cfg(test)]
    pub(in crate::app) fn home_owner_shared(&self) -> Option<&HomeOwner> {
        self.application
            .get_component(&ComponentId::Library)
            .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
            .and_then(|panel| panel.owner(&LibraryKey::Home))
            .and_then(|owner| owner.as_any().downcast_ref::<HomeOwner>())
    }

    pub(in crate::app) fn acknowledge_home_latest(&mut self, source: DestinationLatestSource) {
        self.record_home_latest_acknowledgement(source);
        self.push_tv_workspace_content();
    }

    pub(in crate::app) fn record_home_latest_acknowledgement(
        &mut self,
        source: DestinationLatestSource,
    ) {
        self.acknowledged_home_latest_sources.insert(source);
    }

    /// Selecting a podcast library's Latest view acknowledges its Home marker
    /// and fetches the full Latest list (spec `destination-latest-modes`).
    pub(in crate::app) fn select_audiobookshelf_latest(&mut self, library_id: &str) {
        self.record_home_latest_acknowledgement(DestinationLatestSource::Audiobookshelf(
            library_id.to_owned(),
        ));
        self.spawn_audiobookshelf_latest_shelves(library_id);
    }

    /// Whether one destination's Latest surface carries the Iris
    /// new-content marker (`openspec/specs/destination-latest-modes`): any
    /// item in that source's Home `Recently added` rows — the per-source
    /// store, or the derived Feeds rows — dated inside the frozen launch
    /// window, while the source is not acknowledged. The marker is
    /// evaluated from Home's data even while another tab is active and
    /// before the destination's Latest list has loaded.
    pub(in crate::app) fn destination_latest_marker(
        &self,
        source: &DestinationLatestSource,
    ) -> bool {
        if self.acknowledged_home_latest_sources.contains(source) {
            return false;
        }
        let window = self.app.home_latest_launch_window;
        match source {
            // Stored rows are already filtered to the 14-day cutoff on
            // arrival, so the launch window is the remaining test: an item
            // dated after the launch instant never marks.
            DestinationLatestSource::Emby(_) | DestinationLatestSource::Audiobookshelf(_) => {
                self.home_recently_added.get(source).is_some_and(|items| {
                    items.iter().any(|item| {
                        mbv_ui_model::home_latest::is_new_in_launch_window(item, window)
                    })
                })
            }
            // Feeds has no store entry: its rows derive from `all_entries`
            // at push time, filtered to the same 14-day cutoff.
            DestinationLatestSource::Feeds => {
                let now_secs = mbv_render::screens::feeds_model::current_time_secs();
                self.app.feed_tab.all_entries.iter().any(|entry| {
                    mbv_ui_model::home_latest::timestamp_in_launch_window(
                        entry.pub_date_secs,
                        window,
                    ) && mbv_ui_model::home_latest::is_home_recent(entry.pub_date_secs, now_secs)
                })
            }
        }
    }
}
