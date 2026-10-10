use crate::app::state::app_struct::LevelFillState;
use crate::app::state::events::{
    LibEvent, ModelContentEvent, MusicEvent, NavigateLanding, PendingSeriesHandoff, PlaylistEvent,
    SeriesEvent,
};
use crate::app::{
    AlbumIndex, AlbumIndexState, AlbumSearchEntry, App, FeedHomeVideoState,
    dispatch::notify::ToastSeverity,
};
use mbv_emby_model::EmbyItem;
use mbv_ui_model::ui_util::sort_audio_tracks;

mod audiobookshelf;
mod browse_loads;

fn feed_home_video_selection(state: &FeedHomeVideoState) -> (usize, usize, usize) {
    (state.selected_group, state.video_cursor, state.video_scroll)
}

/// Each variant keeps its own documented no-op arm so a future
/// `ModelContentEvent` variant that must reach the shell drain cannot
/// silently fall through here.
fn handle_model_content_event(ev: ModelContentEvent) {
    match ev {
        // The shell drain applies the Model-owned latest snapshot.
        ModelContentEvent::EmbyLatestSnapshotFetched {
            library_id,
            title,
            items,
        } => drop((library_id, title, items)),
        // The shell drain applies the Model-owned refreshed content.
        ModelContentEvent::HomeContentRefreshed(content) => drop(content),
        // Clearing Model-owned content is handled by the shell drain.
        ModelContentEvent::HomeContentCleared => {}
    }
}

impl App {
    pub(in crate::app) fn handle_lib_event(&mut self, ev: LibEvent) {
        match ev {
            LibEvent::Browse(event) => self.handle_browse_event(event),
            LibEvent::Music(event) => self.handle_music_event(event),
            LibEvent::Series(event) => self.handle_series_event(event),
            LibEvent::Audiobookshelf(event) => self.handle_audiobookshelf_event(event),
            LibEvent::Playlist(event) => self.handle_playlist_event(event),
            // The shell drain applies Model-owned content.
            LibEvent::ModelContent(event) => handle_model_content_event(event),
            LibEvent::Error(error) => self.handle_error(&error),
        }
    }

    fn handle_music_event(&mut self, ev: MusicEvent) {
        match ev {
            MusicEvent::AlbumIndexBuilt { library_id, result } => {
                self.handle_album_index_built(library_id, result);
            }
            MusicEvent::RecursiveAlbumActivated {
                library_id,
                nav_stack,
            } => self.handle_recursive_album_activated(&library_id, nav_stack),
            MusicEvent::AlbumArtistLevelFetched { level_id, artists } => {
                self.handle_album_artist_level_fetched(level_id, artists);
            }
            MusicEvent::GroupWarmupListed { generation, groups } => {
                self.handle_music_group_warmup_listed(generation, groups);
            }
            MusicEvent::AlbumTracksFetched { album_id, tracks } => {
                self.handle_album_tracks_fetched(album_id, tracks);
            }
            MusicEvent::ArtistTracksFetched {
                destination,
                generation,
                artist_id,
                revision,
                result,
            } => {
                let result: Result<Vec<mbv_emby_model::EmbyItem>, mbv_emby::EmbyError> = result;
                self.handle_artist_tracks_fetched(
                    &destination,
                    generation,
                    &artist_id,
                    revision,
                    result,
                );
            }
            MusicEvent::ArtistArtworkFetched {
                destination,
                generation,
                artist_id,
                revision,
                cache_key,
                available,
            } => self.handle_artist_artwork_fetched(
                destination,
                generation,
                &artist_id,
                revision,
                &cache_key,
                available,
            ),
        }
    }

    fn handle_series_event(&mut self, ev: SeriesEvent) {
        match ev {
            SeriesEvent::DetailFetched {
                series_id,
                seasons,
                episodes,
            } => self.handle_series_detail_fetched(
                &series_id,
                mbv_ui_model::browse::SeriesDetail { seasons, episodes },
            ),
            SeriesEvent::SeasonEpisodesFetched {
                series_id,
                season_id,
                episodes,
            } => self.handle_series_season_episodes_fetched(&series_id, season_id, episodes),
        }
    }

    fn handle_playlist_event(&mut self, ev: PlaylistEvent) {
        match ev {
            PlaylistEvent::ListLoaded(items) => self.handle_playlists_loaded(items),
            PlaylistEvent::ListLoadError(error) => self.handle_playlists_load_error(error),
            PlaylistEvent::ItemsLoaded { playlist_id, items } => {
                self.handle_playlist_items_loaded(&playlist_id, items);
            }
            PlaylistEvent::ItemsLoadError { playlist_id, error } => {
                self.handle_playlist_items_load_error(&playlist_id, error);
            }
            PlaylistEvent::Renamed { new_name } => {
                self.handle_playlist_renamed(&new_name);
            }
            PlaylistEvent::Deleted { name } => self.handle_playlist_deleted(&name),
        }
    }

