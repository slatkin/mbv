use super::{
    AlbumIndexState, App, BrowseLevel, FeedHomeVideoState, LibEvent, PanelFocus, PendingQueueAction,
};
use mbv_core::api::MediaItem;
use std::collections::HashMap;

impl App {
    pub(super) fn refresh_lib(&mut self) {
        let lib_idx = if matches!(self.panel_focus, PanelFocus::Library) && self.library_tab > 0 {
            self.library_tab - 1
        } else {
            return;
        };
        self.start_album_index(lib_idx, true);
        self.clear_saved_library_position(lib_idx);
        if self.is_feed_home_video_group_view(lib_idx) {
            if let Some(state) = self.libs[lib_idx].feed_home_video.as_mut() {
                state.loading = true;
            }
        }
        self.log_feed_home_video_state(lib_idx, "refresh_lib_before_spawn");
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
        let scope = self.visible_queue_scope();
        if self.queue_for_scope(scope).items.is_empty() {
            return;
        }
        let ids: Vec<String> = self
            .queue_for_scope(scope)
            .items
            .iter()
            .map(|i| i.id.clone())
            .collect();
        let client = self.client.lock().unwrap();
        if let Ok(fetched) = client.get_items_by_ids(&ids) {
            drop(client);
            let _ = self.merge_refreshed_queue(scope, fetched);
        }
    }

    pub(super) fn refresh_current_view(&mut self) {
        self.force_clear = true;
        if matches!(self.panel_focus, PanelFocus::Queue) {
            self.refresh_queue();
        } else if self.library_tab == 0 {
            if let Err(e) = self.fetch_home() {
                self.flash_status_high(format!("Refresh error: {e}"));
            }
        } else {
            self.refresh_lib();
        }
    }

