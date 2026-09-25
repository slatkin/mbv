use crate::app::infra::ui_util::sort_audio_tracks;
use crate::app::state::app_struct::LevelFillState;
use crate::app::state::types::events::{NavigateLanding, PendingSeriesHandoff};
use crate::app::{
    dispatch::notify::ToastSeverity, AlbumIndex, AlbumIndexState, AlbumSearchEntry, App,
    FeedHomeVideoState, LibEvent, QueueScope,
};

mod audiobookshelf;
mod browse_loads;

impl App {
    pub(in crate::app) fn handle_lib_event(&mut self, ev: LibEvent) {
        let Some(ev) = self.handle_audiobookshelf_event(ev) else {
            return;
        };
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
            } => self.handle_search_items_loaded(lib_idx, parent_id, items),
            LibEvent::AlbumIndexBuilt { library_id, result } => {
                self.handle_album_index_built(library_id, result)
            }
            LibEvent::RecursiveAlbumActivated {
                library_id,
                nav_stack,
            } => self.handle_recursive_album_activated(library_id, nav_stack),
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
                // A fallback artist fetch frees its bounded slot here, and the
                // drain arms the next in-scope album so rows keep appearing
                // progressively; selection-driven fetches share the cache but
                // hold no slot.
                let fallback_completed =
                    self.artist_album_track_fetches_in_flight.remove(&album_id);
                self.album_tracks_loading.remove(&album_id);
                // The cache is also the cursor's source of truth while the
                // album is open, so normalize it once before rendering or
                // resolving the focused track for playback.
                sort_audio_tracks(&mut tracks);
                self.album_tracks_cache.insert(album_id, tracks);
                if fallback_completed {
                    self.drain_artist_album_track_fetches();
                }
            }
            LibEvent::ArtistTracksFetched {
                destination,
                generation,
                artist_id,
                revision,
                result,
            } => self.handle_artist_tracks_fetched(
                destination,
                generation,
                artist_id,
                revision,
                result,
            ),
            LibEvent::ArtistArtworkFetched {
                destination,
                generation,
                artist_id,
                revision,
                cache_key,
                available,
            } => self.handle_artist_artwork_fetched(
                destination,
                generation,
                artist_id,
                revision,
                cache_key,
                available,
            ),
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
            LibEvent::AlbumArtistLevelFetched { level_id, artists } => {
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
                        if !artist.is_empty() {
                            self.album_artist_cache
                                .insert(album_id.clone(), artist.clone());
                        }
                        self.advance_music_grouping_candidates(&album_id, &artist);
                    }
                    self.album_artist_levels
                        .insert(level_id, LevelFillState::Filled { orphan_risk });
                }
                if warmup_completed {
                    self.drain_level_artist_warmups();
                }
            }
            LibEvent::MusicGroupWarmupListed { generation, groups } => {
                if !self.emby_runtime.accepts(generation) {
                    return;
                }
                // One level fill per group-level child (design D5), deduped
                // through the same `LevelFillState::action_for` decision
                // candidate creation uses (`spawn_level_artist_fetch`'s
                // guard). `albums` stays empty: warm-up holds only the group
                // listing, so orphan-`Path` attribution (design D3) has no
                // in-hand album paths. A successful warm-up is marked with
                // orphan risk and receives one path-aware upgrade when that
                // level is later browsed. A fill failure arrives as an empty
                // `AlbumArtistLevelFetched`, marking the level `Failed`
                // (retryable) with no UI error; browsing state is untouched.
                for group in groups {
                    self.enqueue_level_artist_warmup(group.id);
                }
            }
            LibEvent::NavigateTo {
                lib_idx,
                landing,
                switch_tab,
            } => self.handle_navigate_to_event(lib_idx, landing, switch_tab),
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
            // Shell-intercepted content-delivery variants: the lib_rx drain
            // handles them at the Model boundary, so they are unreachable
            // here; the arms keep the exhaustive match total.
            LibEvent::EmbyLatestSnapshotFetched { .. }
            | LibEvent::HomeContentRefreshed(_)
            | LibEvent::HomeContentCleared => {}
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

    /// The `RecursiveAlbumActivated` arm: install the landed nav stack and
    /// consume the deferred tab switch when it belongs to this navigation.
    fn handle_recursive_album_activated(
        &mut self,
        library_id: String,
        nav_stack: Vec<crate::app::state::types::browse::BrowseLevel>,
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
        if belongs_to_pending {
            if let Some(idx) = self.pending_navigate_tab_switch.take() {
                self.set_library_tab(idx + 1);
            }
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
        parent_id: String,
        items: Vec<mbv_core::api::EmbyItem>,
    ) {
        if let Some(lib) = self.libs.get_mut(lib_idx) {
            if let Some(last) = lib.nav_stack.last_mut() {
                if last.parent_id == parent_id {
                    last.all_items = Some(items);
                }
            }
        }
    }

    /// The `AlbumIndexBuilt` arm: arm a pending rebuild or install the built
    /// index (or its unavailability).
    fn handle_album_index_built(
        &mut self,
        library_id: String,
        result: Result<Vec<AlbumSearchEntry>, String>,
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
