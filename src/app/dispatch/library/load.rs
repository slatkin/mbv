use crate::app::state::playback::HomeContent;
use crate::app::{
    App, BrowseLevel, FeedHomeVideoState, LibEvent, TabSelection, dispatch::notify::ToastSeverity,
};
use crate::app::{ModelContentEvent, PlaylistEvent};
use mbv_emby_model::EmbyItem;
use std::collections::HashMap;

impl App {
    pub(in crate::app) fn refresh_lib(&mut self, lib_idx: usize) {
        // Defensive bounds check: the dispatch front door normalizes a stale
        // destination first, but async Service removal can invalidate the
        // matched index between normalization and this call. No-op (never
        // substitute library zero) on a miss.
        if lib_idx >= self.libs.len() {
            return;
        }
        self.start_album_index(lib_idx, true);
        if self.is_feed_home_video_group_view(lib_idx)
            && let Some(state) = self.libs[lib_idx].feed_home_video.as_mut()
        {
            state.loading = true;
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
            && (self.libs[lib_idx].tv_content_mode == Some(mbv_queue::TvContentMode::Latest)
                || self.libs[lib_idx]
                    .nav_stack
                    .last()
                    .and_then(|level| level.tv_content_mode.as_ref())
                    == Some(&mbv_queue::TvContentMode::Latest))
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
            let key = mbv_ui_model::browse::LevelFetchKey::from_level(lvl);
            let loaded_count = lvl.items.len();
            self.spawn_refresh(lib_idx, loaded_count, key);
        }
    }

    /// Refresh the selected browse destination (Home, an Emby library, an
    /// Audiobookshelf library, or Feeds) regardless of Panel focus or whether
    /// the Library panel is painted. A stale selected library index normalizes
    /// to Home and stops without a fetch. This is a data refresh only: it never
    /// resets presentation state, writes preferences, or clears a saved
    /// library position (#745).
    pub(in crate::app) fn refresh_current_view(&mut self) {
        self.force_clear = true;
        if self.normalize_stale_browse_destination() {
            return;
        }
        match self.tab {
            TabSelection::Home => {
                match self.fetch_home() {
                    Ok(content) => {
                        // The fetch runs synchronously (its App-side
                        // side effects are order-sensitive); the
                        // computed content travels to Model-owned
                        // `home_content` via lib_tx (task 5.3d).
                        let _ = self.channels.lib_tx.send(LibEvent::ModelContent(
                            ModelContentEvent::HomeContentRefreshed(Box::new(content)),
                        ));
                    }
                    Err(e) => {
                        self.flash(format!("Refresh error: {e}"), ToastSeverity::Error);
                    }
                }
            }
            TabSelection::EmbyLibrary(lib_idx) => self.refresh_lib(lib_idx),
            TabSelection::AudiobookshelfLibrary(index) => {
                match self.audiobookshelf_kind_at(index) {
                    Some(mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseKind::Book) => {
                        self.audiobookshelf_book_refresh();
                    }
                    _ => self.audiobookshelf_refresh(),
                }
            }
            TabSelection::Feeds => self.refresh_feeds(),
        }
    }

    pub(in crate::app) fn spawn_load_playlists(&mut self) {
        if self.playlists_loading {
            return;
        }
        self.playlists_loading = true;
        let Some(client) = self.emby_snapshot() else {
            self.playlists_loading = false;
            return;
        };
        let tx = self.channels.lib_tx.clone();
        std::thread::spawn(move || match client.get_playlists() {
            Ok(items) => {
                let _ = tx.send(LibEvent::Playlist(PlaylistEvent::ListLoaded(items)));
            }
            Err(e) => {
                let _ = tx.send(LibEvent::Playlist(PlaylistEvent::ListLoadError(format!(
                    "Playlist list failed: {e}"
                ))));
            }
        });
    }

    pub(in crate::app) fn spawn_rename_playlist(&mut self, playlist_id: String, new_name: String) {
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.channels.lib_tx.clone();
        std::thread::spawn(move || {
            if let Err(e) = client.rename_playlist(&playlist_id, &new_name) {
                let _ = tx.send(LibEvent::Error(format!("Rename failed: {e}")));
            } else {
                let _ = tx.send(LibEvent::Playlist(PlaylistEvent::Renamed { new_name }));
            }
            match client.get_playlists() {
                Ok(items) => {
                    let _ = tx.send(LibEvent::Playlist(PlaylistEvent::ListLoaded(items)));
                }
                Err(e) => {
                    let _ = tx.send(LibEvent::Error(e.to_string()));
                }
            }
        });
    }

    pub(in crate::app) fn spawn_delete_playlist(&mut self, playlist_id: String, name: String) {
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.channels.lib_tx.clone();
        std::thread::spawn(move || {
            if let Err(e) = client.delete_playlist(&playlist_id) {
                let _ = tx.send(LibEvent::Error(format!("Delete failed: {e}")));
            } else {
                let _ = tx.send(LibEvent::Playlist(PlaylistEvent::Deleted { name }));
            }
            match client.get_playlists() {
                Ok(items) => {
                    let _ = tx.send(LibEvent::Playlist(PlaylistEvent::ListLoaded(items)));
                }
                Err(e) => {
                    let _ = tx.send(LibEvent::Error(e.to_string()));
                }
            }
        });
    }

