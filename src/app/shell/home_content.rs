//! Shell-side Home content assignment, the Emby destination Latest
//! snapshot, and the destination Latest marker state. Home's `Recently
//! added` store and its rows live on the `App`
//! (`state::app_struct::home_recently_added`, add-gui-library-panel row
//! 6.2); these shell halves project what the App stores.

use super::Model;
use crate::app::state::playback::{
    DestinationLatestSnapshot, DestinationLatestSource, HomeContent,
};
#[cfg(test)]
use mbv_components::home_content::HomeContent as HomeOwner;
#[cfg(test)]
use mbv_components::library_panel::LibraryPanel;
#[cfg(test)]
use mbv_ui_model::library::LibraryKey;
#[cfg(test)]
use mbv_ui_msg::ComponentId;

impl Model {
    pub(in crate::app) fn assign_home_content(&mut self, content: HomeContent) {
        // The App's write owns the eviction; this half is the projection
        // below (add-gui-library-panel row 6.2).
        self.app.apply_home_content(Some(content));
        self.push_home_content();
    }

    pub(in crate::app) fn update_emby_latest_snapshot(
        &mut self,
        library_id: &str,
        items: Vec<mbv_queue::QueueItem>,
    ) {
        let emby_items = items
            .iter()
            .filter_map(mbv_queue::QueueItem::as_emby)
            .cloned()
            .collect::<Vec<_>>();
        self.tv_latest_snapshots.insert(
            library_id.to_string(),
            DestinationLatestSnapshot::new(items),
        );
        // The App's write owns the Latest root level's fill (row 7.1); this
        // half is the shell-owned snapshot store above and the projection
        // below.
        self.app.apply_tv_latest_snapshot(library_id, emby_items);
        self.push_active_emby_library_owner_content();
    }

    pub(in crate::app) fn clear_home_content(&mut self) {
        // The App's write owns the snapshot clear and the eviction (review
        // P2); this half is the projection below.
        self.app.apply_home_content(None);
        self.push_home_content();
    }

    pub(in crate::app) fn fetch_home_at_startup(&mut self) {
        let fetched_home = self.app.load_home_at_startup();
        self.app.home_content.loading = false;
        match fetched_home {
            Some(content) => self.assign_home_content(content),
            None => self.push_home_content(),
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
            DestinationLatestSource::Emby(_) | DestinationLatestSource::Audiobookshelf(_) => self
                .app
                .home_recently_added
                .get(source)
                .is_some_and(|items| {
                    items.iter().any(|item| {
                        mbv_ui_model::home_latest::is_new_in_launch_window(item, window)
                    })
                }),
            // Feeds has no store entry: its rows derive from `all_entries`
            // at merge time, filtered to the same 14-day cutoff.
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
