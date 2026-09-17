use super::types_events::{NavigateLanding, PendingSeriesHandoff};
use super::ui_util::sort_audio_tracks;
use super::{
    notify_actions::ToastSeverity, AlbumIndexState, AlbumSearchEntry, App, BrowseLevel,
    FeedHomeVideoState, LibEvent, QueueScope,
};
use mbv_core::api::EmbyItem;

impl App {
    fn handle_lib_loaded(&mut self, lib_idx: usize, parent_id: String, level: BrowseLevel) {
        // The drain's own parent id tells a root load apart from a deeper
        // level's load for the pending Series landing retry below.
        let loaded_parent_id = parent_id.clone();
        self.handle_loaded_level(lib_idx, parent_id, level);
        self.maybe_capture_library_total_and_apply_default_pill(lib_idx);
        self.maybe_auto_push_tv_season_level(lib_idx);
        self.maybe_auto_push_music_group_level(lib_idx);
        self.maybe_aggregate_feed_after_loaded(lib_idx);
        self.maybe_fetch_next_page(
            lib_idx,
            self.libs[lib_idx]
                .nav_stack
                .last()
                .map(|l| l.resting().cursor())
                .unwrap_or(0),
        );
        self.spawn_all_items_prefetch(lib_idx);
        // A pending Series landing retries once this library's ROOT level has
        // drained (U2 correction: ensure-then-land); a deeper level's load
        // re-arms and waits.
        self.retry_pending_series_landing(lib_idx, &loaded_parent_id);
    }

    /// On the FIRST unfiltered load of a library's top browse level, this
    /// captures the library's TRUE total (`LibraryTab.library_total`) --
    /// `get_user_views` doesn't carry child counts, so this fetch's
    /// `total_count` is the only place that number comes from. If the
    /// library qualifies for the letter-range pill row
    /// (`LIBRARY_PILL_THRESHOLD`) and no pill was already restored from a
    /// saved session, this applies the default (`A–C`) pill and issues one
    /// scoped refresh to replace the level's items with that range -- see
    /// plan §5. A no-op for every subsequent load of the same level
    /// (`library_total` is already `Some`), for music/feed/podcast
    /// libraries, and for non-root levels.
    fn maybe_capture_library_total_and_apply_default_pill(&mut self, lib_idx: usize) {
        let Some(lib) = self.libs.get(lib_idx) else {
            return;
        };
        if lib.library_total.is_some() || lib.library.collection_type == "music" {
            return;
        }
        if lib.nav_stack.len() != 1 {
            return;
        }
        let Some(level) = lib.nav_stack.first() else {
            return;
        };
        if level.loading || level.letter_filter.is_some() {
            return;
        }
        let total = level.total_count;
        let parent_id = level.parent_id.clone();
        let item_types = level.item_types.clone();
        let unplayed_only = level.unplayed_only;
        let sort_by = level.sort_by.clone();
        let sort_order = level.sort_order.clone();
        if let Some(lib) = self.libs.get_mut(lib_idx) {
            lib.library_total = Some(total);
        }
        if total <= super::render::LIBRARY_PILL_THRESHOLD {
            return;
        }
        let filter = super::render::LetterFilter::default_filter();
        if let Some(last) = self.libs[lib_idx].nav_stack.last_mut() {
            last.loading = true;
            last.letter_filter = Some(filter.clone());
        }
        self.spawn_refresh(
            lib_idx,
            parent_id,
            item_types,
            unplayed_only,
            sort_by,
            sort_order,
            0,
            Some(filter),
        );
    }