    pub(in crate::app) fn spawn_open_playlist(&mut self, playlist: &EmbyItem) {
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
        let tx = self.channels.lib_tx.clone();
        let playlist_id = playlist.id.clone();
        std::thread::spawn(move || match client.get_playlist_items(&playlist_id) {
            Ok(items) => {
                let _ = tx.send(LibEvent::Playlist(PlaylistEvent::ItemsLoaded {
                    playlist_id,
                    items,
                }));
            }
            Err(e) => {
                let _ = tx.send(LibEvent::Playlist(PlaylistEvent::ItemsLoadError {
                    playlist_id,
                    error: format!("Playlist load failed: {e}"),
                }));
            }
        });
    }

    /// Fetches a playlist's items synchronously and returns the playable
    /// (non-folder) ones. Flashes and returns `None` when Emby is unavailable,
    /// the fetch fails, the playlist is empty, or nothing in it is playable.
    pub(in crate::app) fn playlist_playable_items(
        &mut self,
        playlist_id: &str,
    ) -> Option<Vec<EmbyItem>> {
        let Some(client) = self.emby_snapshot() else {
            self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
            return None;
        };
        let items = match client.get_playlist_items(playlist_id) {
            Ok(r) => r,
            Err(e) => {
                self.flash(format!("Playlist load failed: {e}"), ToastSeverity::Error);
                return None;
            }
        };
        if items.is_empty() {
            self.flash("Playlist is empty".into(), ToastSeverity::Error);
            return None;
        }
        let playable: Vec<EmbyItem> = items.into_iter().filter(|i| !i.is_folder).collect();
        if playable.is_empty() {
            self.flash("No playable items in playlist".into(), ToastSeverity::Error);
            return None;
        }
        Some(playable)
    }

    pub(in crate::app) fn rebuild_library_tabs_from_views(&mut self, all_views: &[EmbyItem]) {
        // Drain existing libs, preserving nav stacks and scroll pos so that a
        // UserDataChanged websocket refresh (fired when playback starts)
        // doesn't silently reset list scroll position.
        struct SavedLibState {
            nav_stack: Vec<BrowseLevel>,
            feed_home_video: Option<FeedHomeVideoState>,
            library_total: Option<usize>,
            tv_content_mode: Option<mbv_queue::TvContentMode>,
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
                            rows: lvl.rows,
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
                mbv_ui_model::sort_filter::resolve_tv_content_mode(
                    library_total.unwrap_or_default(),
                    saved.and_then(|state| state.tv_content_mode.as_ref()),
                )
            });
            self.libs.push(crate::app::LibraryTab {
                nav_stack: stack,
                feed_home_video,
                library_total,
                tv_content_mode,
                ..crate::app::LibraryTab::new(view.clone())
            });
        }

        self.resolve_launch_service_tab(mbv_queue::ServiceKind::Emby);
    }

    /// Compute the Home Continue Watching snapshot. Continue Watching and
    /// the library catalog are fetched only when Emby is configured and connected.
    /// Without one it is skipped -- not an error -- so Home still populates
    /// from whatever local Sources exist (#543 Part 1). Returns the
    /// computed `HomeContent` instead of writing deleted `App.home`; the
    /// shell assigns it to `Model.home_content` (directly for shell-side
    /// callers, via `LibEvent::ModelContent(ModelContentEvent::HomeContentRefreshed)` for App-internal ones)
    /// and preserves the Continue Watching column cursor at the assignment.
    pub(in crate::app) fn fetch_home(&mut self) -> Result<HomeContent, mbv_emby::EmbyFailure> {
        let mut emby_fetched = false;
        let (continue_items, all_views) = match self.emby_client() {
            Some(client) => {
                emby_fetched = true;
                let client = client.lock().unwrap();
                let views = match client.get_views_classified() {
                    Ok(views) => views,
                    Err(error) => {
                        drop(client);
                        self.handle_emby_runtime_failure(error.clone());
                        return Err(error);
                    }
                };
                (client.get_continue_watching(20).unwrap_or_default(), views)
            }
            _ => (Vec::new(), Vec::new()),
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

    /// The `Newest Episodes` shelf's catalog entries, or an empty list when
    /// the shelf is absent (only that shelf feeds Home).
    pub(in crate::app) fn newest_episodes_items(
        shelves: Vec<mbv_audiobookshelf::AudiobookshelfShelf>,
    ) -> Vec<mbv_queue::AudiobookshelfEpisodeCatalog> {
        shelves
            .into_iter()
            .find(|shelf| shelf.label.eq_ignore_ascii_case("Newest episodes"))
            .map(|shelf| {
                shelf
                    .entries
                    .into_iter()
                    .filter_map(|entry| match entry {
                        mbv_audiobookshelf::AudiobookshelfShelfEntry::Episode(item) => Some(item),
                        mbv_audiobookshelf::AudiobookshelfShelfEntry::Show(_) => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}
