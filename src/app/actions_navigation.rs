use super::ui_util::{is_playable, natural_sort_key, sort_audio_tracks};
use super::{App, BrowseLevel};
use mbv_core::api::EmbyItem;

use super::notify_actions::ToastSeverity;

impl App {
    pub(super) fn select(&mut self, lib_idx: usize) {
        let Some(item) = self.current_lib_item(lib_idx) else {
            return;
        };
        self.select_item(lib_idx, item);
    }

    pub(super) fn select_item(&mut self, lib_idx: usize, item: EmbyItem) {
        if item.is_folder {
            let lib = &mut self.libs[lib_idx];
            lib.nav_stack.push(BrowseLevel {
                parent_id: item.id.clone(),
                title: item.name.clone(),
                items: vec![],
                total_count: 0,
                cursor: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: true,
                scroll: 0,
                all_items: None,
                letter_filter: None,
                music_grouping: None,
            });
            self.save_default_library_position(lib_idx);
            self.spawn_browse(
                lib_idx,
                item.id,
                item.name,
                None,
                false,
                "SortName".into(),
                "Ascending".into(),
            );
        } else if is_playable(&item) {
            if self.is_feed_home_video_group_view(lib_idx) {
                let pos = self
                    .feed_home_video_selected_items(lib_idx)
                    .iter()
                    .position(|i| i.id == item.id);
                if let (Some(pos), Some(state)) = (pos, self.libs[lib_idx].feed_home_video.as_mut())
                {
                    state.video_cursor = pos;
                }
            } else if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut() {
                if let Some(pos) = lvl.items.iter().position(|i| i.id == item.id) {
                    lvl.cursor = pos;
                }
            }
            let fresh = {
                let Some(client) = self.emby_client() else {
                    self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
                    return;
                };
                let c = client.lock().unwrap();
                c.get_items_by_ids(std::slice::from_ref(&item.id))
                    .ok()
                    .and_then(|mut v| {
                        if v.is_empty() {
                            None
                        } else {
                            Some(v.remove(0))
                        }
                    })
                    .unwrap_or(item)
            };
            let autoload = self.config.lock().unwrap().autoload;
            if autoload {
                let parent_id = if self.is_feed_home_video_group_view(lib_idx) {
                    self.feed_home_video_selected_parent_id(lib_idx)
                } else {
                    self.libs[lib_idx]
                        .nav_stack
                        .last()
                        .map(|l| l.parent_id.clone())
                };
                if let Some(parent_id) = parent_id {
                    let Some(client) = self.emby_client() else {
                        self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
                        return;
                    };
                    let client = client.lock().unwrap();
                    match client.get_direct_playable(&parent_id) {
                        Ok(mut siblings) => {
                            siblings.retain(|i| !i.is_folder);
                            siblings.sort_by_key(|a| natural_sort_key(a.sort_key()));
                            if let Some(start_idx) = siblings.iter().position(|i| i.id == fresh.id)
                            {
                                let ct = self.libs[lib_idx].library.collection_type.clone();
                                drop(client);
                                self.replace_playback_queue(siblings.clone(), start_idx);
                                self.queue_source = crate::config::QueueSource::Collection {
                                    collection_type: ct,
                                };
                                if !self.has_direct_remote_queue() {
                                    self.save_queue_state();
                                }
                                self.play_items_routed(
                                    siblings,
                                    start_idx,
                                    self.queue_source.clone(),
                                );
                                return;
                            }
                            drop(client);
                        }
                        Err(_) => {
                            drop(client);
                        }
                    }
                }
            }
            self.play_item(fresh);
        }
    }

