use super::types_playback::HomeContent;
use super::{
    notify_actions::ToastSeverity, App, BrowseLevel, FeedHomeVideoState, LibEvent, PanelFocus,
    PendingQueueAction, ReplacementExecutor, TabSelection,
};
use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::QueueItem;
use std::collections::HashMap;

impl App {
    pub(super) fn refresh_lib(&mut self, lib_idx: usize) {
        // Defensive bounds check: the dispatch front door normalizes a stale
        // destination first, but async Service removal can invalidate the
        // matched index between normalization and this call. No-op (never
        // substitute library zero) on a miss. Callers own the panel-focus
        // gate that the pre-parameterization body enforced here.
        if lib_idx >= self.libs.len() {
            return;
        }
        self.start_album_index(lib_idx, true);
        self.clear_saved_library_position(lib_idx);
        if self.is_feed_home_video_group_view(lib_idx) {
            if let Some(state) = self.libs[lib_idx].feed_home_video.as_mut() {
                state.loading = true;
            }
        }
        self.log_feed_home_video_state(lib_idx, "refresh_lib_before_spawn");
        if self.libs[lib_idx].library.collection_type != "tvshows"
            && self.libs[lib_idx].library.collection_type != "playlists"
        {
            self.spawn_emby_latest_snapshot(
                self.libs[lib_idx].library.id.clone(),
                self.libs[lib_idx].library.name.clone(),
            );
        }
        if self.libs[lib_idx].library.collection_type == "tvshows"
            && self.libs[lib_idx].nav_stack.len() == 1
            && (self.libs[lib_idx].tv_content_mode == Some(mbv_core::config::TvContentMode::Latest)
                || self.libs[lib_idx]
                    .nav_stack
                    .last()
                    .and_then(|level| level.tv_content_mode.as_ref())
                    == Some(&mbv_core::config::TvContentMode::Latest))
        {
            let parent_id = self.libs[lib_idx]
                .nav_stack
                .last()
                .map(|level| level.parent_id.clone());
            if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
                level.loading = true;
            }
            if let Some(parent_id) = parent_id {
                let title = self.libs[lib_idx].library.name.clone();
                self.spawn_tv_latest(lib_idx, parent_id, title);
            }
            return;
        }
        if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut() {
            lvl.loading = true;
            let parent_id = lvl.parent_id.clone();
            let item_types = lvl.item_types.clone();
            let unplayed_only = lvl.unplayed_only;
            let sort_by = lvl.sort_by.clone();
            let sort_order = lvl.sort_order.clone();
            let loaded_count = lvl.items.len();
            let letter_filter = lvl.letter_filter.clone();
            self.spawn_refresh(
                lib_idx,
                parent_id,
                item_types,
                unplayed_only,
                sort_by,
                sort_order,
                loaded_count,
                letter_filter,
            );
        }
    }

    fn refresh_queue(&mut self) {
        let scope = self.viewed_queue_scope();
        if self.queue_for_scope(scope).total_queue_len() == 0 {
            return;
        }
        let ids: Vec<String> = self
            .queue_for_scope(scope)
            .queue
            .slots()
            .iter()
            .filter_map(|s| s.item.as_emby())
            .map(|i| i.id.clone())
            .collect();
        let Some(client) = self.emby_client() else {
            return;
        };
        let client = client.lock().unwrap();
        if let Ok(fetched) = client.get_items_by_ids(&ids) {
            drop(client);
            let _ = self.merge_refreshed_queue(scope, fetched);
        }
    }

    pub(super) fn refresh_current_view(&mut self) {
        self.force_clear = true;
        match self.effective_panel_focus() {
            // Queue refresh is a refresh of the visible queue only and never
            // indexes the selected browse destination.
            PanelFocus::Queue => self.refresh_queue(),
            PanelFocus::Library => {
                if self.normalize_stale_browse_destination() {
                    return;
                }
                // Refreshing the active library view reverts the Wide hero
                // split to the shared arrangement's default ratio. This reset
                // is deliberately inside the library arm, not at the function
                // top: refreshing while the Queue panel holds focus must leave
                // the split untouched (design.md "Refresh reset lives in the
                // library-side refresh arm").
                self.list_pane_width = None;
                self.save_prefs();
                match self.tab {
                    TabSelection::Home => {
                        match self.fetch_home() {
                            Ok(content) => {
                                // The fetch runs synchronously (its App-side
                                // side effects are order-sensitive); the
                                // computed content travels to Model-owned
                                // `home_content` via lib_tx (task 5.3d).
                                let _ = self
                                    .lib_tx
                                    .send(LibEvent::HomeContentRefreshed(Box::new(content)));
                            }
                            Err(e) => {
                                self.flash(format!("Refresh error: {e}"), ToastSeverity::Error)
                            }
                        }
                    }
                    TabSelection::EmbyLibrary(lib_idx) => self.refresh_lib(lib_idx),
                    TabSelection::AudiobookshelfLibrary(index) => {
                        match self.audiobookshelf_kind_at(index) {
                            Some(
                                super::types_audiobookshelf_browse::AudiobookshelfBrowseKind::Book,
                            ) => self.audiobookshelf_book_refresh(),
                            _ => self.audiobookshelf_refresh(),
                        }
                    }
                    TabSelection::Feeds => self.refresh_feeds(),
                }
            }
        }
    }

    pub(super) fn spawn_load_playlists(&mut self) {
        if self.playlists_loading {
            return;
        }
        self.playlists_loading = true;
        let Some(client) = self.emby_snapshot() else {
            self.playlists_loading = false;
            return;
        };
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || match client.get_playlists() {
            Ok(items) => {
                let _ = tx.send(LibEvent::PlaylistsLoaded(items));
            }
            Err(e) => {
                let _ = tx.send(LibEvent::PlaylistsLoadError(format!(
                    "Playlist list failed: {e}"
                )));
            }
        });
    }

    pub(super) fn spawn_rename_playlist(&mut self, playlist_id: String, new_name: String) {
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            if let Err(e) = client.rename_playlist(&playlist_id, &new_name) {
                let _ = tx.send(LibEvent::Error(format!("Rename failed: {e}")));
            } else {
                let _ = tx.send(LibEvent::PlaylistRenamed { new_name });
            }
            match client.get_playlists() {
                Ok(items) => {
                    let _ = tx.send(LibEvent::PlaylistsLoaded(items));
                }
                Err(e) => {
                    let _ = tx.send(LibEvent::Error(e));
                }
            }
        });
    }

    pub(super) fn spawn_delete_playlist(&mut self, playlist_id: String, name: String) {
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            if let Err(e) = client.delete_playlist(&playlist_id) {
                let _ = tx.send(LibEvent::Error(format!("Delete failed: {e}")));
            } else {
                let _ = tx.send(LibEvent::PlaylistDeleted { name });
            }
            match client.get_playlists() {
                Ok(items) => {
                    let _ = tx.send(LibEvent::PlaylistsLoaded(items));
                }
                Err(e) => {
                    let _ = tx.send(LibEvent::Error(e));
                }
            }
        });
    }

    pub(super) fn spawn_open_playlist(&mut self, playlist: EmbyItem) {
        if self.playlists_open_loading {
            return;
        }
        self.playlists_open_loading = true;
        self.playlists_open = Some(playlist.clone());
        self.playlists_open_items = Vec::new();
        self.playlists_open_cursor = 0;
        self.playlists_open_scroll = 0;
        let Some(client) = self.emby_snapshot() else {
            self.playlists_open_loading = false;
            return;
        };
        let tx = self.lib_tx.clone();
        let playlist_id = playlist.id.clone();
        std::thread::spawn(move || match client.get_playlist_items(&playlist_id) {
            Ok(items) => {
                let _ = tx.send(LibEvent::PlaylistItemsLoaded { playlist_id, items });
            }
            Err(e) => {
                let _ = tx.send(LibEvent::PlaylistItemsLoadError {
                    playlist_id,
                    error: format!("Playlist load failed: {e}"),
                });
            }
        });
    }

    pub(super) fn open_playlists_panel(&mut self) {
        self.request_sidebar_dismiss(super::SidebarId::Sessions);
        self.close_settings();
        self.request_sidebar_open(super::SidebarId::Playlists);
        if self.playlists.is_empty() && !self.playlists_loading {
            self.spawn_load_playlists();
        }
    }

    pub(super) fn load_and_play_playlist(&mut self, playlist_id: String) {
        let playlist_name = self
            .playlists
            .iter()
            .find(|p| p.id == playlist_id)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        let Some(client) = self.emby_snapshot() else {
            self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
            return;
        };
        let items = match client.get_playlist_items(&playlist_id) {
            Ok(r) => r,
            Err(e) => {
                self.flash(format!("Playlist load failed: {e}"), ToastSeverity::Error);
                return;
            }
        };
        if items.is_empty() {
            self.flash("Playlist is empty".into(), ToastSeverity::Error);
            return;
        }
        let playable: Vec<EmbyItem> = items.into_iter().filter(|i| !i.is_folder).collect();
        if playable.is_empty() {
            self.flash("No playable items in playlist".into(), ToastSeverity::Error);
            return;
        }
        let action = PendingQueueAction::PlayItems {
            items: playable,
            start_idx: 0,
            source: crate::config::QueueSource::Playlist {
                id: Some(playlist_id),
                name: playlist_name,
            },
            autostart: false,
        };
        // The populated-queue gate defers the replacement; `run_replacement`
        // raises the Playlists sidebar dismiss (and Queue focus) once it runs
        // — immediately on an empty queue, after confirmation on a populated
        // one — so a cancelled load leaves the sidebar open.
        self.request_queue_replacement(action, ReplacementExecutor::Pending);
    }

    pub(super) fn rebuild_library_tabs_from_views(&mut self, all_views: &[EmbyItem]) {
        // Drain existing libs, preserving nav stacks and scroll pos so that a
        // UserDataChanged websocket refresh (fired when playback starts)
        // doesn't silently reset list scroll position.
        struct SavedLibState {
            nav_stack: Vec<BrowseLevel>,
            feed_home_video: Option<FeedHomeVideoState>,
            library_total: Option<usize>,
            tv_content_mode: Option<mbv_core::config::TvContentMode>,
        }
        let old_libs: HashMap<String, SavedLibState> = self
            .libs
            .drain(..)
            .map(|mut l| {
                (
                    l.library.id.clone(),
                    SavedLibState {
                        nav_stack: std::mem::take(&mut l.nav_stack),
                        feed_home_video: l.feed_home_video,
                        library_total: l.library_total,
                        tv_content_mode: l.tv_content_mode,
                    },
                )
            })
            .collect();

        for view in all_views.iter().filter(|v| {
            v.collection_type != "playlists"
                && !self.hidden_libraries.contains(&v.name.to_lowercase())
        }) {
            let saved = old_libs.get(&view.id);
            let stack = saved
                .map(|s| {
                    s.nav_stack
                        .iter()
                        .map(|lvl| BrowseLevel {
                            parent_id: lvl.parent_id.clone(),
                            title: lvl.title.clone(),
                            items: lvl.items.clone(),
                            fetched_rows: lvl.fetched_rows,
                            total_count: lvl.total_count,
                            item_types: lvl.item_types.clone(),
                            unplayed_only: lvl.unplayed_only,
                            sort_by: lvl.sort_by.clone(),
                            sort_order: lvl.sort_order.clone(),
                            loading: false,
                            resting: lvl.resting(),
                            all_items: lvl.all_items.clone(),
                            letter_filter: lvl.letter_filter.clone(),
                            tv_content_mode: lvl.tv_content_mode.clone(),
                            music_grouping: lvl.music_grouping.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            let feed_home_video = saved.and_then(|s| s.feed_home_video.clone());
            let library_total = saved.and_then(|s| s.library_total);
            let tv_content_mode = (view.collection_type == "tvshows").then(|| {
                super::render::resolve_tv_content_mode(
                    library_total.unwrap_or_default(),
                    saved.and_then(|state| state.tv_content_mode.as_ref()),
                )
            });
            self.libs.push(super::LibraryTab {
                nav_stack: stack,
                feed_home_video,
                library_total,
                tv_content_mode,
                ..super::LibraryTab::new(view.clone())
            });
        }

        // Rebuilding the tabs from live views IS the live Emby catalog
        // boundary. A plain local launch reaches it through the Emby startup
        // worker's bootstrap, but a local-daemon/remote attach has a live
        // client at construction and never runs that worker: its catalog
        // arrives here, through `fetch_home`. Without this the launch tab
        // could never resolve stable Emby identities on that path.
        self.emby_catalog_ready = true;
    }

    /// Compute the Home Continue Watching snapshot. Continue Watching and
    /// the library catalog are fetched only when Emby is configured and connected.
    /// Without one it is skipped -- not an error -- so Home still populates
    /// from whatever local Sources exist (#543 Part 1). Returns the
    /// computed `HomeContent` instead of writing deleted `App.home`; the
    /// shell assigns it to `Model.home_content` (directly for shell-side
    /// callers, via `LibEvent::HomeContentRefreshed` for App-internal ones)
    /// and preserves the Continue Watching column cursor at the assignment.
    pub(super) fn fetch_home(&mut self) -> Result<HomeContent, String> {
        let mut emby_fetched = false;
        let (continue_items, all_views) = if let Some(client) = self.emby_client() {
            emby_fetched = true;
            let client = client.lock().unwrap();
            let views = match client.get_views_classified() {
                Ok(views) => views,
                Err(error) => {
                    drop(client);
                    self.handle_emby_runtime_failure(error.clone());
                    return Err(error.to_string());
                }
            };
            (client.get_continue_watching(20).unwrap_or_default(), views)
        } else {
            (Vec::new(), Vec::new())
        };

        // Library tabs are Emby-modeled state; only rebuild them from the
        // freshly fetched views when Emby was actually reachable, so a broken
        // or absent Emby does not clear existing library tabs.
        if emby_fetched {
            self.rebuild_library_tabs_from_views(&all_views);
            for lib_idx in 0..self.libs.len() {
                self.start_album_index(lib_idx, false);
            }
        }

        Ok(HomeContent {
            continue_items,
            loading: false,
        })
    }

    /// The `Newest Episodes` shelf's entries as queue-able items, or an empty
    /// list when the shelf is absent (only that shelf feeds Home).
    pub(super) fn newest_episodes_items(
        shelves: Vec<mbv_core::audiobookshelf::AudiobookshelfShelf>,
    ) -> Vec<QueueItem> {
        shelves
            .into_iter()
            .find(|shelf| shelf.label.eq_ignore_ascii_case("Newest episodes"))
            .map(|shelf| {
                shelf
                    .entries
                    .into_iter()
                    .filter_map(|entry| match entry {
                        mbv_core::audiobookshelf::AudiobookshelfShelfEntry::Episode(item) => {
                            Some(QueueItem::Audiobookshelf(item))
                        }
                        mbv_core::audiobookshelf::AudiobookshelfShelfEntry::Show(_) => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}
