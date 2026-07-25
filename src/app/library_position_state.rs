use super::types_browse::BrowseLevel;
use super::types_feed::FeedHomeVideoState;
use super::App;

impl App {
    /// Save the current position of `lib_idx` (#361 collapsed the old
    /// Default/Power scope split -- there is one view and one saved
    /// position per library now).
    pub(super) fn save_default_library_position(&mut self, lib_idx: usize) {
        let Some(lib) = self.libs.get(lib_idx) else {
            return;
        };
        let library_id = lib.library.id.clone();
        let position = lib.library_position_snapshot();
        self.library_position_state
            .libraries
            .insert(library_id, position);
        crate::config::save_library_position_state(&self.library_position_state);
    }

    /// Whether `lib_idx` is the library currently visible in the left
    /// panel -- used to decide whether a manual refresh/rescan should clear
    /// its saved position (see `refresh_lib`/`trigger_lib_rescan`).
    pub(super) fn active_library_position_scope_for(&self, lib_idx: usize) -> Option<()> {
        (self.library_tab == lib_idx + 1).then_some(())
    }

    pub(super) fn saved_library_position(
        &self,
        lib_idx: usize,
    ) -> Option<crate::config::LibraryPosition> {
        let library_id = self.libs.get(lib_idx)?.library.id.as_str();
        self.library_position_state
            .libraries
            .get(library_id)
            .cloned()
    }

    pub(super) fn replace_saved_library_position(
        &mut self,
        lib_idx: usize,
        position: crate::config::LibraryPosition,
    ) {
        let Some(lib) = self.libs.get(lib_idx) else {
            return;
        };
        self.library_position_state
            .libraries
            .insert(lib.library.id.clone(), position);
        crate::config::save_library_position_state(&self.library_position_state);
    }

    pub(super) fn focus_power_queue_initial_item(&mut self) {
        let playback = self.displayed_queue_playback_state();
        let queue = self.displayed_queue_mut();
        if playback.active && playback.active_idx < queue.items.len() {
            queue.queue_cursor = playback.active_idx;
        } else if queue.queue_cursor >= queue.items.len() && !queue.items.is_empty() {
            queue.queue_cursor = 0;
        }
    }

    pub(super) fn activate_library_position(&mut self, lib_idx: usize) {
        if lib_idx >= self.libs.len() {
            return;
        }
        let current = self
            .libs
            .get(lib_idx)
            .filter(|lib| !lib.nav_stack.is_empty())
            .map(|lib| lib.library_position_snapshot());
        let saved = self.saved_library_position(lib_idx);
        if current.as_ref() == saved.as_ref() {
            if current.is_none() {
                self.ensure_lib_loaded_for(lib_idx);
            } else if self.is_feed_home_video_library(lib_idx) || self.is_podcast_library(lib_idx) {
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    if lib.feed_home_video.is_none() {
                        lib.feed_home_video = Some(FeedHomeVideoState {
                            loading: true,
                            ..FeedHomeVideoState::default()
                        });
                    }
                }
                self.maybe_refresh_feed_groups_after_refresh(lib_idx);
            }
            return;
        }
        match saved {
            Some(position) if !position.levels.is_empty() => {
                let root = &position.levels[0];
                let restore_feed_view =
                    self.is_feed_home_video_library(lib_idx) || self.is_podcast_library(lib_idx);
                let placeholder = BrowseLevel {
                    parent_id: root.parent_id.clone(),
                    title: root.title.clone(),
                    items: Vec::new(),
                    total_count: 0,
                    cursor: 0,
                    item_types: root.item_types.clone(),
                    unplayed_only: root.unplayed_only,
                    sort_by: root.sort_by.clone(),
                    sort_order: root.sort_order.clone(),
                    loading: true,
                    scroll: 0,
                    all_items: None,
                    letter_filter: None,
                };
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    if restore_feed_view {
                        lib.feed_home_video
                            .get_or_insert_with(FeedHomeVideoState::default)
                            .loading = true;
                    }
                    lib.apply_library_position(position.clone(), vec![placeholder]);
                }
                self.spawn_restore_library_position(lib_idx, position);
            }
            _ => {
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    lib.apply_library_position(
                        crate::config::LibraryPosition::default(),
                        Vec::new(),
                    );
                }
                self.ensure_lib_loaded_for(lib_idx);
            }
        }
    }

    pub(super) fn clear_saved_library_position(&mut self, lib_idx: usize) {
        let Some(lib) = self.libs.get(lib_idx) else {
            return;
        };
        if self
            .library_position_state
            .libraries
            .remove(&lib.library.id)
            .is_none()
        {
            return;
        }
        crate::config::save_library_position_state(&self.library_position_state);
    }
}