    /// Plays a track through the album queue path used by the wide music
    /// workspace. Returns false when the track is not in the cached album.
    pub(super) fn play_album_track(&mut self, album_id: &str, track: &EmbyItem) -> bool {
        let mut tracks: Vec<EmbyItem> = self
            .album_tracks_cache
            .get(album_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(is_playable)
            .collect();
        sort_audio_tracks(&mut tracks);
        let Some(start_idx) = tracks.iter().position(|item| item.id == track.id) else {
            return false;
        };
        if self.connected_session_id.is_none() && self.emby_snapshot().is_none() {
            self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
            return false;
        }
        self.queue_source = crate::config::QueueSource::Album;
        self.replace_playback_queue(tracks.clone(), start_idx);
        self.play_items_routed(tracks, start_idx, crate::config::QueueSource::Album);
        if !self.has_direct_remote_queue() {
            self.save_queue_state();
        }
        true
    }

    /// Activation for a row in the album-folder listing
    /// (`is_viewing_album_folders` level). Shared by the Enter key and mouse
    /// click so the two paths cannot drift (see #145 / mouse-click parity fix).
    /// Precondition: caller has confirmed `is_viewing_album_folders(lib_idx)`.
    ///
    /// Wide track-focus entry is owned by `MusicWorkspaceComponent` (Enter on
    /// an album row is component-local), so the wide arm is a no-op that only
    /// exists because the Enter key may still fall through to legacy while the
    /// album's tracks are loading. Narrow has no inline track list to focus
    /// (see `render_album_hero_detail`), so it opens the selection modal
    /// instead (design.md decision 6).
    pub(super) fn activate_album_folder_row(&mut self, lib_idx: usize) {
        if self.layout.main.is_wide_music_active() {
            return;
        }
        if let Some(album) = self.selected_album_item(lib_idx) {
            self.open_album_selection_modal(&album);
        }
    }

    pub(super) fn go_back(&mut self, lib_idx: usize) {
        // Defensive bounds check; see `move_lib_cursor_rows` in
        // `lib_cursor_actions.rs` for the stale index contract. Never
        // substitute library zero on a miss.
        if lib_idx >= self.libs.len() {
            return;
        }
        // Guard: don't pop when already at the root of a synthetic "group" view
        // (music groups: nav_stack[0]=groups, nav_stack[1]=albums; feed home
        // videos: nav_stack[0]=folders, nav_stack[1]=grouped videos) -- there is
        // no list above to go back to. Search-clearing still falls through
        // because this guard only fires when search is None.
        if self.libs[lib_idx].nav_stack.len() == 2
            && (self.is_music_group_view(lib_idx) || self.is_feed_home_video_group_view(lib_idx))
        {
            return;
        }

        // Primary pop -- scoped so the mutable borrow of libs[lib_idx] ends here.
        let did_pop = {
            let lib = &mut self.libs[lib_idx];
            if lib.nav_stack.len() > 1 {
                let child_folder_id = lib.nav_stack.last().map(|l| l.parent_id.clone());
                lib.nav_stack.pop();
                if let (Some(folder_id), Some(parent)) = (child_folder_id, lib.nav_stack.last_mut())
                {
                    if let Some(idx) = parent.items.iter().position(|i| i.id == folder_id) {
                        parent.cursor = idx;
                    }
                }
                true
            } else {
                false
            }
        };

        if did_pop {
            self.save_default_library_position(lib_idx);

            // Skip past the auto-pushed Season level so a single Escape
            // takes the user back to the series list.
            let exposed_seasons = self.libs[lib_idx]
                .nav_stack
                .last()
                .map(|l| {
                    l.items
                        .first()
                        .map(|i| i.item_type == "Season")
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            if exposed_seasons && self.libs[lib_idx].nav_stack.len() > 1 {
                let child_id2 = self.libs[lib_idx]
                    .nav_stack
                    .last()
                    .map(|l| l.parent_id.clone());
                self.libs[lib_idx].nav_stack.pop();
                if let (Some(fid), Some(parent)) =
                    (child_id2, self.libs[lib_idx].nav_stack.last_mut())
                {
                    if let Some(idx) = parent.items.iter().position(|i| i.id == fid) {
                        parent.cursor = idx;
                    }
                }
            }
        }
        self.save_default_library_position(lib_idx);
    }
}
