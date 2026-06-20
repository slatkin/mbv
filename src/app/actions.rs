use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use rand::seq::SliceRandom;
use crate::api::{EmbyClient, MediaItem, TICKS_PER_SECOND};
use crate::player::PlayerCommand;
use crate::ws::WsEvent;
use super::{
    App, PAGE_SIZE, PREFETCH_AHEAD,
    PendingQueueAction, ContextAction, LibEvent, SessionEvent, BrowseLevel, PowerFocus,
};
use super::ui_util::{natural_sort_key, is_playable, sort_episodes, sort_audio_tracks};

impl App {
    pub(super) fn lib_page_size(&self) -> usize {
        self.layout_lib_row_heights.len().saturating_sub(1).max(1)
    }

    pub(super) fn playlist_page_size(&self) -> usize {
        self.layout_playlist_inner.height.saturating_sub(2).max(1) as usize
    }

    pub(super) fn move_lib_cursor(&mut self, delta: i64) {
        let now = Instant::now();
        let idle = now.duration_since(self.last_nav_at) >= Duration::from_millis(150);
        self.last_nav_at = now;
        let lib_off = self.lib_tab_offset();
        let lib_idx = self.tab_idx - lib_off;
        let lib = &mut self.libs[lib_idx];
        if let Some(s) = &mut lib.search {
            let n = s.results.len();
            if n > 0 {
                s.cursor = (s.cursor as i64 + delta).clamp(0, n as i64 - 1) as usize;
            }
            return;
        }
        if let Some(lvl) = lib.nav_stack.last_mut() {
            let n = lvl.items.len();
            if n > 0 {
                lvl.cursor = (lvl.cursor as i64 + delta).clamp(0, n as i64 - 1) as usize;
            }
        }
        if idle { self.maybe_fetch_next_page(lib_idx); }
    }

    pub(super) fn jump_lib_cursor(&mut self, to_end: bool) {
        let lib_off = self.lib_tab_offset();
        let lib_idx = self.tab_idx - lib_off;
        let lib = &mut self.libs[lib_idx];
        if let Some(s) = &mut lib.search {
            let n = s.results.len();
            if n > 0 { s.cursor = if to_end { n - 1 } else { 0 }; }
            return;
        }
        if let Some(lvl) = lib.nav_stack.last_mut() {
            let n = lvl.items.len();
            if n > 0 { lvl.cursor = if to_end { n - 1 } else { 0 }; }
        }
        self.maybe_fetch_next_page(lib_idx);
    }

    pub(super) fn move_home_cursor(&mut self, delta: i64) {
        let sec = self.home.section;
        let (len, cur) = self.home_section_len_cur(sec);
        if delta > 0 {
            if cur + 1 < len { self.set_home_cursor(sec, cur + 1); }
        } else {
            if cur > 0 { self.set_home_cursor(sec, cur - 1); }
        }
    }

    pub(super) fn ensure_home_section_visible(&mut self) {
        let active = self.player.status.lock().unwrap().active;
        let chrome: u16 = if active { 6 } else { 3 };
        let panel_h = self.terminal_height.saturating_sub(chrome);

        let n_latest = self.home.latest.len();
        let n_sections = 1 + n_latest;

        if self.home_card_view {
            let compact = self.terminal_height < 28;
            let max_h_full = if panel_h < 12 { panel_h }
                             else { ((panel_h as u32 * 24 / 25) as u16).min(24) }.max(4);
            let side_h_full   = ((max_h_full as u32 * 4 / 5) as u16).max(3);
            let center_h_full = if compact { side_h_full } else { side_h_full + 2 };
            let visible = (panel_h / center_h_full).max(1).min(n_sections as u16) as usize;
            let sec = self.home.section;
            if sec < self.home_cards_section_offset {
                self.home_cards_section_offset = sec;
            } else if sec >= self.home_cards_section_offset + visible {
                self.home_cards_section_offset = sec + 1 - visible;
            }
            let max_offset = n_sections.saturating_sub(visible);
            if self.home_cards_section_offset > max_offset {
                self.home_cards_section_offset = max_offset;
            }
            return;
        }

        let n_rows = 1 + (n_latest + 1) / 2;
        let visible_rows = if (n_rows as u16) * super::HOME_MIN_SECTION_H <= panel_h {
            n_rows
        } else {
            ((panel_h / super::HOME_MIN_SECTION_H) as usize).max(1)
        };

        let sec = self.home.section;
        let sec_row = if sec == 0 { 0 } else { 1 + (sec - 1) / 2 };
        if sec_row < self.home_panel_section_offset {
            self.home_panel_section_offset = sec_row;
        } else if sec_row >= self.home_panel_section_offset + visible_rows {
            self.home_panel_section_offset = sec_row + 1 - visible_rows;
        }
        let max_offset = n_rows.saturating_sub(visible_rows);
        if self.home_panel_section_offset > max_offset {
            self.home_panel_section_offset = max_offset;
        }
    }

    pub(super) fn presentation_scrollbar_seek(&mut self, row: u16) {
        let sb = self.layout_presentation_sb;
        if sb.height == 0 { return; }
        let n = self.player_tab.items.len();
        if n == 0 { return; }
        let frac = (row.saturating_sub(sb.y)) as f64 / sb.height as f64;
        let target = ((frac * n as f64).round() as usize).min(n - 1);
        self.player_tab.playlist_cursor = target;
    }

    pub(super) fn home_scrollbar_seek(&mut self, row: u16) {
        let sb = self.layout_home_scrollbar;
        if sb.height == 0 { return; }
        let active = self.player.status.lock().unwrap().active;
        let chrome: u16 = if active { 6 } else { 3 };
        let panel_h = self.terminal_height.saturating_sub(chrome);
        let n_latest = self.home.latest.len();
        let n_rows = 1 + (n_latest + 1) / 2;
        let visible_rows = ((panel_h / super::HOME_MIN_SECTION_H) as usize).max(1).min(n_rows);
        let max_offset = n_rows.saturating_sub(visible_rows);
        if max_offset == 0 { return; }
        let frac = (row.saturating_sub(sb.y)) as f64 / sb.height as f64;
        let new_offset = ((frac * max_offset as f64).round() as usize).min(max_offset);
        self.home_panel_section_offset = new_offset;
    }

    pub(super) fn home_section_len_cur(&self, sec: usize) -> (usize, usize) {
        if sec == 0 {
            (self.home.continue_items.len(), self.home.continue_cursor)
        } else {
            self.home.latest.get(sec - 1)
                .map(|c| (c.2.len(), c.3))
                .unwrap_or((0, 0))
        }
    }

    pub(super) fn set_home_cursor(&mut self, sec: usize, val: usize) {
        if sec == 0 {
            self.home.continue_cursor = val;
        } else if let Some(col) = self.home.latest.get_mut(sec - 1) {
            col.3 = val;
        }
    }

    pub(super) fn current_home_item(&self) -> Option<MediaItem> {
        let sec = self.home.section;
        if sec == 0 {
            self.home.continue_items.get(self.home.continue_cursor).cloned()
        } else {
            let col = self.home.latest.get(sec - 1)?;
            col.2.get(col.3).cloned()
        }
    }

    pub(super) fn current_lib_item(&self) -> Option<MediaItem> {
        let lib = self.libs.get(self.tab_idx - self.lib_tab_offset())?;
        if lib.nav_stack.is_empty() {
            Some(lib.library.clone())
        } else {
            if let Some(s) = &lib.search {
                let idx = *s.results.get(s.cursor)?;
                return s.items.get(idx).cloned();
            }
            let lvl = lib.nav_stack.last()?;
            lvl.items.get(lvl.cursor).cloned()
        }
    }

    pub(super) fn is_album_level(&self, lib_idx: usize) -> bool {
        let lib = &self.libs[lib_idx];
        if lib.library.collection_type != "music" { return false; }
        if self.music_levels.is_empty() { return false; }
        let stack_len = lib.nav_stack.len();
        if stack_len < 2 { return false; }
        self.music_levels.get(stack_len - 2).map(|s| s == "album").unwrap_or(false)
    }

    pub(super) fn is_viewing_album_folders(&self, lib_idx: usize) -> bool {
        let lib = &self.libs[lib_idx];
        if lib.library.collection_type != "music" { return false; }
        if self.music_levels.is_empty() { return false; }
        let stack_len = lib.nav_stack.len();
        if stack_len < 1 { return false; }
        self.music_levels.get(stack_len - 1).map(|s| s == "album").unwrap_or(false)
    }