    fn handle_lib_page_appended(
        &mut self,
        lib_idx: usize,
        parent_id: String,
        items: Vec<EmbyItem>,
        total_count: usize,
    ) {
        let mut items = Some(items);
        self.update_current_browse_level(lib_idx, &parent_id, true, |last| {
            last.items.extend(items.take().unwrap());
            last.total_count = total_count;
            last.loading = false;
        });
        self.normalize_current_browse_level_items(lib_idx);
        self.start_or_supersede_music_grouping(lib_idx);
        self.maybe_aggregate_feed_after_page_append(lib_idx, &parent_id);
        self.maybe_fetch_next_page(
            lib_idx,
            self.libs[lib_idx]
                .nav_stack
                .last()
                .map(|l| l.resting().cursor())
                .unwrap_or(0),
        );
    }

    fn handle_lib_refreshed(
        &mut self,
        lib_idx: usize,
        parent_id: String,
        item_types: Option<String>,
        unplayed_only: bool,
        items: Vec<EmbyItem>,
        total_count: usize,
    ) {
        let is_feed_video_refresh = self.is_feed_home_video_library(lib_idx)
            && item_types.as_deref() == Some("Video")
            && unplayed_only;
        if !is_feed_video_refresh {
            let mut items = Some(items);
            self.update_current_browse_level(lib_idx, &parent_id, false, |last| {
                last.items = items.take().unwrap();
                last.total_count = total_count;
                last.loading = false;
            });
        }
        self.normalize_current_browse_level_items(lib_idx);
        self.start_or_supersede_music_grouping(lib_idx);
        self.maybe_refresh_feed_groups_after_refresh(lib_idx);
        self.spawn_all_items_prefetch(lib_idx);
    }

    fn handle_restored_library_position(
        &mut self,
        lib_idx: usize,
        requested_position: crate::config::LibraryPosition,
        position: crate::config::LibraryPosition,
        nav_stack: Vec<BrowseLevel>,
    ) {
        if self.saved_library_position(lib_idx).as_ref() != Some(&requested_position) {
            return;
        }
        // A restore an armed pending Series landing is waiting on is never
        // stale: the landing spawned it and cannot retry until it applies,
        // and the landing is initiated from another tab (queue "Go to
        // Library") whose tab switch happens only on completion.
        let serves_pending_landing = self
            .pending_series_landing
            .as_ref()
            .is_some_and(|pending| pending.lib_idx == lib_idx);
        if self.active_library_position_scope_for(lib_idx).is_none() && !serves_pending_landing {
            return;
        }
        if let Some(lib) = self.libs.get_mut(lib_idx) {
            lib.apply_library_position(position.clone(), nav_stack);
        }
        // Positions saved before the letter-pill feature existed carry no
        // `library_total`, so without this call `should_show_letter_pills`
        // would stay false forever for those libraries. This is a no-op for
        // saves that already have `library_total` set (see the function's
        // own early-return checks).
        self.maybe_capture_library_total_and_apply_default_pill(lib_idx);
        self.maybe_refresh_feed_groups_after_refresh(lib_idx);
        let restored = self
            .libs
            .get(lib_idx)
            .map(|lib| lib.library_position_snapshot());
        if restored.as_ref() != self.saved_library_position(lib_idx).as_ref() {
            if let Some(restored) = restored {
                self.replace_saved_library_position(lib_idx, restored);
            }
        }
        // A pending Series landing retries against the restored root corpus.
        // `arm` only pays the whole-library prefetch for a user-initiated
        // pending landing, so a startup restore stays in the no-prefetch
        // regime the note below protects.
        if let Some(parent_id) = self
            .libs
            .get(lib_idx)
            .and_then(|lib| lib.nav_stack.first())
            .map(|lvl| lvl.parent_id.clone())
        {
            self.retry_pending_series_landing(lib_idx, &parent_id);
        }
        // Deliberately no `spawn_all_items_prefetch` call here (unlike
        // `handle_lib_loaded`'s sibling call, which is safe): this method
        // fires for every library restored at app *startup*, all
        // concurrently. Eagerly fetching+parsing a whole library's worth of
        // full-field items (People, MediaStreams, ...) here piles CPU-bound
        // JSON parsing on top of N other libraries' simultaneous restore
        // fetches and visibly stalls first paint of the default library
        // (#260). `all_items` is a pure cache for instant fuzzy-search open
        // via the unified search modal. The modal reads it lazily
        // (see `AllItemsPrefetched` handling), so nothing here requires
        // it to be warm. If you're tempted to add
        // this back, don't: benchmark against a library with 500+ items
        // first and check `~/.local/state/mbv/mbv.log` for `parent=<id>`
        // `http=`/`parse=` timings from `get_items_sorted`.
    }

