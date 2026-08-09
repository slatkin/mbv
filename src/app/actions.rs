use super::notify_actions::ToastSeverity;
use super::ui_util::{is_playable, natural_sort_key, sort_audio_tracks};
use super::{
    App, BrowseLevel, LocalPlaybackTarget, PanelFocus, PlaybackTarget, RemotePlaybackTarget,
};
use mbv_core::api::EmbyItem;
use mbv_core::player::PlayerCommand;
use mbv_core::ItemId;
use std::sync::Arc;

pub(super) fn enqueue_action_context(
    item_id: &str,
    item_name: &str,
    source: &str,
    bypass: bool,
) -> String {
    let mut context =
        format!("user action=enqueue item_id={item_id:?} item_name={item_name:?} source={source}");
    if bypass {
        context.push_str(" reason=non-library thin-client owns playback");
    }
    context
}

/// Where playback should resume within a restored queue. Prefers locating
/// `last_played_item_id` by ID (robust to the saved `cursor` index having
/// drifted, e.g. if the list was edited before the last save) and falls back
/// to the saved cursor only when there's no last-played id to anchor on.
pub(crate) fn queue_restore_cursor(
    items: &[EmbyItem],
    saved_cursor: usize,
    last_played_item_id: Option<&str>,
    last_played_completed: bool,
) -> usize {
    let fallback = saved_cursor.min(items.len().saturating_sub(1));
    let Some(id) = last_played_item_id else {
        return fallback;
    };
    // If the last-played item is no longer in the restored list (e.g. it was
    // removed from the queue before quitting), fall back to the saved cursor
    // rather than silently jumping to the front of the queue.
    let Some(idx) = items.iter().position(|i| i.id == id) else {
        return fallback;
    };
    if last_played_completed {
        (idx + 1).min(items.len().saturating_sub(1))
    } else {
        idx
    }
}

impl App {
    pub(super) fn playback_target(&self) -> PlaybackTarget {
        match self.connected_session_id.clone() {
            Some(session_id) => PlaybackTarget::Remote(RemotePlaybackTarget { session_id }),
            None => PlaybackTarget::Local(LocalPlaybackTarget),
        }
    }

    pub(super) fn playback_display_target(&self) -> PlaybackTarget {
        if self.connected_session_state.is_some() {
            self.playback_target()
        } else {
            PlaybackTarget::Local(LocalPlaybackTarget)
        }
    }

    pub(super) fn playback_indicator_target(&self) -> PlaybackTarget {
        let local_active = self.player.status.lock().unwrap().active;
        if local_active {
            PlaybackTarget::Local(LocalPlaybackTarget)
        } else {
            self.playback_display_target()
        }
    }
}