    pub(super) fn spawn_load_playlists(&mut self) {
        if self.playlists_loading {
            return;
        }
        self.playlists_loading = true;
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let items = client.get_playlists().unwrap_or_default();
            let _ = tx.send(LibEvent::PlaylistsLoaded(items));
        });
    }

    pub(super) fn spawn_rename_playlist(&mut self, playlist_id: String, new_name: String) {
        let client = self.client.lock().unwrap().clone();
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
        let client = self.client.lock().unwrap().clone();
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

    pub(super) fn spawn_open_playlist(&mut self, playlist: MediaItem) {
        if self.playlists_open_loading {
            return;
        }
        self.playlists_open_loading = true;
        self.playlists_open = Some(playlist.clone());
        self.playlists_open_items = Vec::new();
        self.playlists_open_cursor = 0;
        self.playlists_open_scroll = 0;
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        let playlist_id = playlist.id.clone();
        std::thread::spawn(move || {
            let items = client.get_playlist_items(&playlist_id).unwrap_or_default();
            let _ = tx.send(LibEvent::PlaylistItemsLoaded { playlist_id, items });
        });
    }

    pub(super) fn open_playlists_panel(&mut self) {
        self.show_help = false;
        self.show_sessions = false;
        self.close_settings();
        self.show_playlists = true;
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
        let client = self.client.lock().unwrap().clone();
        let items = match client.get_playlist_items(&playlist_id) {
            Ok(r) => r,
            Err(e) => {
                self.flash_status_high(format!("Playlist load failed: {e}"));
                return;
            }
        };
        if items.is_empty() {
            self.flash_status_high("Playlist is empty".into());
            return;
        }
        let playable: Vec<MediaItem> = items.into_iter().filter(|i| !i.is_folder).collect();
        if playable.is_empty() {
            self.flash_status_high("No playable items in playlist".into());
            return;
        }
        let action = PendingQueueAction::PlayItems {
            items: playable,
            start_idx: 0,
            source: crate::config::QueueSource::Playlist {
                id: Some(playlist_id),
                name: playlist_name,
            },
        };
        self.replace_queue_or_prompt(action);
        if self.confirm_modal.is_none() {
            self.show_playlists = false;
            self.set_panel_focus(PanelFocus::Queue);
        }
    }

    pub(super) fn rebuild_library_tabs_from_views(&mut self, all_views: &[MediaItem]) {
        // Drain existing libs, preserving nav stacks and scroll pos so that a
        // UserDataChanged websocket refresh (fired when playback starts)
        // doesn't silently reset list scroll position.
        struct SavedLibState {
            nav_stack: Vec<BrowseLevel>,
            feed_home_video: Option<FeedHomeVideoState>,
            library_total: Option<usize>,
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
                            total_count: lvl.total_count,
                            cursor: lvl.cursor,
                            item_types: lvl.item_types.clone(),
                            unplayed_only: lvl.unplayed_only,
                            sort_by: lvl.sort_by.clone(),
                            sort_order: lvl.sort_order.clone(),
                            loading: false,
                            scroll: lvl.scroll,
                            all_items: lvl.all_items.clone(),
                            letter_filter: lvl.letter_filter.clone(),
                            music_grouping: lvl.music_grouping.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            let feed_home_video = saved.and_then(|s| s.feed_home_video.clone());
            let library_total = saved.and_then(|s| s.library_total);
            self.libs.push(super::LibraryTab {
                library: view.clone(),
                search: None,
                nav_stack: stack,
                feed_home_video,

                album_track_focus: None,
                artist_header_focus: None,
                series_selection: None,
                series_season_cursor: 0,
                library_total,
            });
        }
    }

    pub(super) fn fetch_home(&mut self) -> Result<(), String> {
        let (continue_items, all_views, user_views) = {
            let client = self.client.lock().unwrap();
            (
                client.get_continue_watching(20).unwrap_or_default(),
                client.get_views()?,
                client.get_user_views().unwrap_or_default(),
            )
        };

        self.home.continue_items = continue_items;
        self.rebuild_library_tabs_from_views(&all_views);
        for lib_idx in 0..self.libs.len() {
            self.start_album_index(lib_idx, false);
        }

        let old_cursors: HashMap<String, usize> = self
            .home
            .latest
            .iter()
            .map(|(_, lib_id, _, cur)| (lib_id.clone(), *cur))
            .collect();

        let mut latest: Vec<(String, String, Vec<MediaItem>, usize)> = Vec::new();
        let client = self.client.lock().unwrap();
        for v in user_views.iter().filter(|v| {
            let lower = v.name.to_lowercase();
            v.collection_type != "playlists"
                && !self.hidden_latest.contains(&lower)
                && !self.hidden_libraries.contains(&lower)
        }) {
            let title = v.name.clone();
            let items = if v.collection_type == "tvshows" {
                client.get_latest_episodes(&v.id, 30).unwrap_or_default()
            } else {
                client.get_latest(&v.id, 30).unwrap_or_default()
            };
            let cursor = old_cursors
                .get(&v.id)
                .copied()
                .unwrap_or(0)
                .min(items.len().saturating_sub(1));
            latest.push((title, v.id.clone(), items, cursor));
        }
        drop(client);
        self.home.latest = latest;

        let n = 1 + self.home.latest.len();
        if self.home.section >= n {
            self.home.section = n.saturating_sub(1);
        }
        Ok(())
    }

    pub(super) fn settings_scroll_follow(&mut self) {
        let cursor = self.settings_cursor;
        let Some(&cursor_line) = self.layout.settings_line_of_cursor.get(cursor) else {
            return;
        };
        let visible = self.layout.settings_content_area.height.max(1) as usize;
        if cursor_line < self.settings_scroll {
            self.settings_scroll = cursor_line;
        } else if cursor_line >= self.settings_scroll + visible {
            self.settings_scroll = cursor_line + 1 - visible;
        }
    }

    pub(super) fn update_lib_search(&mut self, lib_idx: usize) {
        use fuzzy_matcher::skim::SkimMatcherV2;
        use fuzzy_matcher::FuzzyMatcher;

        let query = match self.libs[lib_idx].search.as_ref() {
            Some(s) => s.query.clone(),
            None => return,
        };

        if query.is_empty() {
            if let Some(s) = self.libs[lib_idx].search.as_mut() {
                let n = s.items.len();
                s.results = (0..n).collect();
                s.cursor = 0;
            }
            return;
        }

        let recursive_entries = self
            .libs
            .get(lib_idx)
            .and_then(|lib| self.album_indexes.get(&lib.library.id))
            .and_then(|state| match state {
                AlbumIndexState::Ready(entries) => Some(entries),
                _ => None,
            });
        let scored: Vec<(i64, usize)> = {
            let items = self.libs[lib_idx]
                .search
                .as_ref()
                .map(|s| s.items.as_slice())
                .unwrap_or(&[]);
            let matcher = SkimMatcherV2::default();
            items
                .iter()
                .enumerate()
                .filter_map(|(i, item)| {
                    let score = recursive_entries
                        .and_then(|entries| entries.get(i))
                        .map(|entry| matcher.fuzzy_match(&entry.search_text, &query))
                        .unwrap_or_else(|| matcher.fuzzy_match(&item.display_name(), &query));
                    score.map(|score| (score, i))
                })
                .collect()
        };

        let mut results: Vec<(i64, usize)> = scored;
        results.sort_unstable_by_key(|b| std::cmp::Reverse(b.0));
        let results: Vec<usize> = results.into_iter().map(|(_, i)| i).collect();

        if let Some(s) = self.libs[lib_idx].search.as_mut() {
            s.results = results;
            s.cursor = 0;
        }
    }
}