    fn handle_all_items_prefetched(
        &mut self,
        lib_idx: usize,
        parent_id: &str,
        items: Vec<mbv_emby_model::EmbyItem>,
    ) {
        if let Some(last) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.nav_stack.last_mut())
            .filter(|last| last.parent_id == parent_id)
        {
            last.all_items = Some(items);
        }
        // The whole-library corpus is exactly what a pending Series
        // landing was waiting for (U2 correction).
        self.retry_pending_series_landing(lib_idx, parent_id);
    }

    fn handle_feed_home_video_aggregated(
        &mut self,
        lib_idx: usize,
        parent_id: &str,
        all_items: Vec<mbv_emby_model::EmbyItem>,
        groups: Vec<mbv_ui_model::feed::FeedHomeVideoGroup>,
    ) {
        if let Some(lib) = self.libs.get_mut(lib_idx)
            && lib
                .nav_stack
                .first()
                .is_some_and(|root| root.parent_id == parent_id)
        {
            let (selected_group, video_cursor, video_scroll) = lib
                .feed_home_video
                .as_ref()
                .map_or((0, 0, 0), feed_home_video_selection);
            lib.feed_home_video = Some(FeedHomeVideoState {
                all_items,
                groups,
                loading: false,
                selected_group,
                video_cursor,
                video_scroll,
            });
        }
        self.clamp_feed_home_video_state(lib_idx);
        self.log_feed_home_video_state(lib_idx, "aggregated");
    }

    fn cache_nonempty_album_artist(&mut self, album_id: &str, artist: &str) {
        if !artist.is_empty() {
            self.album_artist_cache
                .insert(album_id.to_string(), artist.to_string());
        }
    }

    fn handle_album_artist_level_fetched(
        &mut self,
        level_id: String,
        artists: Vec<(String, String)>,
    ) {
        let warmup_completed = self.level_artist_warmups_in_flight.remove(&level_id);
        let orphan_risk = matches!(
            self.album_artist_levels.get(&level_id),
            Some(LevelFillState::Loading { orphan_risk: true })
        );
        if artists.is_empty() {
            // HTTP failure (or a trackless level): no fill, the level's
            // albums resolve via the existing settle/fallback path.
            self.album_artist_levels
                .insert(level_id, LevelFillState::Failed);
        } else {
            for (album_id, artist) in artists {
                // An empty artist is never cached: an empty cache row
                // is terminal for readers, and the album must stay
                // free to settle via the fallback path instead. It
                // still advances candidates (as a known-unknown) so
                // one arrival resolves every waiting album at once.
                self.cache_nonempty_album_artist(&album_id, &artist);
                self.advance_music_grouping_candidates(&album_id, &artist);
            }
            self.album_artist_levels
                .insert(level_id, LevelFillState::Filled { orphan_risk });
        }
        if warmup_completed {
            self.drain_level_artist_warmups();
        }
    }

    fn handle_music_group_warmup_listed(
        &mut self,
        generation: mbv_core::service_runtime::SetupGeneration,
        groups: Vec<mbv_emby_model::EmbyItem>,
    ) {
        // One level fill per group-level child (design D5), deduped through
        // the same `LevelFillState::action_for` decision candidate creation
        // uses (`spawn_level_artist_fetch`'s guard). `albums` stays empty:
        // warm-up holds only the group listing, so orphan-`Path` attribution
        // (design D3) has no in-hand album paths. A successful warm-up is
        // marked with orphan risk and receives one path-aware upgrade when
        // that level is later browsed. A fill failure arrives as an empty
        // `AlbumArtistLevelFetched`, marking the level `Failed` (retryable)
        // with no UI error; browsing state is untouched.
        if !self.emby_runtime.accepts(generation) {
            return;
        }
        for group in groups {
            self.enqueue_level_artist_warmup(group.id);
        }
    }

    /// Caches a fetched album track list. A re-fetch replaces the list and
    /// restarts its eviction age; the cache's cap evicts the oldest entry
    /// (issue #917).
    pub(in crate::app) fn cache_album_tracks(&mut self, album_id: String, tracks: Vec<EmbyItem>) {
        self.album_tracks_cache.insert(album_id, tracks);
    }

    fn handle_album_tracks_fetched(
        &mut self,
        album_id: String,
        mut tracks: Vec<mbv_emby_model::EmbyItem>,
    ) {
        // A fallback artist fetch frees its bounded slot here, and the drain
        // arms the next in-scope album so rows keep appearing progressively;
        // selection-driven fetches share the cache but hold no slot.
        let fallback_completed = self.artist_album_track_fetches_in_flight.remove(&album_id);
        self.album_tracks_loading.remove(&album_id);
        // The cache is also the cursor's source of truth while the album is
        // open, so normalize it once before rendering or resolving the
        // focused track for playback.
        sort_audio_tracks(&mut tracks);
        self.cache_album_tracks(album_id, tracks);
        if fallback_completed {
            self.drain_artist_album_track_fetches();
        }
    }

    fn handle_playlists_loaded(&mut self, items: Vec<mbv_emby_model::EmbyItem>) {
        self.playlists = items;
        self.playlists_loading = false;
        self.playlists_cursor = self
            .playlists_cursor
            .min(self.playlists.len().saturating_sub(1));
    }

    fn handle_playlists_load_error(&mut self, error: String) {
        self.playlists_loading = false;
        self.flash_error(error);
    }

    fn handle_playlist_items_loaded(
        &mut self,
        playlist_id: &str,
        items: Vec<mbv_emby_model::EmbyItem>,
    ) {
        if self
            .playlists_open
            .as_ref()
            .is_some_and(|playlist| playlist.id == playlist_id)
        {
            self.playlists_open_items = items;
            self.playlists_open_loading = false;
        }
    }

    fn handle_playlist_items_load_error(&mut self, playlist_id: &str, error: String) {
        if self
            .playlists_open
            .as_ref()
            .is_some_and(|playlist| playlist.id == playlist_id)
        {
            self.playlists_open_loading = false;
        }
        self.flash_error(error);
    }

    fn handle_playlist_renamed(&mut self, new_name: &str) {
        self.dismiss_save_playlist();
        self.force_clear = true;
        self.flash(format!("Renamed to '{new_name}'"), ToastSeverity::Success);
    }

    fn handle_playlist_deleted(&mut self, name: &str) {
        self.dismiss_confirm();
        self.flash(format!("Deleted '{name}'"), ToastSeverity::Success);
    }

    fn handle_error(&mut self, error: &str) {
        self.pending_navigate_tab_switch = None;
        self.pending_series_landing = None;
        self.flash(format!("Library error: {error}"), ToastSeverity::Error);
    }

    /// The `RecursiveAlbumActivated` arm: install the landed nav stack and
    /// consume the deferred tab switch when it belongs to this navigation.
    fn handle_recursive_album_activated(
        &mut self,
        library_id: &str,
        nav_stack: Vec<mbv_ui_model::browse::BrowseLevel>,
    ) {
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
        if belongs_to_pending && let Some(idx) = self.pending_navigate_tab_switch.take() {
            self.set_library_tab(idx + 1);
        }
    }

    /// The `NavigateTo` arm: land the requested navigation target.
    fn handle_navigate_to_event(
        &mut self,
        lib_idx: usize,
        landing: NavigateLanding,
        switch_tab: bool,
    ) {
        match landing {
            NavigateLanding::Chain { mut nav_stack } => {
                for level in &mut nav_stack {
                    self.retain_grouped_music_level_items(lib_idx, level);
                }
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    lib.nav_stack = nav_stack;
                    // A completed navigation IS the saved position from now on;
                    // without this the `switch_tab` activation below compares the
                    // navigated stack against the stale saved position, takes the
                    // restore branch, and clobbers the navigation the user asked
                    // for (queue "Go to Library" / search-sidebar activation
                    // degraded to a bare tab switch).
                    self.save_default_library_position(lib_idx);
                }
                if switch_tab {
                    self.set_library_tab(lib_idx + 1);
                }
            }
            NavigateLanding::Series { reveal, episode_id } => {
                let name = reveal.name.clone();
                if self.libs.get(lib_idx).is_none() {
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
                } else if !self.arm_pending_series_landing(lib_idx, reveal, switch_tab, episode_id)
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
                    self.pending_track_selection = track_id.map(|id| (lib_idx, id));
                } else {
                    self.flash_error("Could not start the album navigation".to_string());
                }
            }
        }
    }

    /// The `SearchItemsLoaded` arm: the flat inline-search fetch re-homes
    /// the write the deleted direct flat-result projector used to do against
    /// the component: the completion lands in the nav level's `all_items`
    /// cache (the same guarded write as `AllItemsPrefetched`) and the shell's
    /// event-scoped projection (5.3d.20c) pushes it into the component. A
    /// completion racing a navigation -- `parent_id` no longer the last
    /// level's -- is stale and must not write.
    fn handle_search_items_loaded(
        &mut self,
        lib_idx: usize,
        parent_id: &str,
        items: Vec<mbv_emby_model::EmbyItem>,
    ) {
        if let Some(lib) = self.libs.get_mut(lib_idx)
            && let Some(last) = lib.nav_stack.last_mut()
            && last.parent_id == parent_id
        {
            last.all_items = Some(items);
        }
    }

    /// The `AlbumIndexBuilt` arm: arm a pending rebuild or install the built
    /// index (or its unavailability).
    fn handle_album_index_built(
        &mut self,
        library_id: String,
        result: Result<Vec<AlbumSearchEntry>, mbv_emby::EmbyError>,
    ) {
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
                    self.album_indexes.insert(
                        library_id.clone(),
                        AlbumIndexState::Ready(std::sync::Arc::new(AlbumIndex::new(entries))),
                    );
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
}