impl App {
    pub(super) fn remote_audio_indexes(&self) -> Vec<i64> {
        self.connected_session_state
            .as_ref()
            .map(|state| {
                state
                    .media_info
                    .audio_streams
                    .iter()
                    .map(|stream| stream.index)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(super) fn remote_subtitle_indexes(&self) -> Vec<i64> {
        self.connected_session_state
            .as_ref()
            .map(|state| {
                state
                    .media_info
                    .subtitle_streams
                    .iter()
                    .map(|stream| stream.index)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(super) fn lib_page_size(&self) -> usize {
        // The library list is rendered into the right panel; use the panel
        // height directly (rows are single-line; subtract 1 for the
        // count/search header line).
        (self.layout.main.left_area.height as usize)
            .saturating_sub(1)
            .max(1)
    }

    pub(super) fn queue_page_size(&self) -> usize {
        self.layout.main.queue_area.height.saturating_sub(1).max(1) as usize
    }

    pub(super) fn current_home_item(&self) -> Option<EmbyItem> {
        let sec = self.home.section;
        if sec == 0 {
            self.home
                .continue_items
                .get(self.home.continue_cursor)
                .cloned()
        } else {
            let col = self.home.latest.get(sec - 1)?;
            col.2.get(col.3).cloned()
        }
    }

    pub(super) fn current_lib_item(&self) -> Option<EmbyItem> {
        let lib_idx = self.tab.library_index()?;
        let lib = self.libs.get(lib_idx)?;
        if lib.nav_stack.is_empty() {
            Some(lib.library.clone())
        } else {
            if let Some(s) = &lib.search {
                let idx = *s.results.get(s.cursor)?;
                return s.items.get(idx).cloned();
            }
            if self.is_feed_home_video_group_view(lib_idx) {
                return self.selected_feed_home_video_item(lib_idx);
            }
            // Track-selection mode (#145 task 4): when the left panel
            // is sitting on the album-folder-listing nav level AND a track
            // is focused (`album_track_focus = Some(idx)`), resolve to that
            // track instead of the album folder item, so play/enqueue/
            // context-menu actions target the focused track. Strictly
            // gated on `is_viewing_album_folders` -- per Task 3's
            // invariant, `album_track_focus` is only ever `Some` when that
            // holds, so this branch is unreachable from every other tab
            // and every other nav level. (The legacy `is_album_level`
            // drilldown this used to also be unreachable from was removed
            // entirely; mouse clicks now mirror Enter via
            // `activate_album_folder_row`.)
            if self.is_viewing_album_folders(lib_idx) {
                if let Some(track_idx) = lib.album_track_focus {
                    if let Some(album) = self.selected_album_item(lib_idx) {
                        if let Some(track) = self
                            .album_tracks_cache
                            .get(&album.id)
                            .and_then(|tracks| tracks.get(track_idx))
                        {
                            return Some(track.clone());
                        }
                    }
                    // Cache miss (async fetch still in flight) or an
                    // out-of-bounds index (shouldn't happen -- Up/Down
                    // clamps -- but stay safe): fall back to the album
                    // folder item below rather than returning None.
                }
            }
            let lvl = lib.nav_stack.last()?;
            lvl.items.get(lvl.cursor).cloned()
        }
    }

    pub(super) fn play_items_routed(&mut self, items: Vec<EmbyItem>, start_idx: usize) {
        if let Some(item) = items.get(start_idx).or_else(|| items.first()) {
            log::info!(target: "library_route", "user action=queue-replace item_id={:?} item_name={:?}", item.id, item.name);
            if self.in_non_library_thin_client_mode() {
                log::info!(target: "library_route", "route bypass action=queue-replace item_id={:?} item_name={:?} reason=non-library thin-client owns playback", item.id, item.name);
            } else {
                let item = item.clone();
                self.apply_route_for_playback(&item);
            }
        }
        let direct_remote = self.has_direct_remote_queue();
        if !direct_remote {
            self.on_queue_replace_silent();
        }
        self.set_queue_scope(self.playback_target_queue_scope());
        // Keep library focus when playing from the library panel.
        if !matches!(self.panel_focus, PanelFocus::Library) {
            self.set_panel_focus(PanelFocus::Queue);
        }
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            self.clear_playback_overlays();
            let id = conn_id.clone();
            let label = items
                .get(start_idx)
                .map(|i| i.playback_label())
                .unwrap_or_default();
            self.flash(
                format!("Requesting playback: {label}"),
                ToastSeverity::Neutral,
            );
            self.submit_attached_sequence(&id, &items, start_idx);
            return;
        }
        let c = Arc::new(self.client.lock().unwrap().clone());
        if direct_remote {
            if let Some(item) = items.get(start_idx) {
                self.flash(
                    format!("Requesting playback: {}", item.playback_label()),
                    ToastSeverity::Neutral,
                );
            }
        }
        self.player.play_queue(
            items,
            start_idx,
            self.queue_source.clone(),
            c,
            self.ui_volume,
        );
        self.player
            .send_command(PlayerCommand::SetMute(self.mute_on));
    }

    pub(super) fn play_item(&mut self, item: EmbyItem) {
        log::info!(target: "library_route", "user action=play item_id={:?} item_name={:?}", item.id, item.name);
        if self.in_non_library_thin_client_mode() {
            log::info!(target: "library_route", "route bypass action=play item_id={:?} item_name={:?} reason=non-library thin-client owns playback", item.id, item.name);
        } else {
            self.apply_route_for_playback(&item);
        }
        let direct_remote = self.has_direct_remote_queue();
        if !direct_remote {
            self.on_queue_replace_silent();
        }
        // Keep library focus when playing from the library panel.
        if !matches!(self.panel_focus, PanelFocus::Library) {
            self.set_panel_focus(PanelFocus::Queue);
        }
        let label = item.playback_label();
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            self.retire_remote_tracking(true);
            self.clear_playback_overlays();
            let id = conn_id.clone();
            let item_id = item.id.clone();
            let start_ticks = item.playback_position_ticks;
            self.flash(
                format!("Requesting playback: {label}"),
                ToastSeverity::Neutral,
            );
            self.do_session_command(move |c| c.session_play(&id, &item_id, start_ticks));
            return;
        }
        if !item.series_id.is_empty() && self.player.always_play_next {
            let c = self.client.lock().unwrap();
            let episodes = c.get_episodes_from(
                &ItemId::new(item.series_id.as_str()),
                &ItemId::new(item.id.as_str()),
            );
            drop(c);
            if episodes.len() > 1 {
                let c = Arc::new(self.client.lock().unwrap().clone());
                if !direct_remote {
                    self.on_queue_replace_silent();
                    self.replace_playback_queue(episodes.clone(), 0);
                }
                self.queue_source = crate::config::QueueSource::Series;
                self.player
                    .play_queue(episodes, 0, self.queue_source.clone(), c, self.ui_volume);
                self.player
                    .send_command(PlayerCommand::SetMute(self.mute_on));
                if !self.has_direct_remote_queue() {
                    self.save_queue_state();
                }
                return;
            }
        }
        let c = Arc::new(self.client.lock().unwrap().clone());
        if !direct_remote {
            self.replace_playback_queue(vec![item.clone()], 0);
        } else {
            self.flash(
                format!("Requesting playback: {label}"),
                ToastSeverity::Neutral,
            );
        }
        self.player
            .play(&item, self.queue_source.clone(), c, self.ui_volume);
        self.player
            .send_command(PlayerCommand::SetMute(self.mute_on));
    }

    pub(super) fn do_enqueue_folder(&mut self, item: mbv_core::api::EmbyItem) {
        log::info!(target: "library_route", "user action=enqueue item_id={:?} item_name={:?}", item.id, item.name);
        if self.in_non_library_thin_client_mode() {
            log::info!(target: "library_route", "route bypass action=enqueue item_id={:?} item_name={:?} reason=non-library thin-client owns playback", item.id, item.name);
        }
        let resolved = self.resolve_route_for_enqueue_folder(&item);
        if self.enqueue_route_conflict(resolved) {
            return;
        }
        let client = self.client.lock().unwrap();
        match client.get_all_playable_recursive(&item.id) {
            Ok(mut items) => {
                items.retain(|i| !i.is_folder);
                items.sort_by_key(|a| natural_sort_key(a.sort_key()));
                let count = items.len();
                drop(client);
                if count == 0 {
                    self.flash("Nothing to enqueue".into(), ToastSeverity::Error);
                    return;
                }
                let scope = self.visible_queue_scope();
                let appended = items.clone();
                let previous_dirty = self.queue_dirty;
                let previous_queue = self.queue_for_scope(scope).clone();
                {
                    let queue = self.queue_for_scope_mut(scope);
                    queue.append_items(items);
                }
                if self.local_queue_metadata_applies(scope) {
                    self.queue_dirty = true;
                }
                if self.sync_playback_queue_after_append(scope, appended) {
                    self.persist_local_queue_state_if_needed(scope);
                    self.retire_remote_tracking_after_queue_mutation();
                } else {
                    self.queue_dirty = previous_dirty;
                    *self.queue_for_scope_mut(scope) = previous_queue;
                }
            }
            Err(e) => {
                drop(client);
                self.flash(format!("Couldn't enqueue items: {e}"), ToastSeverity::Error);
            }
        }
    }

    pub(super) fn select_home(&mut self) {
        let Some(item) = self.current_home_item() else {
            return;
        };
        if item.is_folder {
            if let Some(i) = self.libs.iter().position(|l| l.library.id == item.id) {
                self.set_library_tab(i + 1);
                return;
            }
            let sec = self.home.section;
            if sec > 0 {
                if let Some(lib_id) = self.home.latest.get(sec - 1).map(|c| c.1.clone()) {
                    if let Some(lib_idx) = self.libs.iter().position(|l| l.library.id == lib_id) {
                        let lib = &mut self.libs[lib_idx];
                        lib.search = None;
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
                        self.set_library_tab(lib_idx + 1);
                        self.spawn_browse(
                            lib_idx,
                            item.id,
                            item.name,
                            None,
                            false,
                            "SortName".into(),
                            "Ascending".into(),
                        );
                    }
                }
            }
            return;
        }
        if is_playable(&item) {
            let fresh = {
                let c = self.client.lock().unwrap();
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
            self.play_item(fresh);
        }
    }

    pub(super) fn select(&mut self) {
        let Some(item) = self.current_lib_item() else {
            return;
        };
        if item.is_folder {
            let lib_idx = self.tab.library_index().unwrap();
            let lib = &mut self.libs[lib_idx];
            lib.search = None;
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
            let lib_idx = self.tab.library_index().unwrap();
            if self.libs[lib_idx].search.is_some() {
                self.libs[lib_idx].search = None;
                if self.is_feed_home_video_group_view(lib_idx) {
                    let pos = self
                        .feed_home_video_selected_items(lib_idx)
                        .iter()
                        .position(|i| i.id == item.id);
                    if let (Some(pos), Some(state)) =
                        (pos, self.libs[lib_idx].feed_home_video.as_mut())
                    {
                        state.video_cursor = pos;
                    }
                } else if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut() {
                    if let Some(pos) = lvl.items.iter().position(|i| i.id == item.id) {
                        lvl.cursor = pos;
                    }
                }
                self.save_default_library_position(lib_idx);
            }
            let fresh = {
                let c = self.client.lock().unwrap();
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
            let in_track_focus_mode = self.is_viewing_album_folders(lib_idx)
                && self.libs[lib_idx].album_track_focus.is_some();
            if self.libs[lib_idx].search.is_none() && in_track_focus_mode {
                let level_items = self
                    .selected_album_item(lib_idx)
                    .and_then(|album| self.album_tracks_cache.get(&album.id).cloned())
                    .unwrap_or_default();
                let mut tracks: Vec<EmbyItem> =
                    level_items.into_iter().filter(is_playable).collect();
                sort_audio_tracks(&mut tracks);
                if let Some(start_idx) = tracks.iter().position(|i| i.id == fresh.id) {
                    self.replace_playback_queue(tracks.clone(), start_idx);
                    self.queue_source = crate::config::QueueSource::Album;
                    if !self.has_direct_remote_queue() {
                        self.save_queue_state();
                    }
                    self.play_items_routed(tracks, start_idx);
                    return;
                }
            }
            let autoload = self.client.lock().unwrap().config.autoload;
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
                    let client = self.client.lock().unwrap();
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
                                self.play_items_routed(siblings, start_idx);
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

    /// Activation for a row in the album-folder listing
    /// (`is_viewing_album_folders` level). Shared by the Enter key and mouse
    /// click so the two paths cannot drift (see #145 / mouse-click parity fix).
    /// Precondition: caller has confirmed `is_viewing_album_folders(lib_idx)`.
    pub(super) fn activate_album_folder_row(&mut self, lib_idx: usize) {
        if self.libs[lib_idx].artist_header_focus.is_some() && self.is_music_group_view(lib_idx) {
            return;
        }
        if self.libs[lib_idx].album_track_focus.is_none() {
            self.clear_artist_header_focus(lib_idx);
            self.libs[lib_idx].album_track_focus = Some(0);
        } else {
            let has_focused_track = self
                .selected_album_item(lib_idx)
                .and_then(|album| {
                    self.album_tracks_cache.get(&album.id).and_then(|tracks| {
                        self.libs[lib_idx]
                            .album_track_focus
                            .and_then(|idx| tracks.get(idx))
                    })
                })
                .is_some();
            if !has_focused_track {
                return;
            }
            // Track already focused: play it. Reuses `select()` (track-focus
            // aware via `current_lib_item()`) rather than duplicating
            // queue-build logic here.
            self.select();
        }
    }

    pub(super) fn go_back(&mut self) {
        if let Some(lib_idx) = self.tab.library_index() {
            // Guard: don't pop when already at the root of a synthetic "group" view
            // (music groups: nav_stack[0]=groups, nav_stack[1]=albums; feed home
            // videos: nav_stack[0]=folders, nav_stack[1]=grouped videos) -- there is
            // no list above to go back to. Search-clearing still falls through
            // because this guard only fires when search is None.
            if self.libs[lib_idx].search.is_none()
                && self.libs[lib_idx].nav_stack.len() == 2
                && (self.is_music_group_view(lib_idx)
                    || self.is_feed_home_video_group_view(lib_idx))
            {
                return;
            }

            // Primary pop -- scoped so the mutable borrow of libs[lib_idx] ends here.
            let did_pop = {
                let lib = &mut self.libs[lib_idx];
                if lib.search.take().is_none() && lib.nav_stack.len() > 1 {
                    let child_folder_id = lib.nav_stack.last().map(|l| l.parent_id.clone());
                    lib.nav_stack.pop();
                    if let (Some(folder_id), Some(parent)) =
                        (child_folder_id, lib.nav_stack.last_mut())
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
}

#[cfg(test)]
#[path = "actions_tests_letter.rs"]
mod letter_tests;
#[cfg(test)]
#[path = "actions_tests_queue_state.rs"]
mod queue_state_tests;
#[cfg(test)]
#[path = "actions_tests_queue.rs"]
mod queue_tests;
#[cfg(test)]
#[path = "actions_tests_routes.rs"]
mod route_tests;
#[cfg(test)]
#[path = "actions_tests.rs"]
mod tests;
