//! The browse-stack half of the global UI-state reset (change
//! `separate-data-refresh-from-ui-reset`, row 5.1; issue #745): return every
//! Emby library's root level to its default browse fields, and every retained
//! Audiobookshelf state to its default pill fan-out scope. The shell-owned
//! browse stack lives here, so the reset of that stack lives here too (design
//! D3 of `split-tui-gui-clients`: an inherent `impl App` is in the core, and
//! the `Model` half of the reset stays in the shell). The reset touches no
//! protected domain state.

use crate::app::App;
use mbv_queue::TvContentMode;
use mbv_ui_model::browse::BrowseResting;
use mbv_ui_model::feed::FeedHomeVideoSelection;

impl App {
    /// Return every Emby library's shell-owned browse stack to its root and
    /// its root level to default filter/sort/scope fields, retaining fetched
    /// content (no cache invalidation, no refetch). Deep levels are dropped
    /// because the destination's default context is its root. Every retained
    /// Audiobookshelf podcast state also returns to the default state-pill
    /// fan-out scope (design D5).
    pub fn reset_browse_root_defaults(&mut self) {
        for idx in 0..self.libs.len() {
            self.reset_library_browse_root(idx);
        }
        // The owner's reset pill is `State(All)`; align the shell-owned lazy
        // fan-out scope so a later refresh does not re-request only the stale
        // show pill (#745, design D5: state pill => `committed_show_pill`
        // `None`).
        for state in &mut self.audiobookshelf_browse {
            state.committed_show_pill = None;
        }
    }

    fn reset_library_browse_root(&mut self, idx: usize) {
        if self.libs[idx].nav_stack.is_empty() {
            return;
        }
        let is_tv = self.libs[idx].library.collection_type == "tvshows";
        let (item_types, unplayed_only, sort_by, sort_order) =
            self.default_library_level_fields(idx);
        // The TV content mode is shell-pushed (`context.tv_content_mode`); a
        // TV reset must resolve the existing count-dependent default here, not
        // only in the owner. Every library defaults to `Latest`.
        let tv_mode = is_tv.then(|| {
            mbv_ui_model::sort_filter::resolve_tv_content_mode(
                self.libs[idx].library_total.unwrap_or_default(),
                None,
            )
        });
        let root_item_types = if is_tv {
            Some(
                match tv_mode {
                    Some(TvContentMode::Latest | TvContentMode::Upcoming) => "Episode",
                    _ => "Series",
                }
                .to_string(),
            )
        } else {
            item_types
        };
        let lib = &mut self.libs[idx];
        lib.nav_stack.truncate(1);
        if let Some(root) = lib.nav_stack.first_mut() {
            root.resting = BrowseResting::new(0, 0);
            root.item_types = root_item_types;
            root.unplayed_only = unplayed_only;
            root.sort_by = sort_by;
            root.sort_order = sort_order;
            root.letter_filter = None;
            root.tv_content_mode.clone_from(&tv_mode);
        }
        lib.tv_content_mode = tv_mode;
        // A feed-view library's root scope is its first group.
        if let Some(state) = lib.feed_home_video.as_mut() {
            state.selection = FeedHomeVideoSelection::All;
            // A reset also discards any still-unresolved restored pill index:
            // the reset's own All pick is the position the user returns to.
            state.restored_group = None;
            state.video_cursor = 0;
            state.video_scroll = 0;
        }
    }
}

#[cfg(test)]
mod tests;
