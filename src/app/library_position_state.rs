use super::types_browse::BrowseLevel;
use super::types_feed::FeedHomeVideoState;
use super::App;
use std::time::{Duration, Instant};

/// How long a library-position change must sit unflushed before the
/// deferred write in `flush_library_position_if_idle` fires. Keeps rapid
/// scrolling (arrow-key repeat, mouse wheel, PageUp/PageDown) from doing a
/// disk write plus a blocking shared-document round trip on every single
/// step -- see `save_default_library_position`'s doc comment.
const LIBRARY_POSITION_FLUSH_DELAY: Duration = Duration::from_millis(150);

impl App {
    /// Records the current position of `lib_idx` in memory (#361 collapsed
    /// the old Default/Power scope split -- there is one view and one saved
    /// position per library now). The disk write and shared-document sync
    /// are deferred: this is called on every cursor move (arrow keys,
    /// PageUp/Down, mouse wheel), so doing that I/O here would put a
    /// synchronous disk write -- and, when a shared/roaming daemon is
    /// attached, a blocking IPC round trip -- on every scroll tick. Callers
    /// that need the in-memory state (tests, immediate reads) still see it
    /// updated synchronously; only the persistence is deferred, via
    /// `flush_library_position_if_idle` (called from the run loop) and
    /// `flush_library_position_now` (called at teardown so a final burst is
    /// never lost).
    pub(super) fn save_default_library_position(&mut self, lib_idx: usize) {
        let Some(lib) = self.libs.get(lib_idx) else {
            return;
        };
        let library_id = lib.library.id.clone();
        let position = lib.library_position_snapshot();
        self.library_position_state
            .libraries
            .insert(library_id, position);
        self.library_position_dirty = true;
        self.library_position_dirty_at = Instant::now();
    }

    /// Flushes a pending library-position change once it has sat unflushed
    /// for `LIBRARY_POSITION_FLUSH_DELAY` -- called each run-loop
    /// iteration. A steady stream of cursor moves keeps resetting
    /// `library_position_dirty_at`, so the write only lands once scrolling
    /// pauses.
    pub(in crate::app) fn flush_library_position_if_idle(&mut self) {
        if self.library_position_dirty
            && self.library_position_dirty_at.elapsed() >= LIBRARY_POSITION_FLUSH_DELAY
        {
            self.flush_library_position_now();
        }
    }

    /// Unconditionally persists the in-memory library-position state,
    /// regardless of how recently it changed. Used at teardown so a
    /// position change made just before quitting is never dropped.
    pub(in crate::app) fn flush_library_position_now(&mut self) {
        if !self.library_position_dirty {
            return;
        }
        self.library_position_dirty = false;
        crate::config::save_library_position_state(&self.library_position_state);
        if let Ok(value) = serde_json::to_value(&self.library_position_state) {
            if let Err(error) = self.persist_shared_document(
                mbv_core::shared_state::SharedDocumentKind::LibraryPositionState,
                value,
            ) {
                log::warn!(
                    target: "shared_data",
                    "library position persistence failed: {error}"
                );
            }
        }
    }

    /// Whether `lib_idx` is the library currently visible in the left
    /// panel -- used to decide whether a manual refresh/rescan should clear
    /// its saved position (see `refresh_lib`/`trigger_lib_rescan`).
    pub(super) fn active_library_position_scope_for(&self, lib_idx: usize) -> Option<()> {
        (self.tab.library_index() == Some(lib_idx)).then_some(())
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
        if let Ok(value) = serde_json::to_value(&self.library_position_state) {
            let _ = self.persist_shared_document(
                mbv_core::shared_state::SharedDocumentKind::LibraryPositionState,
                value,
            );
        }
    }

    pub(super) fn focus_queue_initial_item(&mut self) {
        let playback = self.displayed_queue_playback_state();
        let queue = self.displayed_queue_mut();
        let total = queue.total_queue_len();
        if playback.active && playback.active_idx < total {
            queue.queue_cursor = playback.active_idx;
        } else if queue.queue_cursor >= total && total > 0 {
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
                    music_grouping: None,
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
        if let Ok(value) = serde_json::to_value(&self.library_position_state) {
            let _ = self.persist_shared_document(
                mbv_core::shared_state::SharedDocumentKind::LibraryPositionState,
                value,
            );
        }
    }

    fn audiobookshelf_position_key(&self, index: usize) -> Option<String> {
        let library = self.audiobookshelf_libraries.get(index)?;
        let server = self
            .config
            .lock()
            .unwrap()
            .audiobookshelf_setup
            .as_ref()?
            .server_url
            .clone();
        Some(format!("audiobookshelf:{server}:{}", library.id))
    }

    pub(super) fn save_audiobookshelf_position(&mut self, index: usize) {
        let Some(key) = self.audiobookshelf_position_key(index) else {
            return;
        };
        let Some(state) = self.audiobookshelf_browse.get(index) else {
            return;
        };
        let position = crate::config::LibraryPosition {
            levels: vec![crate::config::LibraryPositionLevel {
                parent_id: state.library.id.clone(),
                title: state.library.name.clone(),
                focused_item_id: state.selected_id.clone(),
                cursor_index: state.cursor(),
                item_types: Some("podcast".into()),
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                letter_filter_index: None,
                library_total: Some(state.total),
            }],
            ..Default::default()
        };
        self.library_position_state.libraries.insert(key, position);
        self.library_position_dirty = true;
        self.library_position_dirty_at = Instant::now();
    }

    pub(super) fn activate_audiobookshelf_position(&mut self, index: usize) {
        let saved = self
            .audiobookshelf_position_key(index)
            .and_then(|key| self.library_position_state.libraries.get(&key).cloned());
        let Some(state) = self.audiobookshelf_browse.get_mut(index) else {
            return;
        };
        if state.selected_id.is_none() {
            state.selected_id = saved
                .as_ref()
                .and_then(|position| position.levels.first())
                .and_then(|level| level.focused_item_id.clone());
        }
        let Some(id) = state.selected_id.clone() else {
            if !state.shows.is_empty() {
                state.select(0);
            }
            return;
        };
        if state.shows.iter().any(|show| show.library_item_id == id) {
            state.episodes = state.detail_cache.get(&id).cloned();
        }
        if self.tab.audiobookshelf_index() == Some(index) {
            self.start_audiobookshelf_detail(id);
        }
    }
}