    pub(super) fn handle_lib_event(&mut self, ev: LibEvent) {
        if let LibEvent::AudiobookshelfProgressAcknowledged(update) = ev {
            if !self.audiobookshelf_runtime.accepts(update.generation) {
                return;
            }
            let position_ticks =
                super::audiobookshelf_browse_actions::seconds_to_ticks(update.current_time_seconds);
            self.reconcile_audiobookshelf_progress(
                &update.library_item_id,
                &update.episode_id,
                position_ticks,
                update.current_time_seconds,
                update.is_finished,
            );
            return;
        }
        if let LibEvent::AudiobookshelfBookProgressAcknowledged(update) = ev {
            if !self.audiobookshelf_runtime.accepts(update.generation) {
                return;
            }
            let position_ticks =
                super::audiobookshelf_browse_actions::seconds_to_ticks(update.current_time_seconds);
            self.reconcile_audiobookshelf_book_progress(
                &update.library_item_id,
                position_ticks,
                update.is_finished,
            );
            return;
        }
        if let LibEvent::AudiobookshelfBooksFetched {
            generation,
            library_id,
            result,
        } = ev
        {
            if !self.audiobookshelf_runtime.accepts(generation) {
                return;
            }
            if let Some(index) = self
                .audiobookshelf_libraries
                .iter()
                .position(|library| library.id == library_id)
            {
                let mut next_page = None;
                let mut selected_detail = None;
                if let Some(state) = self.audiobookshelf_book_browse.get_mut(index) {
                    match result {
                        Ok(page) => {
                            state.append_page_books(page.page, page.total, page.items);
                            next_page = state.needs_page();
                            if !state.detail_loading {
                                selected_detail = state.selected_id.clone();
                            }
                        }
                        Err(error) => state.error = Some(error.to_string()),
                    }
                }
                if let Some(selected_detail) = selected_detail {
                    self.start_audiobookshelf_book_detail(selected_detail);
                }
                if let Some(next_page) = next_page {
                    super::service_startup::start_audiobookshelf_books(
                        self.config.lock().unwrap().clone(),
                        generation,
                        library_id,
                        next_page,
                        self.lib_tx.clone(),
                    );
                }
            }
            return;
        }
        if let LibEvent::AudiobookshelfBookDetailFetched {
            generation,
            library_item_id,
            result,
        } = ev
        {
            if !self.audiobookshelf_runtime.accepts(generation) {
                return;
            }
            match result {
                Ok(detail) => {
                    if let Some(state) = self.audiobookshelf_book_browse.iter_mut().find(|state| {
                        state
                            .books
                            .iter()
                            .any(|book| book.library_item_id == library_item_id)
                    }) {
                        state.detail_loading_ids.remove(&library_item_id);
                        state.detail_loading = state
                            .selected_id
                            .as_ref()
                            .is_some_and(|id| state.detail_loading_ids.contains(id));
                        state.detail_cache.insert(library_item_id.clone(), detail);
                    }
                }
                Err(_error) => {
                    if let Some(state) = self.audiobookshelf_book_browse.iter_mut().find(|state| {
                        state
                            .books
                            .iter()
                            .any(|book| book.library_item_id == library_item_id)
                    }) {
                        state.detail_loading_ids.remove(&library_item_id);
                        state.detail_loading = state
                            .selected_id
                            .as_ref()
                            .is_some_and(|id| state.detail_loading_ids.contains(id));
                    }
                }
            }
            return;
        }
        if let LibEvent::AudiobookshelfDetailFetched {
            generation,
            library_item_id,
            result,
        } = ev
        {
            if !self.audiobookshelf_runtime.accepts(generation) {
                return;
            }
            let state = self.audiobookshelf_browse.iter_mut().find(|state| {
                state
                    .shows
                    .iter()
                    .any(|show| show.library_item_id == library_item_id)
            });
            match result {
                Ok(episodes) => {
                    if let Some(state) = state {
                        state.detail_loading = false;
                        state.cache_detail(library_item_id.clone(), episodes.clone());
                        if state.selected_id.as_deref() == Some(&library_item_id) {
                            state.episodes = Some(episodes);
                        }
                    }
                }
                Err(_error) => {
                    if let Some(state) = state {
                        state.detail_loading = false;
                    }
                }
            }
            return;
        }
        if let LibEvent::AudiobookshelfShowsFetched {
            generation,
            library_id,
            result,
        } = ev
        {
            if !self.audiobookshelf_runtime.accepts(generation) {
                return;
            }
            if let Some(index) = self
                .audiobookshelf_libraries
                .iter()
                .position(|library| library.id == library_id)
            {
                let mut next_page = None;
                let mut selected_detail = None;
                if let Some(state) = self.audiobookshelf_browse.get_mut(index) {
                    match result {
                        Ok(page) => {
                            state.append_page(page.page, page.limit, page.total, page.items);
                            next_page = state.needs_page();
                            if state.episodes.is_none() && !state.detail_loading {
                                selected_detail = state.selected_id.clone();
                            }
                        }
                        Err(error) => state.error = Some(error.to_string()),
                    }
                }
                if let Some(selected_detail) = selected_detail {
                    self.start_audiobookshelf_detail(selected_detail);
                }
                if let Some(next_page) = next_page {
                    super::service_startup::start_audiobookshelf_shows(
                        self.config.lock().unwrap().clone(),
                        generation,
                        library_id,
                        next_page,
                        self.lib_tx.clone(),
                    );
                }
            }
            return;
        }
        if let LibEvent::AudiobookshelfShelfFetched {
            generation,
            library_id,
            result,
        } = ev
        {
            if !self.audiobookshelf_runtime.accepts(generation) {
                return;
            }
            if let Ok(shelves) = result {
                let items = App::newest_episodes_items(shelves);
                self.audiobookshelf_shelf_cache.insert(library_id, items);
                // The App owns the shelf cache; the cross-provider pill splice
                // runs in the shell against Model-owned `latest` (task 5.3d).
                // The lib_rx while-drain picks this up in the same drain pass.
                let _ = self.lib_tx.send(LibEvent::AudiobookshelfLatestRebuilt(
                    self.audiobookshelf_latest_sections(),
                ));
            }
            return;
        }
        match ev {
            LibEvent::Loaded {
                lib_idx,
                parent_id,
                level,
            } => self.handle_lib_loaded(lib_idx, parent_id, *level),
            LibEvent::PageAppended {
                lib_idx,
                parent_id,
                items,
                total_count,
            } => self.handle_lib_page_appended(lib_idx, parent_id, items, total_count),
            LibEvent::Refreshed {
                lib_idx,
                parent_id,
                item_types,
                unplayed_only,
                items,
                total_count,
            } => self.handle_lib_refreshed(
                lib_idx,
                parent_id,
                item_types,
                unplayed_only,
                items,
                total_count,
            ),
            LibEvent::RestoreLibraryPosition {
                lib_idx,
                requested_position,
                position,
                nav_stack,
            } => self.handle_restored_library_position(
                lib_idx,
                requested_position,
                position,
                nav_stack,
            ),
            LibEvent::SearchItemsLoaded {
                lib_idx,
                parent_id,
                items,
            } => {
                // The flat inline-search fetch re-homes the write the deleted
                // direct flat-result projector used to do against the
                // component: the completion lands in the nav level's
                // `all_items` cache (the same guarded write as
                // `AllItemsPrefetched`) and the shell's event-scoped
                // projection (5.3d.20c) pushes it into the component. A
                // completion racing a navigation -- `parent_id` no longer the
                // last level's -- is stale and must not write.
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    if let Some(last) = lib.nav_stack.last_mut() {
                        if last.parent_id == parent_id {
                            last.all_items = Some(items);
                        }
                    }
                }
            }
            LibEvent::AlbumIndexBuilt { library_id, result } => {
                let rebuild_pending = matches!(
                    self.album_indexes.get(&library_id),
                    Some(AlbumIndexState::Loading {
                        rebuild_pending: true
                    })
                );
                if rebuild_pending {
                    self.album_indexes.insert(
                        library_id.clone(),
                        AlbumIndexState::Loading {
                            rebuild_pending: false,
                        },
                    );
                    self.spawn_album_index_build(library_id);
                } else {
                    match result {
                        Ok(entries) => {
                            self.album_indexes
                                .insert(library_id.clone(), AlbumIndexState::Ready(entries));
                        }
                        Err(error) => {
                            self.album_indexes
                                .insert(library_id.clone(), AlbumIndexState::Unavailable);
                            self.flash(
                                format!("Couldn't load album index: {error}"),
                                ToastSeverity::Error,
                            );
                        }
                    }
                }
            }
            LibEvent::RecursiveAlbumActivated {
                library_id,
                nav_stack,
            } => {
                let Some(lib_idx) = self
                    .libs
                    .iter()
                    .position(|lib| lib.library.id == library_id)
                else {
                    return;
                };
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    lib.nav_stack = nav_stack;
                }
                // Entering inline track focus for the activated album is the
                // shell's job now (the component owns the cursor; the shell
                // delivers a one-shot enter request at the next sync — wide
                // only, narrow stays unfocused).
                self.save_default_library_position(lib_idx);
                // A `NavigateLanding::Album` landing defers its tab switch to
                // this drain (D4): the landed stack has replaced the nav
                // stack and the saved position above, so the switch's
                // activation compares equal and never restores. Consume it
                // only when this landing belongs to the pending navigation's
                // library; an Inline Search activation, or any other
                // library's landing, must leave it armed (U2 correction).
                let belongs_to_pending = self.pending_navigate_tab_switch.is_some_and(|idx| {
                    self.libs
                        .get(idx)
                        .is_some_and(|lib| lib.library.id == library_id)
                });
                if belongs_to_pending {
                    if let Some(idx) = self.pending_navigate_tab_switch.take() {
                        self.set_library_tab(idx + 1);
                    }
                }
            }
            LibEvent::AllItemsPrefetched {
                lib_idx,
                parent_id,
                items,
            } => {
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    if let Some(last) = lib.nav_stack.last_mut() {
                        if last.parent_id == parent_id {
                            last.all_items = Some(items);
                        }
                    }
                }
                // The whole-library corpus is exactly what a pending Series
                // landing was waiting for (U2 correction).
                self.retry_pending_series_landing(lib_idx, &parent_id);
            }
            LibEvent::FeedHomeVideoAggregated {
                lib_idx,
                parent_id,
                all_items,
                groups,
            } => {
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    if lib
                        .nav_stack
                        .first()
                        .map(|root| root.parent_id == parent_id)
                        .unwrap_or(false)
                    {
                        let (selected_group, video_cursor, video_scroll) = lib
                            .feed_home_video
                            .as_ref()
                            .map(|state| {
                                (state.selected_group, state.video_cursor, state.video_scroll)
                            })
                            .unwrap_or((0, 0, 0));
                        lib.feed_home_video = Some(FeedHomeVideoState {
                            all_items,
                            groups,
                            loading: false,
                            selected_group,
                            video_cursor,
                            video_scroll,
                        });
                    }
                }
                self.clamp_feed_home_video_state(lib_idx);
                self.log_feed_home_video_state(lib_idx, "aggregated");
            }
            LibEvent::AlbumTracksFetched {
                album_id,
                mut tracks,
            } => {
                self.album_tracks_loading.remove(&album_id);
                // The cache is also the cursor's source of truth while the
                // album is open, so normalize it once before rendering or
                // resolving the focused track for playback.
                sort_audio_tracks(&mut tracks);
                self.album_tracks_cache.insert(album_id, tracks);
            }
            LibEvent::SeriesDetailFetched {
                series_id,
                seasons,
                episodes,
            } => self.handle_series_detail_fetched(
                series_id,
                crate::app::SeriesDetail { seasons, episodes },
            ),
            LibEvent::SeriesSeasonEpisodesFetched {
                series_id,
                season_id,
                episodes,
            } => self.handle_series_season_episodes_fetched(series_id, season_id, episodes),
            LibEvent::AudiobookshelfDetailFetched { .. }
            | LibEvent::AudiobookshelfShowsFetched { .. }
            | LibEvent::AudiobookshelfBooksFetched { .. }
            | LibEvent::AudiobookshelfBookDetailFetched { .. }
            | LibEvent::AudiobookshelfShelfFetched { .. }
            | LibEvent::AudiobookshelfProgressAcknowledged(_)
            | LibEvent::AudiobookshelfBookProgressAcknowledged(_) => unreachable!(),
            LibEvent::AlbumArtistFetched { album_id, artist } => {
                self.album_artist_loading.remove(&album_id);
                self.album_artist_cache
                    .insert(album_id.clone(), artist.clone());
                self.album_artist_fetches_active =
                    self.album_artist_fetches_active.saturating_sub(1);
                self.drain_album_artist_fetches();
                self.advance_music_grouping_candidates(&album_id, &artist);
            }
            LibEvent::NavigateTo {
                lib_idx,
                landing,
                switch_tab,
            } => {
                match landing {
                    NavigateLanding::Chain { nav_stack } => {
                        if let Some(lib) = self.libs.get_mut(lib_idx) {
                            lib.nav_stack = nav_stack;
                        }
                        // A completed navigation IS the saved position from now on;
                        // without this the `switch_tab` activation below compares the
                        // navigated stack against the stale saved position, takes the
                        // restore branch, and clobbers the navigation the user asked
                        // for (queue "Go to Library" / search-sidebar activation
                        // degraded to a bare tab switch).
                        if self.libs.get(lib_idx).is_some() {
                            self.save_default_library_position(lib_idx);
                        }
                        if switch_tab {
                            self.set_library_tab(lib_idx + 1);
                        }
                    }
                    NavigateLanding::Series { reveal, episode_id } => {
                        let name = reveal.name.clone();
                        if !self.libs.get(lib_idx).is_some() {
                            self.flash_error(format!("Could not land on '{name}' in its library"));
                        } else if self.activate_searched_series(lib_idx, &reveal) {
                            // D4: the landed root level (pill + cursor) is the
                            // saved position from now on; the fence in
                            // `handle_restored_library_position` then discards
                            // any stale pre-navigation restore.
                            self.save_default_library_position(lib_idx);
                            if switch_tab {
                                self.set_library_tab(lib_idx + 1);
                            }
                            // The landing completed; the Model drain owes the
                            // detail hand-off (task 3.1, design D3).
                            self.pending_series_handoff = Some(PendingSeriesHandoff {
                                lib_idx,
                                reveal,
                                episode_id,
                            });
                        } else if !self
                            .arm_pending_series_landing(lib_idx, reveal, switch_tab, episode_id)
                        {
                            // Miss against a complete corpus (absent item, an
                            // unloadable library): flash the library-error
                            // path and leave the active tab unchanged (task
                            // 4.2's semantics). A satisfiable-but-not-yet
                            // corpus was armed above instead (U2 correction).
                            self.flash_error(format!("Could not land on '{name}' in its library"));
                        }
                    }
                    NavigateLanding::Album {
                        reveal,
                        ancestors,
                        track_id,
                    } => {
                        let entry = AlbumSearchEntry::from_chain(*reveal, ancestors);
                        // Fully async, exactly like Inline Search's album
                        // activation: the nav stack is replaced (and the
                        // landed position saved) on the
                        // `RecursiveAlbumActivated` drain, which then consumes
                        // `pending_navigate_tab_switch` so the tab switch
                        // never compares the landed stack against the stale
                        // saved position (D4).
                        if self.activate_recursive_album(lib_idx, entry) {
                            if switch_tab {
                                self.pending_navigate_tab_switch = Some(lib_idx);
                            }
                            // Deep selection (task 6.2, design D6): the
                            // chosen track rides the activation; the shell
                            // binds it to the activated album at the
                            // `RecursiveAlbumActivated` drain.
                            self.pending_track_selection =
                                track_id.map(|track_id| (lib_idx, track_id));
                        } else {
                            self.flash_error("Could not start the album navigation".to_string());
                        }
                    }
                }
            }
            LibEvent::PlaylistsLoaded(items) => {
                self.playlists = items;
                self.playlists_loading = false;
                self.playlists_cursor = self
                    .playlists_cursor
                    .min(self.playlists.len().saturating_sub(1));
            }
            LibEvent::PlaylistsLoadError(e) => {
                self.playlists_loading = false;
                self.flash_error(e);
            }
            LibEvent::PlaylistItemsLoaded { playlist_id, items } => {
                if self
                    .playlists_open
                    .as_ref()
                    .map(|p| p.id == playlist_id)
                    .unwrap_or(false)
                {
                    self.playlists_open_items = items;
                    self.playlists_open_loading = false;
                }
            }
            LibEvent::PlaylistItemsLoadError { playlist_id, error } => {
                if self
                    .playlists_open
                    .as_ref()
                    .map(|p| p.id == playlist_id)
                    .unwrap_or(false)
                {
                    self.playlists_open_loading = false;
                }
                self.flash_error(error);
            }
            LibEvent::PlaylistRenamed { new_name } => {
                self.dismiss_save_playlist();
                self.force_clear = true;
                self.flash(format!("Renamed to '{new_name}'"), ToastSeverity::Success);
            }
            LibEvent::PlaylistDeleted { name } => {
                self.dismiss_confirm();
                self.flash(format!("Deleted '{name}'"), ToastSeverity::Success);
            }
            LibEvent::QueueEnriched { items } => {
                let _ = self.merge_refreshed_queue(QueueScope::Local, items);
            }
            // Shell-intercepted Home content-delivery variants (task 5.3d):
            // the lib_rx drain handles them at the Model boundary, so they
            // are unreachable here; the arms keep the exhaustive match total.
            LibEvent::HomeContentRefreshed(_)
            | LibEvent::HomeContentCleared
            | LibEvent::AudiobookshelfLatestRebuilt(_)
            | LibEvent::FeedsLatestRebuilt(_) => {}
            LibEvent::Error(e) => {
                // A failed per-kind activation reports through here; drop the
                // deferred tab switch and the pending Series landing so
                // neither can fire on a later, unrelated drain (U2
                // correction). `pending_series_handoff` deliberately survives:
                // it is only armed once the landing already succeeded, and an
                // unscoped later error must not swallow the pending workspace/
                // overlay open -- it is consumed by the next sync pass.
                self.pending_navigate_tab_switch = None;
                self.pending_series_landing = None;
                self.flash(format!("Library error: {e}"), ToastSeverity::Error);
            }
        }
    }
}
