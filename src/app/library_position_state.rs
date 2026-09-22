use super::types_browse::BrowseLevel;
use super::types_browse::BrowseResting;
use super::types_feed::FeedHomeVideoState;
use super::App;
impl App {
    /// Keeps the legacy browse snapshot current in memory for the migration
    /// reader. It deliberately never writes the legacy per-library document;
    /// launch state is serialized only by the orderly-exit snapshot path.
    pub(super) fn persist_library_scroll(&mut self, lib_idx: usize, scroll: usize) {
        if let Some(level) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.nav_stack.last_mut())
        {
            level.set_resting_scroll(scroll);
            self.save_default_library_position(lib_idx);
        }
    }

    /// Updates the legacy snapshot only in memory. The legacy document is a
    /// startup migration source, never a live persistence target.
    pub(super) fn save_default_library_position(&mut self, lib_idx: usize) {
        let Some(lib) = self.libs.get(lib_idx) else {
            return;
        };
        let library_id = lib.library.id.clone();
        let position = lib.library_position_snapshot();
        self.library_position_state
            .libraries
            .insert(library_id, position);
    }

    /// Whether `lib_idx` is the library currently visible in the left
    /// panel -- used to decide whether a manual refresh/rescan should clear
    /// its saved position (see `refresh_lib`/`trigger_lib_rescan`).
    pub(super) fn active_library_position_scope_for(&self, lib_idx: usize) -> Option<()> {
        (self.tab.emby_library_index() == Some(lib_idx)).then_some(())
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
    }

    pub(super) fn focus_queue_initial_item(&mut self) {
        let playback = self.displayed_queue_playback_state();
        let queue = self.displayed_queue_mut();
        let total = queue.total_queue_len();
        let active_slot = playback
            .active
            .then_some(playback.active_idx)
            .flatten()
            .filter(|idx| *idx < total);
        if let Some(idx) = active_slot {
            queue.queue_cursor = idx;
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
            } else if self.is_feed_home_video_library(lib_idx) {
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
                let restore_feed_view = self.is_feed_home_video_library(lib_idx);
                let placeholder = BrowseLevel {
                    fetched_rows: 0,
                    parent_id: root.parent_id.clone(),
                    title: root.title.clone(),
                    items: Vec::new(),
                    total_count: 0,
                    resting: BrowseResting::new(0, 0),
                    item_types: root.item_types.clone(),
                    unplayed_only: root.unplayed_only,
                    sort_by: root.sort_by.clone(),
                    sort_order: root.sort_order.clone(),
                    loading: true,

                    all_items: None,
                    letter_filter: root.letter_filter_index.and_then(|index| {
                        let filter_kind =
                            crate::app::render::LetterFilterKind::from_collection_type(
                                self.libs[lib_idx].library.collection_type.as_str(),
                            );
                        if filter_kind == crate::app::render::LetterFilterKind::Tv
                            && index
                                >= crate::app::render::LetterFilter::count_for_kind(filter_kind)
                        {
                            None
                        } else {
                            crate::app::render::LetterFilter::for_index_for_kind(index, filter_kind)
                        }
                    }),
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
        self.library_position_state
            .libraries
            .remove(&lib.library.id);
    }

    pub(super) fn audiobookshelf_position_key(&self, index: usize) -> Option<String> {
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
                fetched_rows: None,
                parent_id: state.library.id.clone(),
                title: state.library.name.clone(),
                // The podcast tab's selection is the active pill plus the
                // selected episode, not a show id: a saved position no longer
                // records one (design: the remembered pill is session memory
                // and restore ignores the old show-id values; no migration).
                focused_item_id: None,
                cursor_index: 0,
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
    }

    /// Book-shaped sibling of `save_audiobookshelf_position`, keyed by the
    /// same per-library position slot (so podcast and book libraries sharing
    /// a server never collide) and carrying the book list cursor.
    pub(super) fn save_audiobookshelf_book_position(&mut self, index: usize) {
        let Some(key) = self.audiobookshelf_position_key(index) else {
            return;
        };
        let Some(state) = self.audiobookshelf_book_browse.get(index) else {
            return;
        };
        let position = crate::config::LibraryPosition {
            levels: vec![crate::config::LibraryPositionLevel {
                fetched_rows: None,
                parent_id: state.library.id.clone(),
                title: state.library.name.clone(),
                focused_item_id: state.selected_id.clone(),
                cursor_index: state.cursor(),
                item_types: Some("book".into()),
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                letter_filter_index: None,
                library_total: Some(state.total),
            }],
            ..Default::default()
        };
        self.library_position_state.libraries.insert(key, position);
    }

    pub(super) fn activate_audiobookshelf_position(&mut self, index: usize) {
        // A saved position names a show id under the retired show-browser
        // model; restore ignores the saved value entirely (design: no
        // migration). Tab activation is a refresh trigger (design D5): the
        // active pill's required shows are re-requested and their episodes
        // replaced.
        if self.tab.audiobookshelf_index() != Some(index) {
            return;
        }
        self.refresh_audiobookshelf_podcast_pill_shows(index);
    }

    /// Tab-activation refresh for the podcast tab (design D5): drop the
    /// committed pill's required shows' landed episode caches, then re-arm
    /// the bounded fan-out. A show with a fetch still in flight keeps its
    /// single request: stripping its mark here would re-request it and let
    /// the orphaned response clear the new request's mark, multiplying
    /// requests across tab ping-pong — the in-flight response lands into the
    /// cache instead. A show pill re-requests that show only; a state pill
    /// re-requests every listed show.
    fn refresh_audiobookshelf_podcast_pill_shows(&mut self, index: usize) {
        let Some(state) = self.audiobookshelf_browse.get_mut(index) else {
            return;
        };
        let required: Vec<String> = match state.committed_show_pill.as_ref() {
            Some(id) => vec![id.clone()],
            None => state
                .shows
                .iter()
                .map(|show| show.library_item_id.clone())
                .collect(),
        };
        for id in &required {
            if !state.detail_loading_ids.contains_key(id) {
                state.detail_cache.remove(id);
            }
        }
        self.start_audiobookshelf_podcast_fan_out(index);
    }

    /// Book-shaped sibling of `activate_audiobookshelf_position`. A saved
    /// position's `item_types` distinguishes book from podcast slots; only a
    /// book-typed slot is honored for a book library.
    pub(super) fn activate_audiobookshelf_book_position(&mut self, index: usize) {
        let saved = self
            .audiobookshelf_position_key(index)
            .and_then(|key| self.library_position_state.libraries.get(&key).cloned());
        let Some(state) = self.audiobookshelf_book_browse.get_mut(index) else {
            return;
        };
        let saved_is_book = saved
            .as_ref()
            .and_then(|position| position.levels.first())
            .and_then(|level| level.item_types.as_deref())
            == Some("book");
        if state.selected_id.is_none() {
            state.selected_id = if saved_is_book {
                saved
                    .as_ref()
                    .and_then(|position| position.levels.first())
                    .and_then(|level| level.focused_item_id.clone())
            } else {
                None
            };
        }
        let Some(id) = state.selected_id.clone() else {
            if !state.books.is_empty() {
                state.select(0);
            }
            return;
        };
        if self.tab.audiobookshelf_index() == Some(index)
            && !state.books.is_empty()
            && !state.detail_cache.contains_key(&id)
        {
            self.start_audiobookshelf_book_detail(id);
        }
    }
}