    pub(super) fn is_viewing_season_grid(&self, lib_idx: usize) -> bool {
        let lib = &self.libs[lib_idx];
        if lib.search.is_some() { return false; }
        let lvl = match lib.nav_stack.last() { Some(l) => l, None => return false };
        lvl.items.first().map(|i| i.item_type == "Season").unwrap_or(false)
    }

    pub(super) fn is_audio_item(&self) -> bool {
        let idx = self.player_tab.playlist_cursor;
        self.player_tab.items.get(idx)
            .map(|i| i.media_type == "Audio" || i.item_type == "Audio")
            .unwrap_or(false)
    }

    pub(super) fn toggle_mute(&mut self) {
        if self.ui_volume == 0 {
            if let Some(v) = self.pre_mute_volume.take() {
                self.player.send_command(PlayerCommand::SetVolume(v as i64));
                self.ui_volume = v;
            }
        } else {
            self.pre_mute_volume = Some(self.ui_volume);
            self.player.send_command(PlayerCommand::SetVolume(0));
            self.ui_volume = 0;
        }
    }

    pub(super) fn cycle_audio(&mut self) {
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            let cur = self.connected_session_state.as_ref().map(|s| s.audio_index).unwrap_or(1);
            let next = if cur <= 1 { 2 } else { 1 };
            let id = conn_id.clone();
            if let Some(ref mut state) = self.connected_session_state {
                state.audio_index = next;
            }
            self.do_session_command(move |c| c.session_set_audio_index(&id, next));
            return;
        }
        let (tracks, current_id) = {
            let s = self.player.status.lock().unwrap();
            (s.audio_tracks.clone(), s.audio_id)
        };
        if tracks.is_empty() { return; }
        let mut entries: Vec<i64> = vec![0];
        entries.extend(tracks.iter().map(|(id, _)| *id));
        let cur = entries.iter().position(|&id| id == current_id).unwrap_or(0);
        let next = (cur + 1) % entries.len();
        let next_id = entries[next];
        if next_id == 0 {
            self.pre_mute_volume = Some(self.ui_volume);
            self.player.send_command(PlayerCommand::SetVolume(0));
            self.ui_volume = 0;
        } else if current_id == 0 {
            if let Some(v) = self.pre_mute_volume.take() {
                self.player.send_command(PlayerCommand::SetVolume(v as i64));
                self.ui_volume = v;
            }
        }
        self.player.send_command(PlayerCommand::SetAudio(next_id));
    }

    pub(super) fn toggle_sub(&mut self) {
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            let idx = self.connected_session_state.as_ref().map(|s| s.sub_index).unwrap_or(-1);
            let next = if idx == -1 { 1i64 } else { -1i64 };
            let id = conn_id.clone();
            if let Some(ref mut state) = self.connected_session_state {
                state.sub_index = next;
            }
            self.do_session_command(move |c| c.session_set_subtitle_index(&id, next));
            return;
        }
        let (tracks, current_id) = {
            let s = self.player.status.lock().unwrap();
            (s.sub_tracks.clone(), s.sub_id)
        };
        let currently_off = self.player.subs_off.load(std::sync::atomic::Ordering::Relaxed);
        if currently_off {
            self.player.subs_off.store(false, std::sync::atomic::Ordering::Relaxed);
            if let Some(&(first_id, _)) = tracks.first() {
                if current_id == 0 {
                    self.player.send_command(PlayerCommand::SetSub(first_id));
                }
            }
        } else {
            self.player.subs_off.store(true, std::sync::atomic::Ordering::Relaxed);
            if current_id != 0 {
                self.player.send_command(PlayerCommand::SetSub(0));
            }
        }
        self.save_prefs();
    }

    pub(super) fn cycle_sub(&mut self) {
        if let Some(ref _conn_id) = self.connected_session_id.clone() {
            self.toggle_sub();
            return;
        }
        let (tracks, current_id) = {
            let s = self.player.status.lock().unwrap();
            (s.sub_tracks.clone(), s.sub_id)
        };
        if tracks.is_empty() { return; }
        let mut entries: Vec<i64> = vec![0];
        entries.extend(tracks.iter().map(|(id, _)| *id));
        let cur = entries.iter().position(|&id| id == current_id).unwrap_or(0);
        let next = (cur + 1) % entries.len();
        let next_id = entries[next];
        self.player.subs_off.store(next_id == 0, std::sync::atomic::Ordering::Relaxed);
        self.player.send_command(PlayerCommand::SetSub(next_id));
        self.save_prefs();
    }

    pub(super) fn remove_from_playlist(&mut self, pos: usize) {
        let (active, current_idx) = {
            let s = self.player.status.lock().unwrap();
            (s.active, s.current_idx)
        };
        if active && current_idx == pos {
            self.confirm_remove_idx = Some(pos);
            self.flash_status("Remove now-playing item and stop playback? (y/N)".into());
            return;
        }
        let item = self.player_tab.items.remove(pos);
        self.queue_dirty = true;
        self.playlist_undo_stack.push((pos, item));
        self.save_queue_state();
        if active {
            self.player.send_command(PlayerCommand::PlaylistRemove(pos));
        }
        if !self.player_tab.items.is_empty() {
            self.player_tab.playlist_cursor =
                self.player_tab.playlist_cursor.min(self.player_tab.items.len() - 1);
        } else {
            self.player_tab.playlist_cursor = 0;
        }
    }

    fn notify_system(&self, msg: &str) {
        if self.system_notifications {
            let tx = self.notif_action_tx.clone();
            let mut cmd = std::process::Command::new("notify-send");
            cmd.arg("--app-name=mbv").arg("mbv").arg(msg)
                .stderr(std::process::Stdio::null());
            std::thread::spawn(move || {
                if cmd.output().map(|o| o.status.success()).unwrap_or(false) == false {
                    let _ = tx.send("__notif_failed__".into());
                }
            });
        }
    }

    pub(super) fn notify_with_actions(&self, title: &str, body: &str, actions: &[(&str, &str)]) {
        if !self.system_notifications { return; }
        let mut cmd = std::process::Command::new("notify-send");
        cmd.arg("--app-name=mbv").arg(title).arg(body)
            .stderr(std::process::Stdio::null());
        for (id, label) in actions {
            cmd.arg(format!("--action={}={}", id, label));
        }
        let tx = self.notif_action_tx.clone();
        std::thread::spawn(move || {
            match cmd.output() {
                Ok(out) if out.status.success() => {
                    let chosen = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    let _ = tx.send(chosen);
                }
                _ => { let _ = tx.send("__notif_failed__".into()); }
            }
        });
    }

    pub(super) fn trigger_lib_rescan(&mut self, lib_idx: usize) {
        let client = self.client.lock().unwrap().clone();
        let library_id = self.libs[lib_idx].library.id.clone();
        let name = self.libs[lib_idx].library.name.clone();
        std::thread::spawn(move || { let _ = client.post_library_refresh(&library_id); });
        self.flash_status(format!("Scanning '{name}'..."));
    }

    pub(super) fn flash_status(&mut self, msg: String) {
        self.notify_system(&msg);
        self.status = msg;
        self.status_expires = Some(Instant::now() + Duration::from_secs(3));
    }

    pub(super) fn effective_playback_state(&self) -> (bool, usize, i64, i64, bool) {
        if let Some(ref remote) = self.connected_session_state {
            let active_idx = remote.now_playing_item_id.as_ref()
                .and_then(|id| self.player_tab.items.iter().position(|it| &it.id == id))
                .unwrap_or(0);
            let pos_ticks = {
                let elapsed_s = if remote.is_paused { 0.0 } else { self.remote_pos_at.elapsed().as_secs_f64() };
                let pos_s = (self.remote_pos_s as f64 + elapsed_s).min(remote.runtime_s as f64);
                (pos_s * crate::api::TICKS_PER_SECOND as f64) as i64
            };
            (remote.now_playing.is_some(),
             active_idx,
             pos_ticks,
             remote.runtime_s * crate::api::TICKS_PER_SECOND,
             remote.is_paused)
        } else {
            let s = self.player.status.lock().unwrap();
            (s.active, s.current_idx, s.position_ticks, s.runtime_ticks, s.paused)
        }
    }

    pub(super) fn play_items_routed(&mut self, items: Vec<MediaItem>, start_idx: usize) {
        self.on_queue_replace_silent();
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            self.clear_playback_overlays();
            let id = conn_id.clone();
            let item_ids: Vec<String> = items.iter().map(|i| i.id.clone()).collect();
            let start_ticks = items.get(start_idx).map_or(0, |i| i.playback_position_ticks);
            let label = items.get(start_idx).map(|i| i.playback_label()).unwrap_or_default();
            self.flash_status(format!("Playing on remote: {label}"));
            self.do_session_command(move |c| c.session_play_items(&id, &item_ids, start_idx, start_ticks));
            return;
        }
        let c = Arc::new(self.client.lock().unwrap().clone());
        self.player.play_playlist(items, start_idx, c, self.ui_volume);
        self.player.send_command(PlayerCommand::SetMute(self.mute_on));
    }

    pub(super) fn play_item(&mut self, item: MediaItem) {
        self.on_queue_replace_silent();
        let label = item.playback_label();
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            self.clear_playback_overlays();
            let id = conn_id.clone();
            let item_id = item.id.clone();
            let start_ticks = item.playback_position_ticks;
            self.flash_status(format!("Playing on remote: {label}"));
            self.do_session_command(move |c| c.session_play(&id, &item_id, start_ticks));
            return;
        }
        if !item.series_id.is_empty() && self.player.always_play_next {
            let c = self.client.lock().unwrap();
            let episodes = c.get_episodes_from(&item.series_id, &item.id);
            drop(c);
            if episodes.len() > 1 {
                let c = Arc::new(self.client.lock().unwrap().clone());
                self.on_queue_replace_silent();
                self.player_tab.items = episodes.clone();
                self.player_tab.playlist_cursor = 0;
                self.player.play_playlist(episodes, 0, c, self.ui_volume);
                self.player.send_command(PlayerCommand::SetMute(self.mute_on));
                self.queue_source = crate::config::QueueSource::Series;
                self.save_queue_state();
                return;
            }
        }
        let c = Arc::new(self.client.lock().unwrap().clone());
        self.player_tab.items = vec![item.clone()];
        self.player_tab.playlist_cursor = 0;
        self.player.play(&item, c, self.ui_volume);
        self.player.send_command(PlayerCommand::SetMute(self.mute_on));
    }

    pub(super) fn enqueue_selected(&mut self) {
        if self.tab_idx == 0 {
            let Some(item) = self.current_home_item() else { return };
            if item.is_folder { self.do_enqueue_folder(item); return; }
            if !is_playable(&item) { return; }
            let name = item.display_name();
            self.player_tab.items.push(item);
            self.queue_dirty = true;
            self.flash_status(format!("Added: {name}"));
            self.save_queue_state();
        } else if self.tab_idx >= 2 && self.tab_idx != self.log_tab_idx() {
            let Some(item) = self.current_lib_item() else { return };
            if item.is_folder { self.do_enqueue_folder(item); return; }
            if !is_playable(&item) { return; }
            let name = item.display_name();
            self.player_tab.items.push(item);
            self.queue_dirty = true;
            self.flash_status(format!("Added: {name}"));
            self.save_queue_state();
        }
    }

    pub(super) fn do_enqueue_folder(&mut self, item: crate::api::MediaItem) {
        let client = self.client.lock().unwrap();
        match client.get_all_playable_recursive(&item.id) {
            Ok(mut items) => {
                items.retain(|i| !i.is_folder);
                items.sort_by_key(|a| natural_sort_key(a.sort_key()));
                let count = items.len();
                drop(client);
                if count == 0 { self.flash_status("Nothing to enqueue".into()); return; }
                for i in items { self.player_tab.items.push(i); }
                self.queue_dirty = true;
                self.flash_status(format!("Enqueued {count} items from {}", item.display_name()));
                self.save_queue_state();
            }
            Err(e) => { drop(client); self.flash_status(format!("Error: {e}")); }
        }
    }

    pub(super) fn select_home(&mut self) {
        let Some(item) = self.current_home_item() else { return };
        if item.is_folder {
            if let Some(i) = self.libs.iter().position(|l| l.library.id == item.id) {
                self.set_tab(i + 2);
                return;
            }
            let sec = self.home.section;
            if sec > 0 {
                if let Some(lib_id) = self.home.latest.get(sec - 1).map(|c| c.1.clone()) {
                    if let Some(lib_idx) = self.libs.iter().position(|l| l.library.id == lib_id) {
                        let lib = &mut self.libs[lib_idx];
                        lib.search = None;
                        lib.nav_stack.push(BrowseLevel {
                            parent_id: item.id.clone(), title: item.name.clone(),
                            items: vec![], total_count: 0, cursor: 0,
                            item_types: None, unplayed_only: false,
                            sort_by: "SortName".into(), sort_order: "Ascending".into(),
                            loading: true, all_items: None,
                        });
                        self.set_tab(lib_idx + 2);
                        self.spawn_browse(lib_idx, item.id, item.name, None, false, "SortName".into(), "Ascending".into());
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
                    .and_then(|mut v| if v.is_empty() { None } else { Some(v.remove(0)) })
                    .unwrap_or(item)
            };
            self.play_item(fresh);
        }
    }

    pub(super) fn select(&mut self) {
        let Some(item) = self.current_lib_item() else { return };
        if item.is_folder {
            let lib_idx = self.tab_idx - self.lib_tab_offset();
            let lib = &mut self.libs[lib_idx];
            lib.search = None;
            lib.nav_stack.push(BrowseLevel {
                parent_id: item.id.clone(), title: item.name.clone(),
                items: vec![], total_count: 0, cursor: 0,
                item_types: None, unplayed_only: false,
                sort_by: "SortName".into(), sort_order: "Ascending".into(),
                loading: true, all_items: None,
            });
            if let Some(v) = self.layout_lib_scroll.get_mut(lib_idx) { *v = 0; }
            self.spawn_browse(lib_idx, item.id, item.name, None, false, "SortName".into(), "Ascending".into());
        } else if is_playable(&item) {
            let lib_idx = self.tab_idx - self.lib_tab_offset();
            if self.libs[lib_idx].search.is_some() {
                self.libs[lib_idx].search = None;
                if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut() {
                    if let Some(pos) = lvl.items.iter().position(|i| i.id == item.id) {
                        lvl.cursor = pos;
                    }
                }
                if let Some(v) = self.layout_lib_scroll.get_mut(lib_idx) { *v = 0; }
            }
            let fresh = {
                let c = self.client.lock().unwrap();
                c.get_items_by_ids(std::slice::from_ref(&item.id))
                    .ok()
                    .and_then(|mut v| if v.is_empty() { None } else { Some(v.remove(0)) })
                    .unwrap_or(item)
            };
            if self.libs[lib_idx].search.is_none() && self.is_album_level(lib_idx) {
                let level_items = self.libs[lib_idx].nav_stack.last()
                    .map(|l| l.items.clone())
                    .unwrap_or_default();
                let mut tracks: Vec<MediaItem> = level_items.into_iter()
                    .filter(|i| is_playable(i))
                    .collect();
                sort_audio_tracks(&mut tracks);
                if let Some(start_idx) = tracks.iter().position(|i| i.id == fresh.id) {
                    self.player_tab.items = tracks.clone();
                    self.player_tab.playlist_cursor = start_idx;
                    self.play_items_routed(tracks, start_idx);
                    self.queue_source = crate::config::QueueSource::Album;
                    self.save_queue_state();
                    return;
                }
            }
            let autoload = self.client.lock().unwrap().config.autoload;
            if autoload {
                if let Some(parent_id) = self.libs[lib_idx].nav_stack.last().map(|l| l.parent_id.clone()) {
                    let client = self.client.lock().unwrap();
                    match client.get_direct_playable(&parent_id) {
                        Ok(mut siblings) => {
                            siblings.retain(|i| !i.is_folder);
                            siblings.sort_by_key(|a| natural_sort_key(a.sort_key()));
                            if let Some(start_idx) = siblings.iter().position(|i| i.id == fresh.id) {
                                let ct = self.libs[lib_idx].library.collection_type.clone();
                                drop(client);
                                self.player_tab.items = siblings.clone();
                                self.player_tab.playlist_cursor = start_idx;
                                self.play_items_routed(siblings, start_idx);
                                self.queue_source = crate::config::QueueSource::Collection { collection_type: ct };
                                self.save_queue_state();
                                return;
                            }
                            drop(client);
                        }
                        Err(_) => { drop(client); }
                    }
                }
            }
            self.play_item(fresh);
        }
    }

    pub(super) fn go_back(&mut self) {
        if self.tab_idx > 1 && self.tab_idx != self.log_tab_idx() {
            let lib_off = self.lib_tab_offset();
            let lib_idx = self.tab_idx - lib_off;
            let lib = &mut self.libs[lib_idx];
            if lib.search.take().is_none() && lib.nav_stack.len() > 1 {
                let child_folder_id = lib.nav_stack.last().map(|l| l.parent_id.clone());
                lib.nav_stack.pop();
                if let (Some(folder_id), Some(parent)) = (child_folder_id, lib.nav_stack.last_mut()) {
                    if let Some(idx) = parent.items.iter().position(|i| i.id == folder_id) {
                        parent.cursor = idx;
                    }
                }
                if let Some(v) = self.layout_lib_scroll.get_mut(lib_idx) { *v = 0; }
            }
        }
    }

    pub(super) fn execute_context_action(&mut self, action: Option<ContextAction>) {
        match action {
            Some(ContextAction::Play) => {
                if self.tab_idx == 0 { self.select_home(); }
                else if self.tab_idx == 1 {
                    let t = self.player_tab.playlist_cursor;
                    if t < self.player_tab.items.len() {
                        if let Some(ref conn_id) = self.connected_session_id.clone() {
                            let item = self.player_tab.items[t].clone();
                            let id = conn_id.clone();
                            let item_ids: Vec<String> = self.player_tab.items.iter().map(|i| i.id.clone()).collect();
                            let start_ticks = item.playback_position_ticks;
                            let label = item.playback_label();
                            self.flash_status(format!("Playing on remote: {label}"));
                            self.do_session_command(move |c| c.session_play_items(&id, &item_ids, t, start_ticks));
                        } else {
                            self.player.send_command(PlayerCommand::JumpTo(t));
                        }
                    }
                }
                else { self.select(); }
            }
            Some(ContextAction::PlayFolder(id)) => {
                let ct = if self.tab_idx > 1 { self.libs[self.tab_idx - self.lib_tab_offset()].library.collection_type.clone() } else { String::new() };
                self.play_folder(&id);
                self.queue_source = crate::config::QueueSource::Collection { collection_type: ct };
                self.save_queue_state();
            }
            Some(ContextAction::ShuffleFolder(id)) => {
                self.shuffle_folder(&id);
            }
            Some(ContextAction::Enqueue) => self.enqueue_selected(),
            Some(ContextAction::EnqueueFolder(item)) => self.do_enqueue_folder(item),
            Some(ContextAction::MarkPlayed(id))   => self.context_set_played(&id, true),
            Some(ContextAction::MarkUnplayed(id)) => self.context_set_played(&id, false),
            Some(ContextAction::RemoveFromContinueWatching) => self.remove_from_continue_watching(),
            Some(ContextAction::RemoveFromPlaylist(pos)) => self.remove_from_playlist(pos),
            Some(ContextAction::GoToLibrary(item_id)) => {
                let lib_ids: Vec<(usize, String)> = self.libs.iter().enumerate()
                    .map(|(i, lib)| (i, lib.library.id.clone()))
                    .collect();
                self.spawn_navigate_to_item(item_id, lib_ids);
            }
            None => {}
        }
    }

    fn context_set_played(&mut self, item_id: &str, played: bool) {
        let client = self.client.lock().unwrap();
        let result = if played { client.mark_played(item_id) } else { client.mark_unplayed(item_id) };
        drop(client);
        match result {
            Ok(()) => {
                if self.tab_idx == 0 { let _ = self.fetch_home(); } else { self.refresh_lib(); }
            }
            Err(e) => self.flash_status(format!("Error: {e}")),
        }
    }

    fn remove_from_continue_watching(&mut self) {
        let Some(item) = self.home.continue_items.get(self.home.continue_cursor).cloned() else { return };
        let client = self.client.lock().unwrap();
        let result = client.hide_from_resume(&item.id);
        drop(client);
        match result {
            Ok(()) => { let _ = self.fetch_home(); }
            Err(e) => self.flash_status(format!("Error: {e}")),
        }
    }

    pub(super) fn toggle_watched_home(&mut self) {
        let Some(item) = self.current_home_item() else { return };
        if item.is_folder || item.is_audio() { return; }
        let client = self.client.lock().unwrap();
        let result = if item.played { client.mark_unplayed(&item.id) } else { client.mark_played(&item.id) };
        drop(client);
        match result {
            Ok(()) => { let _ = self.fetch_home(); }
            Err(e) => self.flash_status(format!("Error: {e}")),
        }
    }

    pub(super) fn toggle_watched(&mut self) {
        let Some(item) = self.current_lib_item() else { return };
        if item.is_folder || item.is_audio() { return; }
        let client = self.client.lock().unwrap();
        let result = if item.played { client.mark_unplayed(&item.id) } else { client.mark_played(&item.id) };
        drop(client);
        match result {
            Ok(()) => self.refresh_lib(),
            Err(e) => self.flash_status(format!("Error: {e}")),
        }
    }

    pub(super) fn refresh_lib(&mut self) {
        if self.tab_idx <= 1 || self.tab_idx == self.log_tab_idx() { return; }
        let lib_idx = self.tab_idx - self.lib_tab_offset();
        if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut() {
            lvl.loading = true;
            let parent_id = lvl.parent_id.clone();
            let item_types = lvl.item_types.clone();
            let unplayed_only = lvl.unplayed_only;
            let sort_by = lvl.sort_by.clone();
            let sort_order = lvl.sort_order.clone();
            let loaded_count = lvl.items.len();
            self.spawn_refresh(lib_idx, parent_id, item_types, unplayed_only, sort_by, sort_order, loaded_count);
        }
    }

    fn refresh_queue(&mut self) {
        if self.player_tab.items.is_empty() { return; }
        let ids: Vec<String> = self.player_tab.items.iter().map(|i| i.id.clone()).collect();
        let client = self.client.lock().unwrap();
        if let Ok(fetched) = client.get_items_by_ids(&ids) {
            let mut map: HashMap<String, crate::api::MediaItem> =
                fetched.into_iter().map(|i| (i.id.clone(), i)).collect();
            for item in &mut self.player_tab.items {
                if let Some(fresh) = map.remove(&item.id) {
                    *item = fresh;
                }
            }
        }
    }

    pub(super) fn refresh_current_view(&mut self) {
        self.force_clear = true;
        if self.tab_idx == 0 {
            if let Err(e) = self.fetch_home() {
                self.flash_status(format!("Refresh error: {e}"));
            }
        } else if self.tab_idx == 1 {
            self.refresh_queue();
        } else if self.tab_idx != self.log_tab_idx() {
            self.refresh_lib();
        }
    }

    pub(super) fn shuffle_play(&mut self) {
        if self.tab_idx <= 1 || self.tab_idx == self.log_tab_idx() { return; }
        let lib_idx = self.tab_idx - self.lib_tab_offset();
        let parent_id = {
            let lib = &self.libs[lib_idx];
            let item = lib.nav_stack.last().and_then(|lvl| {
                let idx = lib.search.as_ref()
                    .and_then(|s| s.results.get(s.cursor).copied())
                    .unwrap_or(lvl.cursor);
                lvl.items.get(idx)
            });
            item.filter(|i| i.is_folder)
                .map(|i| i.id.clone())
                .or_else(|| lib.nav_stack.last().map(|l| l.parent_id.clone()))
                .unwrap_or_else(|| lib.library.id.clone())
        };
        let client = self.client.lock().unwrap();
        match client.get_all_videos_recursive(&parent_id) {
            Ok(mut items) => {
                items.retain(|i| !i.is_folder);
                if items.is_empty() { drop(client); self.flash_status("Nothing to shuffle".into()); return; }
                items.shuffle(&mut rand::rng());
                let count = items.len();
                drop(client);
                self.player_tab.items = items.clone();
                self.player_tab.playlist_cursor = 0;
                self.tab_idx = 1;
                self.flash_status(format!("Shuffling {count} items"));
                self.play_items_routed(items, 0);
                self.queue_source = crate::config::QueueSource::Shuffle;
                self.save_queue_state();
            }
            Err(e) => { let msg = format!("Error: {e}"); drop(client); self.flash_status(msg); }
        }
    }

    pub(super) fn play_folder(&mut self, folder_id: &str) {
        let client = self.client.lock().unwrap();
        match client.get_all_playable_recursive(folder_id) {
            Ok(mut items) => {
                items.retain(|i| !i.is_folder);
                items.sort_by_key(|a| natural_sort_key(a.sort_key()));
                if items.is_empty() { drop(client); self.flash_status("Nothing to play".into()); return; }
                let count = items.len();
                drop(client);
                self.player_tab.items = items.clone();
                self.player_tab.playlist_cursor = 0;
                self.tab_idx = 1;
                self.flash_status(format!("Playing {count} items"));
                self.play_items_routed(items, 0);
            }
            Err(e) => { drop(client); self.flash_status(format!("Error: {e}")); }
        }
    }

    pub(super) fn shuffle_folder(&mut self, folder_id: &str) {
        let client = self.client.lock().unwrap();
        match client.get_all_playable_recursive(folder_id) {
            Ok(mut items) => {
                items.retain(|i| !i.is_folder);
                if items.is_empty() { drop(client); self.flash_status("Nothing to shuffle".into()); return; }
                items.shuffle(&mut rand::rng());
                let count = items.len();
                drop(client);
                self.player_tab.items = items.clone();
                self.player_tab.playlist_cursor = 0;
                self.tab_idx = 1;
                self.flash_status(format!("Shuffling {count} items"));
                self.play_items_routed(items, 0);
                self.queue_source = crate::config::QueueSource::Shuffle;
                self.save_queue_state();
            }
            Err(e) => { drop(client); self.flash_status(format!("Error: {e}")); }
        }
    }

    pub(super) fn set_tab(&mut self, idx: usize) {
        if idx != self.tab_idx && !self.card_image_states.is_empty() {
            self.force_clear = true;
        }
        self.tab_idx = idx;
        self.ensure_tab_visible();
        if self.tab_idx == 0 {
            self.home.section = 0;
            let _ = self.fetch_home();
        } else {
            self.ensure_library_loaded();
        }
    }

    pub(super) fn ensure_library_loaded(&mut self) {
        if self.tab_idx <= 1 || self.tab_idx == self.log_tab_idx() { return; }
        let idx = self.tab_idx - self.lib_tab_offset();
        self.ensure_lib_loaded_for(idx);
    }

    pub(super) fn ensure_lib_loaded_for(&mut self, idx: usize) {
        if idx >= self.libs.len() { return; }
        if self.libs[idx].nav_stack.is_empty() {
            let lib_id = self.libs[idx].library.id.clone();
            let lib_name = self.libs[idx].library.name.clone();
            let (item_types, unplayed_only, sort_by, sort_order) = match self.libs[idx].library.collection_type.as_str() {
                "movies"               => (Some("Movie".to_string()), false, "SortName", "Ascending"),
                "channels"|"homevideos" if lib_name == "Youtube" => (Some("Video".to_string()), true, "DateCreated", "Ascending"),
                _                      => (None, false, "SortName", "Ascending"),
            };
            self.libs[idx].nav_stack.push(BrowseLevel {
                parent_id: lib_id.clone(), title: lib_name.clone(),
                items: vec![], total_count: 0, cursor: 0,
                item_types: item_types.clone(), unplayed_only,
                sort_by: sort_by.into(), sort_order: sort_order.into(),
                loading: true, all_items: None,
            });
            self.spawn_browse(idx, lib_id, lib_name, item_types, unplayed_only, sort_by.into(), sort_order.into());
        }
    }

    pub(super) fn refresh_after_stop(&mut self) {
        let _ = self.fetch_home();
        let fetches: Vec<(usize, String, Option<String>, bool, String, String, usize)> = self.libs.iter().enumerate()
            .filter_map(|(i, lib)| lib.nav_stack.last().map(|lvl| {
                (i, lvl.parent_id.clone(), lvl.item_types.clone(), lvl.unplayed_only,
                 lvl.sort_by.clone(), lvl.sort_order.clone(), lvl.items.len())
            }))
            .collect();
        for (lib_idx, parent_id, item_types, unplayed_only, sort_by, sort_order, loaded_count) in fetches {
            self.spawn_refresh(lib_idx, parent_id, item_types, unplayed_only, sort_by, sort_order, loaded_count);
        }
    }

    pub(super) fn spawn_browse(&self, lib_idx: usize, parent_id: String, title: String,
                    item_types: Option<String>, unplayed_only: bool,
                    sort_by: String, sort_order: String) {
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            match client.get_items_sorted(&parent_id, item_types.as_deref(), unplayed_only, 0, PAGE_SIZE, &sort_by, &sort_order) {
                Ok((items, total_count)) => {
                    let _ = tx.send(LibEvent::Loaded {
                        lib_idx,
                        parent_id: parent_id.clone(),
                        level: BrowseLevel {
                            parent_id, title, items, total_count, cursor: 0,
                            item_types, unplayed_only,
                            sort_by, sort_order,
                            loading: false, all_items: None,
                        },
                    });
                }
                Err(e) => { let _ = tx.send(LibEvent::Error(e)); }
            }
        });
    }

    pub(super) fn spawn_navigate_to_item(&self, item_id: String, lib_ids: Vec<(usize, String)>) {
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let ancestors = match client.get_ancestors(&item_id) {
                Ok(a) => a,
                Err(e) => { let _ = tx.send(LibEvent::Error(e)); return; }
            };
            let lib_root_id = match ancestors.last() {
                Some(a) => a.id.clone(),
                None => { let _ = tx.send(LibEvent::Error("Item has no ancestors".into())); return; }
            };
            let lib_idx = match lib_ids.iter().find(|(_, id)| *id == lib_root_id) {
                Some(&(idx, _)) => idx,
                None => { let _ = tx.send(LibEvent::Error("Item not found in any library".into())); return; }
            };
            let chain: Vec<&crate::api::MediaItem> = ancestors.iter().rev().collect();
            let mut nav_stack: Vec<BrowseLevel> = Vec::new();
            for i in 0..chain.len() {
                let parent_id = chain[i].id.clone();
                let title = chain[i].name.clone();
                let target_id = if i + 1 < chain.len() { chain[i + 1].id.clone() } else { item_id.clone() };
                let (mut items, total_count) = match client.get_items_sorted(&parent_id, None, false, 0, 500, "SortName", "Ascending") {
                    Ok(x) => x,
                    Err(e) => { let _ = tx.send(LibEvent::Error(e)); return; }
                };
                if items.first().map(|it| it.item_type == "Episode").unwrap_or(false) {
                    sort_episodes(&mut items);
                }
                let cursor = items.iter().position(|it| it.id == target_id).unwrap_or(0);
                nav_stack.push(BrowseLevel {
                    parent_id, title, items, total_count, cursor,
                    item_types: None, unplayed_only: false,
                    sort_by: "SortName".into(), sort_order: "Ascending".into(),
                    loading: false, all_items: None,
                });
            }
            let _ = tx.send(LibEvent::NavigateTo { lib_idx, nav_stack });
        });
    }

    fn spawn_browse_page(&self, lib_idx: usize, parent_id: String, start_index: usize,
                         item_types: Option<String>, unplayed_only: bool,
                         sort_by: String, sort_order: String) {
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            match client.get_items_sorted(&parent_id, item_types.as_deref(), unplayed_only, start_index, PAGE_SIZE, &sort_by, &sort_order) {
                Ok((items, total_count)) => {
                    let _ = tx.send(LibEvent::PageAppended { lib_idx, parent_id, items, total_count });
                }
                Err(e) => { let _ = tx.send(LibEvent::Error(e)); }
            }
        });
    }

    fn spawn_all_items_prefetch(&self, lib_idx: usize) {
        let lib = &self.libs[lib_idx];
        let lvl = match lib.nav_stack.last() { Some(l) => l, None => return };
        if lvl.items.len() >= lvl.total_count { return; }
        let parent_id = lvl.parent_id.clone();
        let total_count = lvl.total_count;
        let item_types = lvl.item_types.clone();
        let unplayed_only = lvl.unplayed_only;
        let sort_by = lvl.sort_by.clone();
        let sort_order = lvl.sort_order.clone();
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            if let Ok((items, _)) = client.get_items_sorted(&parent_id, item_types.as_deref(), unplayed_only, 0, total_count, &sort_by, &sort_order) {
                let _ = tx.send(LibEvent::AllItemsPrefetched { lib_idx, parent_id, items });
            }
        });
    }

    pub(super) fn spawn_search_items_load(&self, lib_idx: usize) {
        let lib = &self.libs[lib_idx];
        let lvl = match lib.nav_stack.last() { Some(l) => l, None => return };
        let parent_id = lvl.parent_id.clone();
        let total_count = lvl.total_count;
        let item_types = lvl.item_types.clone();
        let unplayed_only = lvl.unplayed_only;
        let sort_by = lvl.sort_by.clone();
        let sort_order = lvl.sort_order.clone();
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            match client.get_items_sorted(&parent_id, item_types.as_deref(), unplayed_only, 0, total_count, &sort_by, &sort_order) {
                Ok((items, _)) => {
                    let _ = tx.send(LibEvent::SearchItemsLoaded { lib_idx, parent_id, items });
                }
                Err(_) => {}
            }
        });
    }

    fn spawn_refresh(&self, lib_idx: usize, parent_id: String,
                     item_types: Option<String>, unplayed_only: bool,
                     sort_by: String, sort_order: String, loaded_count: usize) {
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        let limit = loaded_count.max(PAGE_SIZE);
        std::thread::spawn(move || {
            match client.get_items_sorted(&parent_id, item_types.as_deref(), unplayed_only, 0, limit, &sort_by, &sort_order) {
                Ok((items, total_count)) => {
                    let _ = tx.send(LibEvent::Refreshed { lib_idx, parent_id, items, total_count });
                }
                Err(e) => { let _ = tx.send(LibEvent::Error(e)); }
            }
        });
    }

    fn maybe_fetch_next_page(&mut self, lib_idx: usize) {
        let lib = &self.libs[lib_idx];
        if lib.search.is_some() { return; }
        let lvl = match lib.nav_stack.last() { Some(l) => l, None => return };
        if lvl.loading { return; }
        if lvl.items.len() >= lvl.total_count { return; }
        if lvl.cursor + PREFETCH_AHEAD < lvl.items.len() { return; }
        let start_index = lvl.items.len();
        let parent_id = lvl.parent_id.clone();
        let item_types = lvl.item_types.clone();
        let unplayed_only = lvl.unplayed_only;
        let sort_by = lvl.sort_by.clone();
        let sort_order = lvl.sort_order.clone();
        if let Some(last) = self.libs[lib_idx].nav_stack.last_mut() { last.loading = true; }
        self.spawn_browse_page(lib_idx, parent_id, start_index, item_types, unplayed_only, sort_by, sort_order);
    }

    pub(super) fn spawn_sessions_load(&mut self) {
        self.sessions_loading = true;
        let client = self.client.lock().unwrap().clone();
        let tx = self.sessions_tx.clone();
        std::thread::spawn(move || {
            match client.get_sessions() {
                Ok(sessions) => { let _ = tx.send(SessionEvent::Loaded(sessions)); }
                Err(e)       => { let _ = tx.send(SessionEvent::Error(e)); }
            }
        });
    }

    pub(super) fn session_jump_track(&mut self, conn_id: &str, delta: i64, fallback_cmd: &'static str) {
        self.clear_playback_overlays();
        let id = conn_id.to_string();
        let current_remote_id = self.connected_session_state.as_ref()
            .and_then(|s| s.now_playing_item_id.as_deref())
            .map(str::to_string);
        let target = current_remote_id
            .and_then(|rid| self.player_tab.items.iter().position(|i| i.id == rid))
            .and_then(|idx| {
                let t = idx as i64 + delta;
                if t >= 0 && (t as usize) < self.player_tab.items.len() { Some(t as usize) } else { None }
            })
            .map(|t| (t, self.player_tab.items[t].playback_position_ticks));
        if let Some((target_idx, start_ticks)) = target {
            let item_ids: Vec<String> = self.player_tab.items.iter().map(|i| i.id.clone()).collect();
            self.do_session_command(move |c| c.session_play_items(&id, &item_ids, target_idx, start_ticks));
        } else {
            self.do_session_command(move |c| c.session_transport(&id, fallback_cmd));
        }
    }

    pub(super) fn clear_playback_overlays(&mut self) {
        self.skip_intro_end_ticks = None;
        self.next_up_item = None;
        self.status.clear();
    }

    pub(super) fn do_session_command(&self, f: impl FnOnce(&EmbyClient) -> Result<(), String> + Send + 'static) {
        let client = self.client.lock().unwrap().clone();
        let tx = self.sessions_tx.clone();
        std::thread::spawn(move || {
            if let Err(e) = f(&client) {
                let _ = tx.send(SessionEvent::Error(e));
                return;
            }
            match client.get_sessions() {
                Ok(sessions) => { let _ = tx.send(SessionEvent::Loaded(sessions)); }
                Err(e)       => { let _ = tx.send(SessionEvent::Error(e)); }
            }
        });
    }

    pub(super) fn handle_lib_event(&mut self, ev: LibEvent) {
        match ev {
            LibEvent::Loaded { lib_idx, parent_id, level } => {
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    if let Some(last) = lib.nav_stack.last_mut() {
                        if last.parent_id == parent_id && last.loading {
                            *last = level;
                        }
                    }
                }
                if self.is_album_level(lib_idx) {
                    let title = self.libs[lib_idx].nav_stack.last()
                        .map(|l| l.title.clone())
                        .unwrap_or_default();
                    log::debug!(target: "app", "album: entered «{title}»");
                    if let Some(last) = self.libs[lib_idx].nav_stack.last_mut() {
                        sort_audio_tracks(&mut last.items);
                    }
                }
                if let Some(last) = self.libs[lib_idx].nav_stack.last_mut() {
                    if last.items.first().map(|i| i.item_type == "Episode").unwrap_or(false) {
                        sort_episodes(&mut last.items);
                    }
                }
                self.maybe_fetch_next_page(lib_idx);
                self.spawn_all_items_prefetch(lib_idx);
            }
            LibEvent::PageAppended { lib_idx, parent_id, items, total_count } => {
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    if let Some(last) = lib.nav_stack.last_mut() {
                        if last.parent_id == parent_id && last.loading {
                            last.items.extend(items);
                            last.total_count = total_count;
                            last.loading = false;
                        }
                    }
                }
                if self.is_album_level(lib_idx) {
                    if let Some(last) = self.libs[lib_idx].nav_stack.last_mut() {
                        sort_audio_tracks(&mut last.items);
                    }
                }
                if let Some(last) = self.libs[lib_idx].nav_stack.last_mut() {
                    if last.items.first().map(|i| i.item_type == "Episode").unwrap_or(false) {
                        sort_episodes(&mut last.items);
                    }
                }
                self.maybe_fetch_next_page(lib_idx);
            }
            LibEvent::Refreshed { lib_idx, parent_id, items, total_count } => {
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    if let Some(last) = lib.nav_stack.last_mut() {
                        if last.parent_id == parent_id {
                            last.items = items;
                            last.total_count = total_count;
                            last.loading = false;
                        }
                    }
                }
                if self.is_album_level(lib_idx) {
                    if let Some(last) = self.libs[lib_idx].nav_stack.last_mut() {
                        sort_audio_tracks(&mut last.items);
                    }
                }
                if let Some(last) = self.libs[lib_idx].nav_stack.last_mut() {
                    if last.items.first().map(|i| i.item_type == "Episode").unwrap_or(false) {
                        sort_episodes(&mut last.items);
                    }
                }
                self.spawn_all_items_prefetch(lib_idx);
            }
            LibEvent::SearchItemsLoaded { lib_idx, parent_id, items } => {
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    let current_parent = lib.nav_stack.last().map(|l| l.parent_id.as_str());
                    if current_parent == Some(&parent_id) {
                        if let Some(s) = lib.search.as_mut() {
                            s.items = items;
                            s.loading = false;
                        }
                    }
                }
                self.update_lib_search(lib_idx);
            }
            LibEvent::AllItemsPrefetched { lib_idx, parent_id, items } => {
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    if let Some(last) = lib.nav_stack.last_mut() {
                        if last.parent_id == parent_id {
                            last.all_items = Some(items);
                        }
                    }
                }
            }
            LibEvent::AlbumYearFetched { album_id, year } => {
                self.album_year_loading.remove(&album_id);
                self.album_year_cache.insert(album_id, year);
            }
            LibEvent::NavigateTo { lib_idx, nav_stack } => {
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    lib.nav_stack = nav_stack;
                    lib.search = None;
                }
                let target_tab = lib_idx + self.lib_tab_offset();
                self.set_tab(target_tab);
            }
            LibEvent::PlaylistsLoaded(items) => {
                self.playlists = items;
                self.playlists_loading = false;
                self.playlists_cursor = self.playlists_cursor.min(self.playlists.len().saturating_sub(1));
            }
            LibEvent::PlaylistItemsLoaded { playlist_id, items } => {
                if self.playlists_open.as_ref().map(|p| p.id == playlist_id).unwrap_or(false) {
                    self.playlists_open_items = items;
                    self.playlists_open_loading = false;
                }
            }
            LibEvent::QueueRestored { items, source, last_played_item_id, last_played_completed } => {
                if items.is_empty() {
                    crate::config::clear_queue_state();
                    return;
                }
                let cursor = if let Some(ref id) = last_played_item_id {
                    let idx = items.iter().position(|i| &i.id == id).unwrap_or(0);
                    if last_played_completed {
                        (idx + 1).min(items.len().saturating_sub(1))
                    } else {
                        idx
                    }
                } else {
                    0
                };
                self.last_played_item_id = last_played_item_id;
                self.last_played_completed = last_played_completed;
                self.player_tab.items = items;
                self.player_tab.playlist_cursor = cursor;
                self.queue_source = source;
                self.queue_restored = true;
                self.queue_dirty = false;
                if self.client.lock().unwrap().config.start_on_queue {
                    self.tab_idx = 1;
                }
            }
            LibEvent::Error(e) => {
                self.flash_status(format!("Error: {e}"));
            }
        }
    }

    pub(super) fn try_quit(&mut self) -> bool {
        if self.queue_dirty && self.queue_is_saved_playlist() {
            self.replace_queue_or_prompt(PendingQueueAction::Quit);
            false
        } else {
            if !self.player.is_remote() { self.player.stop(); }
            true
        }
    }

    pub(super) fn on_queue_replace_silent(&mut self) {
        self.queue_source = crate::config::QueueSource::Unknown;
        self.queue_restored = false;
        self.queue_dirty = false;
    }

    pub(super) fn replace_queue_or_prompt(&mut self, action: PendingQueueAction) {
        if self.queue_dirty && self.queue_is_saved_playlist() {
            self.pending_queue_action = Some(action);
            self.show_save_playlist_modal = true;
        } else {
            self.execute_pending_queue_action(action);
        }
    }

    pub(super) fn execute_pending_queue_action(&mut self, action: PendingQueueAction) {
        self.queue_dirty = false;
        match action {
            PendingQueueAction::PlayItems { items, start_idx, source } => {
                self.queue_source = source;
                self.queue_restored = false;
                self.player_tab.items = items.clone();
                self.player_tab.playlist_cursor = start_idx;
                let c = Arc::new(self.client.lock().unwrap().clone());
                self.player.play_playlist(items, start_idx, c, self.ui_volume);
                self.save_queue_state();
            }
            PendingQueueAction::ClearQueue => {
                self.queue_source = crate::config::QueueSource::Unknown;
                self.queue_restored = false;
                self.player.stop();
                self.player_tab.items.clear();
                self.player_tab.playlist_cursor = 0;
                self.playlist_undo_stack.clear();
                self.save_queue_state();
            }
            PendingQueueAction::Quit => {
                if !self.player.is_remote() { self.player.stop(); }
            }
        }
    }

    pub(super) fn queue_is_saved_playlist(&self) -> bool {
        matches!(&self.queue_source, crate::config::QueueSource::Playlist { id: Some(_), .. })
    }

    fn queue_playlist_id(&self) -> Option<&str> {
        if let crate::config::QueueSource::Playlist { id: Some(ref id), .. } = self.queue_source {
            Some(id.as_str())
        } else {
            None
        }
    }

    pub(super) fn queue_playlist_name(&self) -> &str {
        if let crate::config::QueueSource::Playlist { ref name, .. } = self.queue_source {
            name.as_str()
        } else {
            ""
        }
    }

    pub(super) fn save_playlist_to_emby(&self) {
        let Some(playlist_id) = self.queue_playlist_id() else { return };
        let item_ids: Vec<String> = self.player_tab.items.iter().map(|i| i.id.clone()).collect();
        let client = self.client.lock().unwrap().clone();
        let playlist_id = playlist_id.to_string();
        std::thread::spawn(move || {
            if let Err(e) = client.update_playlist_items(&playlist_id, &item_ids) {
                log::error!(target: "playlist", "Failed to save playlist: {e}");
            }
        });
    }

    fn delete_playlist_on_emby(&mut self) {
        let Some(playlist_id) = self.queue_playlist_id() else { return };
        let name = self.queue_playlist_name().to_string();
        let client = self.client.lock().unwrap().clone();
        let playlist_id = playlist_id.to_string();
        log::info!(target: "playlist", "consume: deleting fully-consumed playlist id={playlist_id} name={name:?}");
        self.queue_source = crate::config::QueueSource::Unknown;
        std::thread::spawn(move || {
            if let Err(e) = client.delete_playlist(&playlist_id) {
                log::error!(target: "playlist", "Failed to delete playlist id={playlist_id}: {e}");
            } else {
                log::info!(target: "playlist", "Deleted playlist id={playlist_id} name={name:?}");
            }
        });
    }

    pub(super) fn consume_item(&mut self, idx: usize) {
        let is_video = self.player_tab.items.get(idx).map_or(false, |i| i.is_video());
        if idx < self.player_tab.items.len() {
            self.player_tab.items.remove(idx);
        }
        self.player.send_command(crate::player::PlayerCommand::PlaylistRemove(idx));
        self.queue_dirty = true;

        if is_video
            && self.client.lock().unwrap().config.save_playlist_on_consume
            && self.queue_is_saved_playlist()
        {
            if self.player_tab.items.is_empty() {
                self.delete_playlist_on_emby();
            } else {
                log::info!(target: "playlist", "consume: saving playlist after removing idx={idx}");
                self.save_playlist_to_emby();
            }
            self.queue_dirty = false;
        }
    }

    pub(super) fn power_focused_lib_idx(&self) -> Option<usize> {
        match self.power_focus {
            PowerFocus::Library(idx) => Some(idx),
            PowerFocus::Queue => None,
        }
    }

    pub(super) fn power_focus_next(&mut self) {
        let n = self.libs.len();
        self.power_focus = match self.power_focus {
            PowerFocus::Queue => if n > 0 { PowerFocus::Library(0) } else { PowerFocus::Queue },
            PowerFocus::Library(idx) => {
                if idx + 1 < n { PowerFocus::Library(idx + 1) } else { PowerFocus::Queue }
            }
        };
        self.power_ensure_focused_visible();
    }

    pub(super) fn power_focus_prev(&mut self) {
        let n = self.libs.len();
        self.power_focus = match self.power_focus {
            PowerFocus::Queue => if n > 0 { PowerFocus::Library(n - 1) } else { PowerFocus::Queue },
            PowerFocus::Library(0) => PowerFocus::Queue,
            PowerFocus::Library(idx) => PowerFocus::Library(idx - 1),
        };
        self.power_ensure_focused_visible();
    }

    fn power_ensure_focused_visible(&mut self) {
        let PowerFocus::Library(lib_idx) = self.power_focus else { return };
        let n_cols = self.power_lib_col_areas.len().max(2);
        if lib_idx < self.power_lib_col_scroll {
            self.power_lib_col_scroll = lib_idx;
        } else if lib_idx >= self.power_lib_col_scroll + n_cols {
            self.power_lib_col_scroll = lib_idx + 1 - n_cols;
        }
    }

    pub(super) fn save_queue_state(&self) {
        let state = crate::config::QueueState {
            source: self.queue_source.clone(),
            item_ids: self.player_tab.items.iter().map(|i| i.id.clone()).collect(),
            cursor: self.player_tab.playlist_cursor,
            last_played_item_id: self.last_played_item_id.clone(),
            last_played_completed: self.last_played_completed,
        };
        if state.item_ids.is_empty() {
            crate::config::clear_queue_state();
        } else {
            crate::config::save_queue_state(&state);
        }
    }

    pub(super) fn spawn_restore_queue_state(&mut self) {
        let Some(state) = crate::config::load_queue_state() else { return };
        if state.item_ids.is_empty() { return; }
        let (item_ids, source, last_played_item_id, last_played_completed) =
            (state.item_ids, state.source, state.last_played_item_id, state.last_played_completed);
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let items = client.get_items_by_ids(&item_ids).unwrap_or_default();
            let _ = tx.send(LibEvent::QueueRestored { items, source, last_played_item_id, last_played_completed });
        });
    }

    pub(super) fn spawn_load_playlists(&mut self) {
        if self.playlists_loading { return; }
        self.playlists_loading = true;
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let items = client.get_playlists().unwrap_or_default();
            let _ = tx.send(LibEvent::PlaylistsLoaded(items));
        });
    }

    pub(super) fn spawn_open_playlist(&mut self, playlist: MediaItem) {
        if self.playlists_open_loading { return; }
        self.playlists_open_loading = true;
        self.playlists_open = Some(playlist.clone());
        self.playlists_open_items = Vec::new();
        self.playlists_open_cursor = 0;
        self.playlists_open_scroll = 0;
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        let playlist_id = playlist.id.clone();
        std::thread::spawn(move || {
            let items = client.get_items_sorted(&playlist_id, None, false, 0, 5000, "IndexNumber", "Ascending")
                .map(|(v, _)| v)
                .unwrap_or_default();
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
        let playlist_name = self.playlists.iter().find(|p| p.id == playlist_id)
            .map(|p| p.name.clone()).unwrap_or_default();
        let client = self.client.lock().unwrap().clone();
        let (items, _) = match client.get_items_sorted(&playlist_id, None, false, 0, 5000, "IndexNumber", "Ascending") {
            Ok(r) => r,
            Err(e) => { self.flash_status(format!("Playlist load failed: {e}")); return; }
        };
        if items.is_empty() { self.flash_status("Playlist is empty".into()); return; }
        let playable: Vec<MediaItem> = items.into_iter().filter(|i| !i.is_folder).collect();
        if playable.is_empty() { self.flash_status("No playable items in playlist".into()); return; }
        let action = PendingQueueAction::PlayItems {
            items: playable, start_idx: 0,
            source: crate::config::QueueSource::Playlist { id: Some(playlist_id), name: playlist_name },
        };
        self.replace_queue_or_prompt(action);
        if !self.show_save_playlist_modal {
            self.show_playlists = false;
            self.set_tab(1);
        }
    }

    pub(super) fn fetch_home(&mut self) -> Result<(), String> {
        let client = self.client.lock().unwrap();

        self.home.continue_items = client.get_continue_watching(10).unwrap_or_default();

        let all_views = client.get_views()?;
        let old_libs: HashMap<String, Vec<BrowseLevel>> = self.libs.drain(..)
            .map(|mut l| (l.library.id.clone(), std::mem::take(&mut l.nav_stack)))
            .collect();

        for view in all_views.iter().filter(|v| v.collection_type != "playlists" && !self.hidden_libraries.contains(&v.name.to_lowercase())) {
            let stack = old_libs.get(&view.id)
                .map(|s| s.iter().map(|lvl| BrowseLevel {
                    parent_id: lvl.parent_id.clone(), title: lvl.title.clone(),
                    items: lvl.items.clone(), total_count: lvl.total_count, cursor: lvl.cursor,
                    item_types: lvl.item_types.clone(), unplayed_only: lvl.unplayed_only,
                    sort_by: lvl.sort_by.clone(), sort_order: lvl.sort_order.clone(),
                    loading: false, all_items: lvl.all_items.clone(),
                }).collect())
                .unwrap_or_default();
            self.libs.push(super::LibraryTab { library: view.clone(), nav_stack: stack, search: None });
        }
        let n = self.libs.len();
        self.layout_lib_scroll.resize(n, 0);
        self.layout_lib_row_heights.resize_with(n, Vec::new);
        self.layout_lib_table_area.resize(n, ratatui::layout::Rect::default());

        let old_cursors: HashMap<String, usize> = self.home.latest.iter()
            .map(|(_, lib_id, _, cur)| (lib_id.clone(), *cur))
            .collect();

        let user_views = client.get_user_views().unwrap_or_default();
        let mut latest: Vec<(String, String, Vec<MediaItem>, usize)> = Vec::new();
        for v in user_views.iter().filter(|v| {
            let lower = v.name.to_lowercase();
            v.collection_type != "playlists"
                && !self.hidden_latest.contains(&lower)
                && !self.hidden_libraries.contains(&lower)
        }) {
            let title = format!("Latest {}", v.name);
            let items = if v.collection_type == "tvshows" {
                client.get_latest_episodes(&v.id, 15).unwrap_or_default()
            } else {
                client.get_latest(&v.id, 15).unwrap_or_default()
            };
            let cursor = old_cursors.get(&v.id).copied().unwrap_or(0)
                .min(items.len().saturating_sub(1));
            latest.push((title, v.id.clone(), items, cursor));
        }
        drop(client);
        self.home.latest = latest;

        let n = 1 + self.home.latest.len();
        if self.home.section >= n {
            self.home.section = n.saturating_sub(1);
        }
        self.ensure_home_section_visible();
        Ok(())
    }

    pub(super) fn handle_ws_event(&mut self, ev: WsEvent) {
        match ev {
            WsEvent::Play { item_ids, play_now, start_position_ticks, start_index } => {
                log::info!(target: "ws", "Play: {} id(s), play_now={play_now}", item_ids.len());
                if !play_now { return; }
                self.on_queue_replace_silent();
                let items = {
                    let c = self.client.lock().unwrap();
                    match c.get_items_by_ids(&item_ids) {
                        Ok(v) => v,
                        Err(e) => { let msg = format!("WS play error: {e}"); drop(c); self.flash_status(msg); return; }
                    }
                };
                if items.is_empty() {
                    log::warn!(target: "ws", "Play: no items found for ids={}", item_ids.join(","));
                    return;
                }
                let start_idx = start_index.min(items.len().saturating_sub(1));
                self.tab_idx = 1;
                if items.len() == 1 {
                    let mut item = items[0].clone();
                    if start_position_ticks > 0 { item.playback_position_ticks = start_position_ticks; }
                    self.player_tab.items = vec![item.clone()];
                    self.player_tab.playlist_cursor = 0;
                    self.flash_status(item.playback_label());
                    let c = Arc::new(self.client.lock().unwrap().clone());
                    self.player.play(&item, c, self.ui_volume);
                } else {
                    let count = items.len();
                    self.player_tab.items = items.clone();
                    self.player_tab.playlist_cursor = start_idx;
                    self.flash_status(format!("Playing {count} items"));
                    let c = Arc::new(self.client.lock().unwrap().clone());
                    let active = self.player.status.lock().unwrap().active;
                    log::info!(target: "ws", "Play multi: active={active}, count={count}, start_idx={start_idx}");
                    if active {
                        let mut start_item = items[start_idx].clone();
                        if start_position_ticks > 0 { start_item.playback_position_ticks = start_position_ticks; }
                        self.player.play(&start_item, c, self.ui_volume);
                    } else {
                        let mut items_with_pos = items.clone();
                        if start_position_ticks > 0 {
                            items_with_pos[start_idx].playback_position_ticks = start_position_ticks;
                        }
                        self.player.play_playlist(items_with_pos, start_idx, c, self.ui_volume);
                    }
                }
                self.queue_source = crate::config::QueueSource::Remote;
                self.save_queue_state();
            }
            WsEvent::Stop => { self.player.stop(); }
            WsEvent::Pause => {
                if !self.player.status.lock().unwrap().paused {
                    self.player.send_command(PlayerCommand::TogglePause);
                }
            }
            WsEvent::Unpause => {
                if self.player.status.lock().unwrap().paused {
                    self.player.send_command(PlayerCommand::TogglePause);
                }
            }
            WsEvent::NextTrack => {
                let idx = self.player.status.lock().unwrap().current_idx;
                if idx + 1 < self.player_tab.items.len() {
                    self.player.send_command(PlayerCommand::JumpTo(idx + 1));
                }
            }
            WsEvent::PreviousTrack => {
                let idx = self.player.status.lock().unwrap().current_idx;
                if idx > 0 { self.player.send_command(PlayerCommand::JumpTo(idx - 1)); }
            }
            WsEvent::TogglePause => {
                self.player.send_command(PlayerCommand::TogglePause);
            }
            WsEvent::Seek(ticks) => {
                self.player.send_command(PlayerCommand::SeekAbsolute(
                    ticks as f64 / TICKS_PER_SECOND as f64,
                ));
            }
            WsEvent::SeekRelative(secs) => {
                self.player.send_command(PlayerCommand::Seek(secs));
            }
            WsEvent::SetVolume(v) => {
                let vol_max = self.player.status.lock().unwrap().volume_max;
                self.player.send_command(PlayerCommand::SetVolume(v.clamp(0, vol_max)));
            }
            WsEvent::VolumeUp => {
                let st = self.player.status.lock().unwrap();
                let v = (st.volume + 5).min(st.volume_max);
                drop(st);
                self.player.send_command(PlayerCommand::SetVolume(v));
            }
            WsEvent::VolumeDown => {
                let v = self.player.status.lock().unwrap().volume.saturating_sub(5);
                self.player.send_command(PlayerCommand::SetVolume(v));
            }
            WsEvent::UserDataChanged => { let _ = self.fetch_home(); }
        }
    }

    pub(super) fn settings_scroll_follow(&mut self) {
        let cursor = self.settings_cursor;
        let Some(&cursor_line) = self.settings_line_of_cursor.get(cursor) else { return };
        let visible = self.terminal_height.saturating_sub(4) as usize;
        if cursor_line < self.settings_scroll {
            self.settings_scroll = cursor_line;
        } else if cursor_line >= self.settings_scroll + visible {
            self.settings_scroll = cursor_line + 1 - visible;
        }
    }

    pub(super) fn update_lib_search(&mut self, lib_idx: usize) {
        use fuzzy_matcher::FuzzyMatcher;
        use fuzzy_matcher::skim::SkimMatcherV2;

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

        let scored: Vec<(i64, usize)> = {
            let items = self.libs[lib_idx].search.as_ref()
                .map(|s| s.items.as_slice())
                .unwrap_or(&[]);
            let matcher = SkimMatcherV2::default();
            items.iter().enumerate()
                .filter_map(|(i, item)| matcher.fuzzy_match(&item.display_name(), &query).map(|s| (s, i)))
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
