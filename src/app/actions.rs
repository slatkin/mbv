use super::ui_util::{is_playable, natural_sort_key, sort_audio_tracks, sort_episodes, take_chars};
use super::{
    AlbumIndexState, App, ArtistHeaderSelection, BrowseLevel, FeedHomeVideoState, LibEvent,
    LocalPlaybackTarget, PanelFocus, PendingQueueAction, PlaybackTarget, QueueScope,
    RemotePlaybackTarget, SessionEvent,
};
use crate::app::images::NAV_IMAGE_FETCH_IDLE_DELAY;
use crate::app::render::indicators::IndicatorData;
use mbv_core::api::{EmbyClient, MediaItem, TICKS_PER_SECOND};
use mbv_core::player::PlayerCommand;
use mbv_core::ws::WsEvent;
use rand::seq::SliceRandom;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

// #286: `App::ring_terminal_bell()` writes to a thread-local buffer instead
// of real stderr in test builds, so tests never touch the process-wide
// STDERR_FILENO fd. The test that verifies the bell rings used to redirect
// that fd directly via `libc::dup2`, which raced against *any other test*
// ringing the bell concurrently on a different thread (e.g. one that calls
// `flash_status`/`flash_status_high`, both of which also ring the bell) and
// produced flaky doubled "\x07\x07" captures. `cargo test` runs each test
// on its own OS thread, so a thread-local is naturally isolated per test
// with no locking required -- this removes the race at its root instead of
// serializing around it.
#[cfg(test)]
thread_local! {
    static TEST_BELL_LOG: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn enqueue_action_context(item_id: &str, item_name: &str, source: &str, bypass: bool) -> String {
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
    items: &[MediaItem],
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

impl PlaybackTarget {
    pub(super) fn toggle_play_pause(&self, app: &mut App) {
        match self {
            Self::Local(target) => target.toggle_play_pause(app),
            Self::Remote(target) => target.toggle_play_pause(app),
        }
    }

    pub(super) fn stop(&self, app: &mut App) {
        match self {
            Self::Local(target) => target.stop(app),
            Self::Remote(target) => target.stop(app),
        }
    }

    pub(super) fn seek_relative(&self, app: &mut App, delta: f64) {
        match self {
            Self::Local(target) => target.seek_relative(app, delta),
            Self::Remote(target) => target.seek_relative(app, delta),
        }
    }

    pub(super) fn jump_track(&self, app: &mut App, step: i64, transport: &'static str) {
        match self {
            Self::Local(target) => target.jump_track(app, step),
            Self::Remote(target) => target.jump_track(app, step, transport),
        }
    }

    pub(super) fn toggle_command_mute(&self, app: &mut App) {
        match self {
            Self::Local(target) => target.toggle_command_mute(app),
            Self::Remote(target) => target.toggle_command_mute(app),
        }
    }

    pub(super) fn is_audio_item(&self, app: &App) -> bool {
        match self {
            Self::Local(target) => target.is_audio_item(app),
            Self::Remote(target) => target.is_audio_item(app),
        }
    }

    pub(super) fn toggle_soft_mute(&self, app: &mut App) {
        match self {
            Self::Local(target) => target.toggle_soft_mute(app),
            Self::Remote(target) => target.toggle_soft_mute(app),
        }
    }

    pub(super) fn cycle_audio(&self, app: &mut App) {
        match self {
            Self::Local(target) => target.cycle_audio(app),
            Self::Remote(target) => target.cycle_audio(app),
        }
    }

    pub(super) fn adjust_volume(&self, app: &mut App, delta: i64) {
        match self {
            Self::Local(target) => target.adjust_volume(app, delta),
            Self::Remote(target) => target.adjust_volume(app, delta),
        }
    }

    pub(super) fn cycle_sub(&self, app: &mut App) {
        match self {
            Self::Local(target) => target.cycle_sub(app),
            Self::Remote(target) => target.cycle_sub(app),
        }
    }

    pub(super) fn displayed_volume(&self, app: &App) -> i64 {
        match self {
            Self::Local(target) => target.displayed_volume(app),
            Self::Remote(target) => target.displayed_volume(app),
        }
    }

    pub(super) fn displayed_mute(&self, app: &App) -> bool {
        match self {
            Self::Local(target) => target.displayed_mute(app),
            Self::Remote(target) => target.displayed_mute(app),
        }
    }

    pub(super) fn indicator_data(&self, app: &App) -> Option<IndicatorData> {
        match self {
            Self::Local(target) => target.indicator_data(app),
            Self::Remote(target) => target.indicator_data(app),
        }
    }
}

impl LocalPlaybackTarget {
    fn toggle_play_pause(&self, app: &mut App) {
        app.player.send_command(PlayerCommand::TogglePause);
    }

    fn stop(&self, app: &mut App) {
        app.player.stop();
    }

    fn seek_relative(&self, app: &mut App, delta: f64) {
        app.player.send_command(PlayerCommand::Seek(delta));
    }

    fn jump_track(&self, app: &mut App, step: i64) {
        if step >= 0 {
            app.player.next();
        } else {
            app.player.previous();
        }
    }

    fn toggle_command_mute(&self, app: &mut App) {
        app.mute_on = !app.mute_on;
        app.player.send_command(PlayerCommand::SetMute(app.mute_on));
        app.save_prefs();
    }

    fn is_audio_item(&self, app: &App) -> bool {
        let idx = app.player_tab.queue_cursor;
        app.player_tab
            .items
            .get(idx)
            .map(|i| i.media_type == "Audio" || i.item_type == "Audio")
            .unwrap_or(false)
    }

    fn toggle_soft_mute(&self, app: &mut App) {
        if app.ui_volume == 0 {
            if let Some(v) = app.pre_mute_volume.take() {
                app.player.send_command(PlayerCommand::SetVolume(v as i64));
                app.ui_volume = v;
            }
        } else {
            app.pre_mute_volume = Some(app.ui_volume);
            app.player.send_command(PlayerCommand::SetVolume(0));
            app.ui_volume = 0;
        }
        app.save_prefs();
    }

    fn cycle_audio(&self, app: &mut App) {
        let (tracks, current_id) = {
            let s = app.player.status.lock().unwrap();
            (s.audio_tracks.clone(), s.audio_id)
        };
        if tracks.is_empty() {
            return;
        }
        let mut entries: Vec<i64> = vec![0];
        entries.extend(tracks.iter().map(|(id, _)| *id));
        let cur = entries.iter().position(|&id| id == current_id).unwrap_or(0);
        let next = (cur + 1) % entries.len();
        let next_id = entries[next];
        if next_id == 0 {
            app.pre_mute_volume = Some(app.ui_volume);
            app.player.send_command(PlayerCommand::SetVolume(0));
            app.ui_volume = 0;
        } else if current_id == 0 {
            if let Some(v) = app.pre_mute_volume.take() {
                app.player.send_command(PlayerCommand::SetVolume(v as i64));
                app.ui_volume = v;
            }
        }
        app.player.send_command(PlayerCommand::SetAudio(next_id));
    }

    fn adjust_volume(&self, app: &mut App, delta: i64) {
        let active = app.player.status.lock().unwrap().active;
        if active {
            let st = app.player.status.lock().unwrap();
            let v = (st.volume + delta).clamp(0, st.volume_max) as u8;
            drop(st);
            app.player.send_command(PlayerCommand::SetVolume(v as i64));
            app.ui_volume = v;
        } else {
            app.ui_volume = (app.ui_volume as i64 + delta).clamp(0, 200) as u8;
        }
        app.save_prefs();
    }

    fn cycle_sub(&self, app: &mut App) {
        let (active, tracks, current_id) = {
            let s = app.player.status.lock().unwrap();
            (s.active, s.sub_tracks.clone(), s.sub_id)
        };
        if !active {
            app.cycle_subtitle_mode();
            return;
        }
        if tracks.is_empty() {
            return;
        }
        let mut entries: Vec<i64> = vec![0];
        entries.extend(tracks.iter().map(|(id, _, _)| *id));
        let next_id = App::next_subtitle_entry(&entries, current_id);
        app.player.send_command(PlayerCommand::SetSub(next_id));
        app.save_prefs();
    }

    fn displayed_volume(&self, app: &App) -> i64 {
        let s = app.player.status.lock().unwrap();
        if s.active {
            if s.muted {
                0
            } else {
                s.volume
            }
        } else {
            app.ui_volume as i64
        }
    }

    fn displayed_mute(&self, app: &App) -> bool {
        app.mute_on
    }

    fn indicator_data(&self, app: &App) -> Option<IndicatorData> {
        let pst = app.player.status.lock().unwrap();
        if !pst.active {
            return None;
        }
        let video_is_image = pst.video_is_image;
        let res_h = pst.video_height;
        let is_audio_only = video_is_image;
        let res_str = if video_is_image || res_h == 0 {
            if pst.audio_codec.is_empty() {
                "--".to_string()
            } else {
                pst.audio_codec.to_uppercase()
            }
        } else {
            format!("{}p", res_h)
        };
        let res_dim = res_str == "--";
        let raw_lang = pst.audio_lang.to_lowercase();
        let (audio_label, audio_dim): (String, bool) = if raw_lang.is_empty() {
            ("x".into(), true)
        } else {
            (take_chars(&raw_lang, 2), false)
        };
        let sub_id = pst.sub_id;
        let raw_sub_lang = pst.sub_lang.to_lowercase();
        drop(pst);
        let sub_label = if sub_id == 0 {
            "off".into()
        } else if !raw_sub_lang.is_empty() {
            take_chars(&raw_sub_lang, 3)
        } else {
            "CC".into()
        };
        Some(IndicatorData {
            res_label: res_str,
            res_dim,
            audio_label,
            audio_dim,
            audio_only: is_audio_only,
            sub_label,
        })
    }
}

impl RemotePlaybackTarget {
    fn toggle_play_pause(&self, app: &mut App) {
        let session_id = self.session_id.clone();
        app.do_session_command(move |c| c.session_transport(&session_id, "PlayPause"));
    }

    fn stop(&self, app: &mut App) {
        let session_id = self.session_id.clone();
        app.do_session_command(move |c| c.session_transport(&session_id, "Stop"));
    }

    fn seek_relative(&self, app: &mut App, delta: f64) {
        let pos_s = app
            .connected_session_state
            .as_ref()
            .map(|s| s.position_s)
            .unwrap_or(0);
        let target = App::remote_seek_ticks(pos_s, delta);
        let session_id = self.session_id.clone();
        app.do_session_command(move |c| c.session_seek(&session_id, target));
    }

    fn jump_track(&self, app: &mut App, step: i64, transport: &'static str) {
        app.session_jump_track(&self.session_id, step, transport);
    }

    fn toggle_command_mute(&self, app: &mut App) {
        app.session_toggle_mute();
    }

    fn is_audio_item(&self, app: &App) -> bool {
        app.connected_session_state
            .as_ref()
            .map(|s| s.media_info.audio_only)
            .unwrap_or(false)
    }

    fn toggle_soft_mute(&self, app: &mut App) {
        // No session-level mute primitive exists for `a`, so keep routing the
        // remote path through the audio-track cycle behavior.
        self.cycle_audio(app);
    }

    fn cycle_audio(&self, app: &mut App) {
        let remote_indexes = app.remote_audio_indexes();
        let cur = app
            .connected_session_state
            .as_ref()
            .map(|s| s.audio_index)
            .unwrap_or(1);
        let next = if remote_indexes.is_empty() {
            if cur <= 1 {
                2
            } else {
                1
            }
        } else {
            let cur_pos = remote_indexes
                .iter()
                .position(|&idx| idx == cur)
                .unwrap_or(0);
            remote_indexes[(cur_pos + 1) % remote_indexes.len()]
        };
        if let Some(ref mut state) = app.connected_session_state {
            state.audio_index = next;
        }
        let session_id = self.session_id.clone();
        app.do_session_command(move |c| c.session_set_audio_index(&session_id, next));
    }

    fn adjust_volume(&self, app: &mut App, delta: i64) {
        let vol = app
            .connected_session_state
            .as_ref()
            .map(|s| s.volume)
            .unwrap_or(50);
        let new_vol = (vol + delta).clamp(0, 100);
        let session_id = self.session_id.clone();
        app.do_session_command(move |c| c.session_set_volume(&session_id, new_vol));
    }

    fn cycle_sub(&self, app: &mut App) {
        let remote_indexes = app.remote_subtitle_indexes();
        if remote_indexes.is_empty() {
            app.toggle_sub();
            return;
        }
        let current = app
            .connected_session_state
            .as_ref()
            .map(|s| s.sub_index)
            .unwrap_or(-1);
        let mut entries = Vec::with_capacity(remote_indexes.len() + 1);
        entries.push(-1);
        entries.extend(remote_indexes);
        let next = App::next_subtitle_entry(&entries, current);
        if let Some(ref mut state) = app.connected_session_state {
            state.sub_index = next;
        }
        let session_id = self.session_id.clone();
        app.do_session_command(move |c| c.session_set_subtitle_index(&session_id, next));
    }

    fn displayed_volume(&self, app: &App) -> i64 {
        app.connected_session_state
            .as_ref()
            .map(|s| s.volume)
            .unwrap_or_else(|| LocalPlaybackTarget.displayed_volume(app))
    }

    fn displayed_mute(&self, app: &App) -> bool {
        app.connected_session_state
            .as_ref()
            .map(|s| s.muted)
            .unwrap_or_else(|| LocalPlaybackTarget.displayed_mute(app))
    }

    fn indicator_data(&self, app: &App) -> Option<IndicatorData> {
        let remote = app.connected_session_state.as_ref()?;
        let audio_label = remote
            .media_info
            .audio_streams
            .iter()
            .find(|stream| stream.index == remote.audio_index)
            .map(|stream| {
                if !stream.language.is_empty() {
                    take_chars(&stream.language.to_lowercase(), 2)
                } else {
                    take_chars(&stream.label.to_lowercase(), 2)
                }
            })
            .unwrap_or_else(|| "---".to_string());
        let sub_label = if remote.sub_index < 0 {
            "off".to_string()
        } else {
            remote
                .media_info
                .subtitle_streams
                .iter()
                .find(|stream| stream.index == remote.sub_index)
                .map(|stream| {
                    if !stream.language.is_empty() {
                        take_chars(&stream.language.to_lowercase(), 3)
                    } else {
                        take_chars(&stream.label.to_lowercase(), 3)
                    }
                })
                .unwrap_or_else(|| "CC".to_string())
        };
        let res_label = if remote.media_info.video_label.is_empty() {
            "---".to_string()
        } else if remote.media_info.audio_only {
            remote
                .media_info
                .video_label
                .split("  |  ")
                .next()
                .unwrap_or(&remote.media_info.video_label)
                .to_string()
        } else {
            remote
                .media_info
                .video_label
                .split_whitespace()
                .next()
                .unwrap_or(&remote.media_info.video_label)
                .to_string()
        };
        Some(IndicatorData {
            res_label: res_label.clone(),
            res_dim: res_label == "---",
            audio_label: audio_label.clone(),
            audio_dim: audio_label == "---",
            audio_only: remote.media_info.audio_only,
            sub_label,
        })
    }
}

impl App {
    fn remote_audio_indexes(&self) -> Vec<i64> {
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

    fn remote_subtitle_indexes(&self) -> Vec<i64> {
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

    pub(super) fn move_lib_cursor(&mut self, delta: i64) {
        let now = Instant::now();
        let idle = now.duration_since(self.last_nav_at) >= NAV_IMAGE_FETCH_IDLE_DELAY;
        self.last_nav_at = now;
        self.mark_power_library_navigation(now);
        let lib_idx = self.library_tab.saturating_sub(1);

        if matches!(self.panel_focus, PanelFocus::Library)
            && self.libs[lib_idx].search.is_none()
            && self.libs[lib_idx].album_track_focus.is_none()
            && self.move_power_music_group_display_cursor(lib_idx, delta)
        {
            self.save_default_library_position(lib_idx);
            if idle {
                self.maybe_fetch_next_page(lib_idx);
            }
            return;
        }

        if self.libs[lib_idx].search.is_none() && self.is_feed_home_video_group_view(lib_idx) {
            if let Some(state) = self.libs[lib_idx].feed_home_video.as_mut() {
                let n = state.selected_len();
                if n > 0 {
                    state.video_cursor =
                        (state.video_cursor as i64 + delta).clamp(0, n as i64 - 1) as usize;
                    self.save_default_library_position(lib_idx);
                }
            }
            return;
        }

        // With letter-grouped display, navigate in sorted display order so
        // the cursor follows what the user sees (articles stripped) rather than raw item order.
        if !self.layout.main.left_sorted_indices.is_empty() {
            let needs_sorted = self.libs[lib_idx].search.is_none()
                && self.libs[lib_idx].nav_stack.last().is_some();
            if needs_sorted {
                let current = self.libs[lib_idx].nav_stack.last().unwrap().cursor;
                let sorted_n = self.layout.main.left_sorted_indices.len();
                let pos = self
                    .layout
                    .main
                    .left_sorted_indices
                    .iter()
                    .position(|&i| i == current)
                    .unwrap_or(0);
                let new_pos = (pos as i64 + delta).clamp(0, sorted_n as i64 - 1) as usize;
                let new_cursor = self.layout.main.left_sorted_indices[new_pos];
                if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut() {
                    lvl.cursor = new_cursor;
                }
                self.save_default_library_position(lib_idx);
                if idle {
                    self.maybe_fetch_next_page(lib_idx);
                }
                return;
            }
        }

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
                self.save_default_library_position(lib_idx);
            }
        }
        if idle {
            self.maybe_fetch_next_page(lib_idx);
        }
    }

    pub(super) fn jump_lib_cursor(&mut self, to_end: bool) {
        let lib_idx = self.library_tab.saturating_sub(1);

        if matches!(self.panel_focus, PanelFocus::Library)
            && self.libs[lib_idx].search.is_none()
            && self.libs[lib_idx].album_track_focus.is_none()
            && self.jump_power_music_group_display_cursor(lib_idx, to_end)
        {
            self.save_default_library_position(lib_idx);
            self.maybe_fetch_next_page(lib_idx);
            return;
        }

        if self.libs[lib_idx].search.is_none() && self.is_feed_home_video_group_view(lib_idx) {
            if let Some(state) = self.libs[lib_idx].feed_home_video.as_mut() {
                let n = state.selected_len();
                if n > 0 {
                    state.video_cursor = if to_end { n - 1 } else { 0 };
                    self.save_default_library_position(lib_idx);
                }
            }
            return;
        }

        // With letter-grouped display, Home/End jump to the first/last item
        // in sorted display order (article-stripped), not raw item order.
        if !self.layout.main.left_sorted_indices.is_empty() {
            let needs_sorted = self.libs[lib_idx].search.is_none()
                && !self.layout.main.left_sorted_indices.is_empty();
            if needs_sorted {
                let n = self.layout.main.left_sorted_indices.len();
                let new_cursor =
                    self.layout.main.left_sorted_indices[if to_end { n - 1 } else { 0 }];
                if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut() {
                    lvl.cursor = new_cursor;
                }
                self.save_default_library_position(lib_idx);
                self.maybe_fetch_next_page(lib_idx);
                return;
            }
        }

        let lib = &mut self.libs[lib_idx];
        if let Some(s) = &mut lib.search {
            let n = s.results.len();
            if n > 0 {
                s.cursor = if to_end { n - 1 } else { 0 };
            }
            return;
        }
        if let Some(lvl) = lib.nav_stack.last_mut() {
            let n = lvl.items.len();
            if n > 0 {
                lvl.cursor = if to_end { n - 1 } else { 0 };
                self.save_default_library_position(lib_idx);
            }
        }
        self.maybe_fetch_next_page(lib_idx);
    }

    pub(super) fn current_home_item(&self) -> Option<MediaItem> {
        if let Some(hs) = self.search.state() {
            return hs.filtered_results().get(hs.cursor).copied().cloned();
        }
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

    pub(super) fn spawn_global_search(&mut self, query: String) {
        let client = self.client.lock().unwrap().clone();
        self.search.spawn_global_search(client, query);
    }

    pub(super) fn current_lib_item(&self) -> Option<MediaItem> {
        let lib_idx = self.library_tab.checked_sub(1)?;
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
            // Track-selection mode (#145 task 4): when the power-left panel
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

    pub(super) fn is_viewing_album_folders(&self, lib_idx: usize) -> bool {
        let lib = &self.libs[lib_idx];
        if lib.library.collection_type != "music" {
            return false;
        }
        if self.music_levels.is_empty() {
            return false;
        }
        let stack_len = lib.nav_stack.len();
        if stack_len < 1 {
            return false;
        }
        self.music_levels
            .get(stack_len - 1)
            .map(|s| s == "album")
            .unwrap_or(false)
    }

    pub(super) fn is_viewing_season_grid(&self, lib_idx: usize) -> bool {
        let lib = &self.libs[lib_idx];
        if lib.search.is_some() {
            return false;
        }
        let lvl = match lib.nav_stack.last() {
            Some(l) => l,
            None => return false,
        };
        lvl.items
            .first()
            .map(|i| i.item_type == "Season")
            .unwrap_or(false)
    }
    /// Activates series-selection mode for the given Series item.
    /// Ensures the series detail is fetched and sets `series_selection`
    /// to start at the first episode.
    pub(super) fn enter_series_selection(&mut self, lib_idx: usize, item: &MediaItem) {
        if item.item_type != "Series" || item.id.is_empty() {
            return;
        }
        // Ensure the series detail (seasons + episodes) is fetched.
        self.fetch_series_detail(item.id.clone());
        self.libs[lib_idx].series_selection = Some(0);
    }

    /// Returns the episodes for the current season in series-selection
    /// mode, or `None` if not in selection mode.
    pub(super) fn series_selection_episodes(&self, lib_idx: usize) -> Option<Vec<MediaItem>> {
        let _ep_idx = self.libs[lib_idx].series_selection?;
        let item = self.power_selected_series_item(lib_idx)?;
        let detail = self.series_detail_cache.get(&item.id)?;
        let season = detail
            .seasons
            .get(self.libs[lib_idx].series_season_cursor)?;
        detail.episodes.get(&season.id).cloned()
    }

    /// Switches to the previous (`delta == -1`) or next (`delta == 1`)
    /// season while in series-selection mode. Adjusts the season cursor
    /// and ensures episodes for the new season are fetched.
    pub(super) fn switch_series_selection_season(&mut self, lib_idx: usize, delta: i64) {
        let Some(item) = self.power_selected_series_item(lib_idx) else {
            return;
        };
        let Some(detail) = self.series_detail_cache.get(&item.id).cloned() else {
            return;
        };
        let n = detail.seasons.len();
        if n == 0 {
            return;
        }
        let cur = self.libs[lib_idx].series_season_cursor;
        let new_cur = (cur as i64 + delta).clamp(0, n as i64 - 1) as usize;
        if new_cur == cur {
            return;
        }
        let new_season = &detail.seasons[new_cur];
        // Ensure episodes for the new season are fetched.
        if !detail.episodes.contains_key(&new_season.id) {
            let series_id = item.id.clone();
            let season_id = new_season.id.clone();
            let client = self.client.lock().unwrap().clone();
            let tx = self.lib_tx.clone();
            std::thread::spawn(move || {
                let eps = client
                    .get_items_sorted(
                        &season_id,
                        None,
                        false,
                        0,
                        super::PAGE_SIZE,
                        "IndexNumber",
                        "Ascending",
                    )
                    .map(|(items, _total)| items)
                    .unwrap_or_default();
                let _ = tx.send(LibEvent::SeriesSeasonEpisodesFetched {
                    series_id,
                    season_id,
                    episodes: eps,
                });
            });
        }
        self.libs[lib_idx].series_season_cursor = new_cur;
        // Reset episode cursor to first episode.
        self.libs[lib_idx].series_selection = Some(0);
    }

    pub(super) fn is_home_video_view(&self, lib_idx: usize) -> bool {
        let lib = &self.libs[lib_idx];
        lib.library.collection_type == "homevideos"
    }

    pub(super) fn toggle_mute(&mut self) {
        self.playback_target().toggle_soft_mute(self);
    }

    /// Session-aware mute toggle for `Action::ToggleMute` (the `m` key) when
    /// attached to a remote session. Mirrors `cycle_audio()`/`cycle_sub()`:
    /// computes an explicit target state (not a blind server-side toggle),
    /// writes it into `connected_session_state` optimistically, and fires the
    /// outbound command asynchronously via `do_session_command`. Does not
    /// touch local player mute state or the persisted `mute_on` preference --
    /// those are exclusively the local (no-session) branch's concern.
    pub(super) fn session_toggle_mute(&mut self) {
        let Some(conn_id) = self.connected_session_id.clone() else {
            return;
        };
        let current = self
            .connected_session_state
            .as_ref()
            .map(|s| s.muted)
            .unwrap_or(false);
        let next = !current;
        if let Some(ref mut state) = self.connected_session_state {
            state.muted = next;
        }
        self.do_session_command(move |c| c.session_set_mute(&conn_id, next));
    }

    pub(super) fn cycle_audio(&mut self) {
        self.playback_target().cycle_audio(self);
    }

    /// Clone the current subtitle prefs from the shared Arc and notify the player thread.
    pub(super) fn push_subtitle_prefs(&self) {
        let prefs = self.player.subtitle_prefs.lock().unwrap().clone();
        self.player
            .send_command(mbv_core::player::PlayerCommand::SetSubtitlePrefs {
                mode: prefs.mode,
                subtitle_lang: prefs.subtitle_lang,
                audio_lang: prefs.audio_lang,
            });
    }

    pub(super) fn cycle_subtitle_mode(&mut self) {
        let (new_mode, cfg) = {
            let mut c = self.client.lock().unwrap();
            c.config.subtitle_mode =
                super::ui_util::next_subtitle_mode(&c.config.subtitle_mode).to_string();
            (c.config.subtitle_mode.clone(), c.config.clone())
        };
        self.player.subtitle_prefs.lock().unwrap().mode = new_mode.clone();
        self.push_subtitle_prefs();
        if let Err(e) = crate::config::save_config_settings(&cfg) {
            log::warn!(target: "config", "config save failed: {e}");
        }
        self.flash_status(format!("Subtitle mode: {new_mode}"));
    }

    /// Returns the next entry in a subtitle-cycle sequence, wrapping around.
    /// `entries` is the ordered list of subtitle option ids -- the "off"
    /// sentinel first (`0` for local playback, `-1` for remote sessions),
    /// followed by each available track/index -- and `current` is the
    /// presently active selection. Shared by the remote-session and local
    /// branches of `cycle_sub` so both walk the exact same wraparound logic
    /// (see #86: local `z` used to be a plain on/off toggle instead of
    /// cycling through every track like the remote path).
    pub(super) fn next_subtitle_entry(entries: &[i64], current: i64) -> i64 {
        if entries.is_empty() {
            return current;
        }
        let cur_pos = entries.iter().position(|&e| e == current).unwrap_or(0);
        entries[(cur_pos + 1) % entries.len()]
    }

    /// Toggles between "off" and the last-selected subtitle index for a
    /// remote session. The only remaining caller is `cycle_sub`'s
    /// remote-session branch, as a fallback for when the session reports
    /// zero subtitle tracks (nothing to cycle through). Local playback no
    /// longer routes through here -- see #86, which replaced its on/off
    /// toggle with full track-cycling in `cycle_sub`.
    pub(super) fn toggle_sub(&mut self) {
        let Some(conn_id) = self.connected_session_id.clone() else {
            return;
        };
        let remote_indexes = self.remote_subtitle_indexes();
        let idx = self
            .connected_session_state
            .as_ref()
            .map(|s| s.sub_index)
            .unwrap_or(-1);
        let next = if idx == -1 {
            remote_indexes.first().copied().unwrap_or(1)
        } else {
            -1
        };
        if let Some(ref mut state) = self.connected_session_state {
            state.sub_index = next;
        }
        self.do_session_command(move |c| c.session_set_subtitle_index(&conn_id, next));
    }

    pub(super) fn cycle_sub(&mut self) {
        self.playback_target().cycle_sub(self);
    }

    fn notify_system(&self, msg: &str) {
        if self.system_notifications {
            let tx = self.notif_action_tx.clone();
            let mut cmd = std::process::Command::new("notify-send");
            cmd.arg("--app-name=mbv")
                .arg("mbv")
                .arg(msg)
                .stderr(std::process::Stdio::null());
            std::thread::spawn(move || {
                if !cmd.output().map(|o| o.status.success()).unwrap_or(false) {
                    let _ = tx.send("__notif_failed__".into());
                }
            });
        }
    }

    #[cfg(not(test))]
    fn ring_terminal_bell() {
        use std::io::Write;

        let mut stderr = std::io::stderr();
        let _ = stderr.write_all(b"\x07");
        let _ = stderr.flush();
    }

    // See the `TEST_BELL_LOG` doc comment above for why test builds don't
    // touch real stderr here.
    #[cfg(test)]
    fn ring_terminal_bell() {
        TEST_BELL_LOG.with(|log| log.borrow_mut().push(b'\x07'));
    }

    pub(super) fn notify_with_actions(&self, title: &str, body: &str, actions: &[(&str, &str)]) {
        Self::ring_terminal_bell();
        if !self.system_notifications {
            return;
        }
        let mut cmd = std::process::Command::new("notify-send");
        cmd.arg("--app-name=mbv")
            .arg(title)
            .arg(body)
            .stderr(std::process::Stdio::null());
        for (id, label) in actions {
            cmd.arg(format!("--action={}={}", id, label));
        }
        let tx = self.notif_action_tx.clone();
        std::thread::spawn(move || match cmd.output() {
            Ok(out) if out.status.success() => {
                let chosen = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let _ = tx.send(chosen);
            }
            _ => {
                let _ = tx.send("__notif_failed__".into());
            }
        });
    }

    pub(super) fn trigger_lib_rescan(&mut self, lib_idx: usize) {
        self.clear_saved_library_position(lib_idx);
        let client = self.client.lock().unwrap().clone();
        let library_id = self.libs[lib_idx].library.id.clone();
        let name = self.libs[lib_idx].library.name.clone();
        std::thread::spawn(move || {
            let _ = client.post_library_refresh(&library_id);
        });
        self.flash_status(format!("Scanning '{name}'..."));
    }

    pub(super) fn flash_status(&mut self, msg: String) {
        Self::ring_terminal_bell();
        self.notify_system(&msg);
        self.status = msg;
        self.status_expires = Some(Instant::now() + Duration::from_secs(2));
    }

    pub(super) fn flash_status_high(&mut self, msg: String) {
        Self::ring_terminal_bell();
        self.notify_system(&msg);
        self.status = msg;
        self.status_expires = Some(Instant::now() + Duration::from_secs(5));
    }

    /// Enforces #223's queue-route invariant: an item whose resolved
    /// route differs from the queue's current route (`active_route`) is
    /// rejected with a toast instead of being appended or silently
    /// swapping the player. Returns `true` if the enqueue was rejected --
    /// the caller must abort without mutating the queue.
    ///
    /// Short-circuits `false` (no conflict) whenever the app is currently
    /// in a thin-client mode that has nothing to do with library routing
    /// (a Sessions-panel attached session, or a non-library-route direct
    /// remote / local-daemon connection) -- both leave `active_route` at
    /// `None`, so without this check any item resolving to a configured
    /// `library_routes` entry would be wrongly rejected for a reason
    /// unrelated to library routing. Mirrors the same condition Task 9
    /// uses to gate `apply_route_for_playback`.
    pub(super) fn enqueue_route_conflict(&mut self, resolved_name: Option<String>) -> bool {
        if self.in_non_library_thin_client_mode() {
            return false;
        }
        if resolved_name != self.active_route {
            self.flash_status_high(
                "Can't mix libraries in a routed queue -- clear queue first".to_string(),
            );
            true
        } else {
            false
        }
    }

    pub(super) fn effective_playback_state(&self) -> super::PlaybackState {
        if let Some(ref remote) = self.connected_session_state {
            let maybe_active_idx = remote
                .now_playing_item_id
                .as_ref()
                .and_then(|id| self.player_tab.items.iter().position(|it| &it.id == id));
            let active_idx = maybe_active_idx.unwrap_or(0);
            let pos_ticks = {
                let elapsed_s = if remote.is_paused {
                    0.0
                } else {
                    self.remote_pos_at.elapsed().as_secs_f64()
                };
                let pos_s = (self.remote_pos_s as f64 + elapsed_s).min(remote.runtime_s as f64);
                (pos_s * mbv_core::api::TICKS_PER_SECOND as f64) as i64
            };
            super::PlaybackState {
                active: remote.now_playing.is_some() && maybe_active_idx.is_some(),
                active_idx,
                position_ticks: pos_ticks,
                runtime_ticks: remote.runtime_s * mbv_core::api::TICKS_PER_SECOND,
                paused: remote.is_paused,
            }
        } else {
            let s = self.player.status.lock().unwrap();
            super::PlaybackState {
                active: s.active,
                active_idx: s.current_idx,
                position_ticks: s.position_ticks,
                runtime_ticks: s.runtime_ticks,
                paused: s.paused,
            }
        }
    }

    pub(super) fn displayed_queue_playback_state(&self) -> super::PlaybackState {
        if self.queue_scope_is_playback(self.visible_queue_scope()) {
            self.effective_playback_state()
        } else {
            super::PlaybackState::default()
        }
    }

    pub(super) fn play_items_routed(&mut self, items: Vec<MediaItem>, start_idx: usize) {
        if let Some(item) = items.get(start_idx).or_else(|| items.first()) {
            log::info!(target: "library_route", "user action=queue-replace item_id={:?} item_name={:?}", item.id, item.name);
            if self.in_non_library_thin_client_mode() {
                log::info!(target: "library_route", "route bypass action=queue-replace item_id={:?} item_name={:?} reason=non-library thin-client owns playback", item.id, item.name);
            } else {
                let item = item.clone();
                self.apply_route_for_playback(&item);
            }
        }
        self.on_queue_replace_silent();
        self.set_queue_scope(self.playback_target_queue_scope());
        // Keep library focus when playing from the power-view library panel.
        if !matches!(self.panel_focus, PanelFocus::Library) {
            self.set_panel_focus(PanelFocus::Queue);
        }
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            self.clear_playback_overlays();
            let id = conn_id.clone();
            let item_ids: Vec<String> = items.iter().map(|i| i.id.clone()).collect();
            let start_ticks = items
                .get(start_idx)
                .map_or(0, |i| i.playback_position_ticks);
            let label = items
                .get(start_idx)
                .map(|i| i.playback_label())
                .unwrap_or_default();
            self.flash_status(format!("Playing on remote: {label}"));
            self.do_session_command(move |c| {
                c.session_play_items(&id, &item_ids, start_idx, start_ticks)
            });
            return;
        }
        let c = Arc::new(self.client.lock().unwrap().clone());
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

    pub(super) fn play_item(&mut self, item: MediaItem) {
        log::info!(target: "library_route", "user action=play item_id={:?} item_name={:?}", item.id, item.name);
        if self.in_non_library_thin_client_mode() {
            log::info!(target: "library_route", "route bypass action=play item_id={:?} item_name={:?} reason=non-library thin-client owns playback", item.id, item.name);
        } else {
            self.apply_route_for_playback(&item);
        }
        self.on_queue_replace_silent();
        // Keep library focus when playing from the power-view library panel.
        if !matches!(self.panel_focus, PanelFocus::Library) {
            self.set_panel_focus(PanelFocus::Queue);
        }
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
                self.replace_playback_queue(episodes.clone(), 0);
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
        self.replace_playback_queue(vec![item.clone()], 0);
        self.player
            .play(&item, self.queue_source.clone(), c, self.ui_volume);
        self.player
            .send_command(PlayerCommand::SetMute(self.mute_on));
    }

    pub(super) fn enqueue_selected(&mut self) {
        if self.library_tab == 0 {
            let Some(item) = self.current_home_item() else {
                return;
            };
            if item.is_folder {
                self.do_enqueue_folder(item);
                return;
            }
            if !is_playable(&item) {
                return;
            }
            log::info!(target: "library_route", "user action=enqueue item_id={:?} item_name={:?}", item.id, item.name);
            if self.in_non_library_thin_client_mode() {
                log::info!(target: "library_route", "route bypass action=enqueue item_id={:?} item_name={:?} reason=non-library thin-client owns playback", item.id, item.name);
            }
            let resolved = self.route_for_item_via_ancestors(&item.id).map(|(n, _)| n);
            if self.enqueue_route_conflict(resolved) {
                return;
            }
            self.append_item_to_queue_and_sync(item);
        } else {
            if self.enqueue_selected_artist_header() {
                return;
            }
            let Some(item) = self.current_lib_item() else {
                return;
            };
            if item.is_folder {
                self.do_enqueue_folder(item);
                return;
            }
            if !is_playable(&item) {
                return;
            }
            let lib_idx = self.library_tab - 1;
            let bypass = self.in_non_library_thin_client_mode();
            log::info!(target: "library_route", "{}", enqueue_action_context(&item.id, &item.name, "library-view", bypass));
            let resolved = self.route_for_active_library_view(lib_idx).map(|(n, _)| n);
            if self.enqueue_route_conflict(resolved) {
                return;
            }
            self.append_item_to_queue_and_sync(item);
        }
    }

    /// Shared append/sync/rollback tail for a single-item enqueue
    /// (extracted from `enqueue_selected`'s two branches, which had
    /// duplicated this verbatim): appends `item` to the visible queue,
    /// marks local queue metadata dirty when applicable, flashes a status
    /// confirmation, and syncs the append to the direct-remote queue /
    /// local persistence -- rolling the whole append back if the sync
    /// fails.
    fn append_item_to_queue_and_sync(&mut self, item: MediaItem) {
        let name = item.display_name();
        let scope = self.visible_queue_scope();
        let appended = item.clone();
        let previous_dirty = self.queue_dirty;
        let previous_queue = self.queue_for_scope(scope).clone();
        self.queue_for_scope_mut(scope).append_item(item);
        if self.local_queue_metadata_applies(scope) {
            self.queue_dirty = true;
        }
        self.flash_status(format!("Added: {name}"));
        if self.sync_playback_queue_after_append(scope, vec![appended]) {
            self.persist_local_queue_state_if_needed(scope);
        } else {
            self.queue_dirty = previous_dirty;
            *self.queue_for_scope_mut(scope) = previous_queue;
        }
    }

    pub(super) fn power_artist_header_action_lib_idx(&self) -> Option<usize> {
        if matches!(self.panel_focus, PanelFocus::Library) && self.library_tab > 0 {
            Some(self.library_tab - 1)
        } else {
            None
        }
    }

    fn selected_artist_header_action(&mut self) -> Option<(usize, ArtistHeaderSelection)> {
        let lib_idx = self.power_artist_header_action_lib_idx()?;
        self.selected_artist_header_album_items(lib_idx)
            .map(|(selection, _)| (lib_idx, selection))
    }

    fn resolve_artist_header_playable_items(
        &mut self,
        lib_idx: usize,
        selection: &ArtistHeaderSelection,
    ) -> Result<Vec<MediaItem>, String> {
        let albums = self
            .artist_header_album_items_for_selection(lib_idx, selection)
            .unwrap_or_default();
        let client = self.client.lock().unwrap();
        let mut resolved = Vec::new();
        for album in albums {
            let mut items = client.get_all_playable_recursive(&album.id)?;
            items.retain(|item| !item.is_folder && is_playable(item));
            sort_audio_tracks(&mut items);
            resolved.extend(items);
        }
        Ok(resolved)
    }

    pub(super) fn enqueue_artist_header_selection(
        &mut self,
        lib_idx: usize,
        selection: &ArtistHeaderSelection,
    ) -> bool {
        log::info!(target: "library_route", "user action=enqueue item_id={:?} item_name={:?} source=artist-header", selection.first_album_id, selection.artist_label);
        if self.in_non_library_thin_client_mode() {
            log::info!(target: "library_route", "route bypass action=enqueue item_id={:?} item_name={:?} source=artist-header reason=non-library thin-client owns playback", selection.first_album_id, selection.artist_label);
        }
        let resolved = self.route_for_active_library_view(lib_idx).map(|(n, _)| n);
        if self.enqueue_route_conflict(resolved) {
            return true;
        }
        let items = match self.resolve_artist_header_playable_items(lib_idx, selection) {
            Ok(items) => items,
            Err(e) => {
                self.flash_status_high(format!("Error: {e}"));
                return true;
            }
        };
        let count = items.len();
        if count == 0 {
            self.flash_status_high("Nothing to enqueue".into());
            return true;
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
        self.flash_status(format!(
            "Enqueued {count} items from {}",
            selection.artist_label
        ));
        if self.sync_playback_queue_after_append(scope, appended) {
            self.persist_local_queue_state_if_needed(scope);
        } else {
            self.queue_dirty = previous_dirty;
            *self.queue_for_scope_mut(scope) = previous_queue;
        }
        true
    }

    fn enqueue_selected_artist_header(&mut self) -> bool {
        let Some((lib_idx, selection)) = self.selected_artist_header_action() else {
            return false;
        };
        self.enqueue_artist_header_selection(lib_idx, &selection)
    }

    pub(super) fn play_artist_header_selection(
        &mut self,
        lib_idx: usize,
        selection: &ArtistHeaderSelection,
        shuffle: bool,
    ) -> bool {
        let mut items = match self.resolve_artist_header_playable_items(lib_idx, selection) {
            Ok(items) => items,
            Err(e) => {
                self.flash_status_high(format!("Error: {e}"));
                return true;
            }
        };
        let count = items.len();
        if count == 0 {
            self.flash_status_high(if shuffle {
                "Nothing to shuffle".into()
            } else {
                "Nothing to play".into()
            });
            return true;
        }
        if shuffle {
            items.shuffle(&mut rand::rng());
        }
        self.replace_playback_queue(items.clone(), 0);
        self.set_panel_focus(PanelFocus::Queue);
        self.flash_status(if shuffle {
            format!("Shuffling {count} items")
        } else {
            format!("Playing {count} items")
        });
        self.queue_source = if shuffle {
            crate::config::QueueSource::Shuffle
        } else {
            crate::config::QueueSource::Collection {
                collection_type: self.libs[lib_idx].library.collection_type.clone(),
            }
        };
        if !self.has_direct_remote_queue() {
            self.save_queue_state();
        }
        self.play_items_routed(items, 0);
        true
    }

    pub(super) fn play_selected_artist_header(&mut self, shuffle: bool) -> bool {
        let Some((lib_idx, selection)) = self.selected_artist_header_action() else {
            return false;
        };
        self.play_artist_header_selection(lib_idx, &selection, shuffle)
    }

    pub(super) fn do_enqueue_folder(&mut self, item: mbv_core::api::MediaItem) {
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
                    self.flash_status_high("Nothing to enqueue".into());
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
                self.flash_status(format!(
                    "Enqueued {count} items from {}",
                    item.display_name()
                ));
                if self.sync_playback_queue_after_append(scope, appended) {
                    self.persist_local_queue_state_if_needed(scope);
                } else {
                    self.queue_dirty = previous_dirty;
                    *self.queue_for_scope_mut(scope) = previous_queue;
                }
            }
            Err(e) => {
                drop(client);
                self.flash_status_high(format!("Error: {e}"));
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
            let lib_idx = self.library_tab - 1;
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
            let lib_idx = self.library_tab - 1;
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
                let mut tracks: Vec<MediaItem> =
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
        if self.library_tab > 0 {
            let lib_idx = self.library_tab - 1;

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

    pub(super) fn shuffle_play(&mut self) {
        if self.library_tab == 0 {
            return;
        }
        if self.play_selected_artist_header(true) {
            return;
        }
        let lib_idx = self.library_tab - 1;
        let parent_id = {
            let lib = &self.libs[lib_idx];
            let item = lib.nav_stack.last().and_then(|lvl| {
                let idx = lib
                    .search
                    .as_ref()
                    .and_then(|s| s.results.get(s.cursor).copied())
                    .unwrap_or(lvl.cursor);
                lvl.items.get(idx)
            });
            item.filter(|i| i.is_folder)
                .map(|i| i.id.clone())
                .or_else(|| lib.nav_stack.last().map(|l| l.parent_id.clone()))
                .unwrap_or_else(|| lib.library.id.clone())
        };
        // Delegate to the same fetch the context menu's Shuffle action uses
        // (`ContextAction::ShuffleFolder` -> `shuffle_folder`), rather than
        // duplicating this logic against `get_all_videos_recursive`, which
        // only requests Episode/Movie/Video types and so silently excludes
        // Audio -- Ctrl+S on a music album (all-Audio contents) always
        // fetched zero items and reported "Nothing to shuffle" even though
        // the album had playable tracks, while the context menu (already on
        // `get_all_playable_recursive`, which includes Audio) worked fine.
        self.shuffle_folder(&parent_id);
    }

    pub(super) fn play_folder(&mut self, folder_id: &str) {
        let client = self.client.lock().unwrap();
        match client.get_all_playable_recursive(folder_id) {
            Ok(mut items) => {
                items.retain(|i| !i.is_folder);
                items.sort_by_key(|a| natural_sort_key(a.sort_key()));
                if items.is_empty() {
                    drop(client);
                    self.flash_status_high("Nothing to play".into());
                    return;
                }
                let count = items.len();
                drop(client);
                self.replace_playback_queue(items.clone(), 0);
                self.set_panel_focus(PanelFocus::Queue);
                self.flash_status(format!("Playing {count} items"));
                self.play_items_routed(items, 0);
            }
            Err(e) => {
                drop(client);
                self.flash_status_high(format!("Error: {e}"));
            }
        }
    }

    pub(crate) fn is_tvshows_library(&self, lib_idx: usize) -> bool {
        self.libs[lib_idx].library.collection_type == "tvshows"
    }

    /// Whether the currently focused library tab is a tvshows library.
    /// Same bounds-check-then-delegate shape as `is_in_podcast_library`.
    ///
    /// Caveat: this reads the *active tab*, not the folder actually being
    /// shuffled -- `shuffle_folder`'s `folder_id` argument is not consulted
    /// here. That's fine for its two current callers (`shuffle_play`, only
    /// reachable once the left panel is already on a library tab; and the
    /// context menu's Shuffle action, only offered for a folder while
    /// browsing a library tab), but it would silently pick the wrong fetch
    /// for a folder reached some other way -- e.g. a future caller
    /// shuffling a folder surfaced by the global search overlay, or a
    /// Home-tab aggregate (Continue Watching/Latest), while a *different*
    /// library tab happens to be focused underneath. A robust fix for that
    /// case would resolve `folder_id`'s owning library via
    /// `get_ancestors`, the way `route_for_item_via_ancestors` in
    /// `library_route.rs` already does for the analogous "which library
    /// actually owns this item" problem in route resolution.
    fn active_lib_is_tvshows(&self) -> bool {
        let Some(lib_idx) = self.library_tab.checked_sub(1) else {
            return false;
        };
        lib_idx < self.libs.len() && self.is_tvshows_library(lib_idx)
    }

    pub(super) fn shuffle_folder(&mut self, folder_id: &str) {
        // TV libraries shuffle from a video-only fetch (Episode/Movie/Video)
        // so a season/series shuffle can't pull in stray Audio items (e.g.
        // theme songs); every other library type keeps the broader
        // playable-items fetch used for enqueue/play-all, which does
        // include Audio (needed for music libraries -- see the bug this
        // replaced).
        let is_tvshows = self.active_lib_is_tvshows();
        let client = self.client.lock().unwrap();
        let fetch = if is_tvshows {
            client.get_all_videos_recursive(folder_id)
        } else {
            client.get_all_playable_recursive(folder_id)
        };
        match fetch {
            Ok(mut items) => {
                items.retain(|i| !i.is_folder);
                if items.is_empty() {
                    drop(client);
                    self.flash_status_high("Nothing to shuffle".into());
                    return;
                }
                items.shuffle(&mut rand::rng());
                let count = items.len();
                drop(client);
                self.replace_playback_queue(items.clone(), 0);
                self.set_panel_focus(PanelFocus::Queue);
                self.flash_status(format!("Shuffling {count} items"));
                self.queue_source = crate::config::QueueSource::Shuffle;
                if !self.has_direct_remote_queue() {
                    self.save_queue_state();
                }
                self.play_items_routed(items, 0);
            }
            Err(e) => {
                drop(client);
                self.flash_status_high(format!("Error: {e}"));
            }
        }
    }

    pub(super) fn spawn_sessions_load(&mut self) {
        self.sessions_loading = true;
        let client = self.client.lock().unwrap().clone();
        let tx = self.sessions_tx.clone();
        std::thread::spawn(move || match client.get_sessions() {
            Ok(sessions) => {
                let _ = tx.send(SessionEvent::Loaded(sessions));
            }
            Err(e) => {
                let _ = tx.send(SessionEvent::Error(e));
            }
        });
    }

    pub(super) fn session_jump_track(
        &mut self,
        conn_id: &str,
        delta: i64,
        fallback_cmd: &'static str,
    ) {
        self.clear_playback_overlays();
        let id = conn_id.to_string();
        let current_remote_id = self
            .connected_session_state
            .as_ref()
            .and_then(|s| s.now_playing_item_id.as_deref())
            .map(str::to_string);
        let target = current_remote_id
            .and_then(|rid| self.player_tab.items.iter().position(|i| i.id == rid))
            .and_then(|idx| {
                let t = idx as i64 + delta;
                if t >= 0 && (t as usize) < self.player_tab.items.len() {
                    Some(t as usize)
                } else {
                    None
                }
            })
            .map(|t| (t, self.player_tab.items[t].playback_position_ticks));
        if let Some((target_idx, start_ticks)) = target {
            let item_ids: Vec<String> =
                self.player_tab.items.iter().map(|i| i.id.clone()).collect();
            self.do_session_command(move |c| {
                c.session_play_items(&id, &item_ids, target_idx, start_ticks)
            });
        } else {
            self.do_session_command(move |c| c.session_transport(&id, fallback_cmd));
        }
    }

    /// Compute the absolute tick position for a remote-session seek, given
    /// the current position in seconds and a relative delta in seconds.
    ///
    /// This reconstructs the asymmetric math the old inline remote-session
    /// `<`/`>` handlers in `input.rs` had: rewinding (`delta < 0`) clamps at
    /// zero, fast-forwarding does not (matching the prior
    /// `(pos_s - 5).max(0)` vs. `(pos_s + 5)`). Used by `action::dispatch`'s
    /// `Action::SeekRelative` arm; kept here alongside its sibling
    /// session-math helpers (`session_jump_track`, `do_session_command`)
    /// rather than in `action.rs`, since it's pure session-position math with
    /// no dependency on the `Action` seam itself.
    pub(super) fn remote_seek_ticks(pos_s: i64, delta: f64) -> i64 {
        let moved = pos_s + delta as i64;
        let target = if delta < 0.0 { moved.max(0) } else { moved };
        target * TICKS_PER_SECOND
    }

    pub(super) fn clear_playback_overlays(&mut self) {
        self.skip_intro_end_ticks = None;
        self.next_up_item = None;
        self.status.clear();
    }

    pub(super) fn do_session_command(
        &self,
        f: impl FnOnce(&EmbyClient) -> Result<(), String> + Send + 'static,
    ) {
        let client = self.client.lock().unwrap().clone();
        let tx = self.sessions_tx.clone();
        std::thread::spawn(move || {
            if let Err(e) = f(&client) {
                let _ = tx.send(SessionEvent::Error(e));
                return;
            }
            match client.get_sessions() {
                Ok(sessions) => {
                    let _ = tx.send(SessionEvent::Loaded(sessions));
                }
                Err(e) => {
                    let _ = tx.send(SessionEvent::Error(e));
                }
            }
        });
    }

    fn update_current_browse_level(
        &mut self,
        lib_idx: usize,
        parent_id: &str,
        require_loading: bool,
        mut update: impl FnMut(&mut BrowseLevel),
    ) -> bool {
        let Some(lib) = self.libs.get_mut(lib_idx) else {
            return false;
        };
        let Some(last) = lib.nav_stack.last_mut() else {
            return false;
        };
        if last.parent_id != parent_id || (require_loading && !last.loading) {
            return false;
        }
        update(last);
        true
    }

    fn normalize_current_browse_level_items(&mut self, lib_idx: usize) {
        if let Some(last) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.nav_stack.last_mut())
        {
            if last
                .items
                .first()
                .map(|item| item.item_type == "Episode")
                .unwrap_or(false)
            {
                sort_episodes(&mut last.items);
            }
        }
    }

    fn snap_grouped_album_cursor_to_display_order(&mut self, lib_idx: usize) {
        if !self.is_viewing_album_folders(lib_idx) {
            return;
        }
        // The grouped-by-artist album views (music.rs/list.rs) display albums
        // sorted by artist, not in the raw SortName-by-album-title order the
        // API returns them in — so the freshly-loaded default cursor (index 0
        // in raw order) can land on an arbitrary album instead of the first one
        // the user actually sees on screen. Snap it to the first album in (a
        // synchronous best-effort guess at) display order. Mirrors
        // `App::resolve_group_album_artist`'s fallback chain via
        // `initial_group_artist_sort_key`.
        if let Some(last) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.nav_stack.last_mut())
        {
            if !last.items.is_empty() {
                let mut order: Vec<usize> = (0..last.items.len()).collect();
                order
                    .sort_by_key(|&i| super::render::initial_group_artist_sort_key(&last.items[i]));
                last.cursor = order[0];
            }
        }
    }

    fn handle_loaded_level(&mut self, lib_idx: usize, parent_id: String, level: BrowseLevel) {
        let mut level = Some(level);
        self.update_current_browse_level(lib_idx, &parent_id, true, |last| {
            *last = level.take().unwrap();
        });
        self.normalize_current_browse_level_items(lib_idx);
        self.snap_grouped_album_cursor_to_display_order(lib_idx);
    }

    fn maybe_auto_push_power_tv_season_level(&mut self, lib_idx: usize) {
        // In Power View: when a season list arrives for a TV library,
        // automatically push a loading placeholder and fetch the first season's
        // episodes so the user lands directly in the combined series view.
        let should_auto_push = self.library_tab == lib_idx + 1
            && self
                .libs
                .get(lib_idx)
                .map(|lib| {
                    lib.library.collection_type == "tvshows"
                        && lib
                            .nav_stack
                            .last()
                            .map(|l| {
                                l.items
                                    .first()
                                    .map(|i| i.item_type == "Season")
                                    .unwrap_or(false)
                            })
                            .unwrap_or(false)
                })
                .unwrap_or(false);

        if should_auto_push {
            let (season_id, season_name) = self
                .libs
                .get(lib_idx)
                .and_then(|lib| lib.nav_stack.last())
                .and_then(|l| l.items.get(l.cursor))
                .map(|s| (s.id.clone(), s.name.clone()))
                .unwrap_or_default();
            if !season_id.is_empty() {
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    lib.nav_stack.push(BrowseLevel {
                        parent_id: season_id.clone(),
                        title: season_name.clone(),
                        items: vec![],
                        total_count: 0,
                        cursor: 0,
                        item_types: Some("Episode".into()),
                        unplayed_only: false,
                        sort_by: "SortName".into(),
                        sort_order: "Ascending".into(),
                        loading: true,
                        scroll: 0,
                        all_items: None,
                        letter_filter: None,
                    });
                }
                self.spawn_browse(
                    lib_idx,
                    season_id,
                    season_name,
                    Some("Episode".into()),
                    false,
                    "SortName".into(),
                    "Ascending".into(),
                );
            }
        }
    }

    fn handle_lib_loaded(&mut self, lib_idx: usize, parent_id: String, level: BrowseLevel) {
        self.handle_loaded_level(lib_idx, parent_id, level);
        self.maybe_capture_library_total_and_apply_default_pill(lib_idx);
        self.maybe_auto_push_power_tv_season_level(lib_idx);
        self.maybe_auto_push_power_music_group_level(lib_idx);
        self.maybe_aggregate_feed_after_loaded(lib_idx);
        self.maybe_fetch_next_page(lib_idx);
        self.spawn_all_items_prefetch(lib_idx);
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
        items: Vec<MediaItem>,
        total_count: usize,
    ) {
        let mut items = Some(items);
        self.update_current_browse_level(lib_idx, &parent_id, true, |last| {
            last.items.extend(items.take().unwrap());
            last.total_count = total_count;
            last.loading = false;
        });
        self.normalize_current_browse_level_items(lib_idx);
        self.maybe_aggregate_feed_after_page_append(lib_idx, &parent_id);
        self.maybe_fetch_next_page(lib_idx);
    }

    fn handle_lib_refreshed(
        &mut self,
        lib_idx: usize,
        parent_id: String,
        item_types: Option<String>,
        unplayed_only: bool,
        items: Vec<MediaItem>,
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
        if self.active_library_position_scope_for(lib_idx).is_none() {
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
        // Deliberately no `spawn_all_items_prefetch` call here (unlike
        // `handle_lib_loaded`'s sibling call, which is safe): this method
        // fires for every library restored at app *startup*, all
        // concurrently. Eagerly fetching+parsing a whole library's worth of
        // full-field items (People, MediaStreams, ...) here piles CPU-bound
        // JSON parsing on top of N other libraries' simultaneous restore
        // fetches and visibly stalls first paint of the default library
        // (#260). `all_items` is a pure cache for instant `/`-search open
        // (see `spawn_search_items_load`'s lazy fallback in
        // `input.rs`/`handle_lib_event`'s `SearchItemsLoaded` handling) --
        // nothing here requires it to be warm. If you're tempted to add
        // this back, don't: benchmark against a library with 500+ items
        // first and check `~/.local/state/mbv/mbv.log` for `parent=<id>`
        // `http=`/`parse=` timings from `get_items_sorted`.
    }

    pub(super) fn handle_lib_event(&mut self, ev: LibEvent) {
        match ev {
            LibEvent::Loaded {
                lib_idx,
                parent_id,
                level,
            } => self.handle_lib_loaded(lib_idx, parent_id, level),
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
                            self.flash_status_high(format!("Error: {error}"));
                        }
                    }
                    if let Some(lib_idx) = self
                        .libs
                        .iter()
                        .position(|lib| lib.library.id == library_id)
                    {
                        self.sync_recursive_album_search(lib_idx);
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
                    lib.search = None;
                    lib.album_track_focus = Some(0);
                }
                self.save_default_library_position(lib_idx);
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
            LibEvent::AlbumTracksFetched { album_id, tracks } => {
                self.album_tracks_loading.remove(&album_id);
                self.album_tracks_cache.insert(album_id, tracks);
            }
            LibEvent::SeriesDetailFetched {
                series_id,
                seasons,
                episodes,
            } => {
                self.series_detail_loading.remove(&series_id);
                self.series_detail_cache
                    .insert(series_id, crate::app::SeriesDetail { seasons, episodes });
            }
            LibEvent::SeriesSeasonEpisodesFetched {
                series_id,
                season_id,
                episodes,
            } => {
                if let Some(detail) = self.series_detail_cache.get_mut(&series_id) {
                    detail.episodes.insert(season_id, episodes);
                }
            }
            LibEvent::AlbumArtistFetched { album_id, artist } => {
                self.album_artist_loading.remove(&album_id);
                self.album_artist_cache.insert(album_id, artist);
                self.album_artist_fetches_active =
                    self.album_artist_fetches_active.saturating_sub(1);
                self.drain_album_artist_fetches();
            }
            LibEvent::NavigateTo {
                lib_idx,
                nav_stack,
                switch_tab,
            } => {
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    lib.nav_stack = nav_stack;
                    lib.search = None;
                }
                if switch_tab {
                    self.search.close();
                    self.set_library_tab(lib_idx + 1);
                }
            }
            LibEvent::PlaylistsLoaded(items) => {
                self.playlists = items;
                self.playlists_loading = false;
                self.playlists_cursor = self
                    .playlists_cursor
                    .min(self.playlists.len().saturating_sub(1));
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
            LibEvent::QueueEnriched { items } => {
                let _ = self.merge_refreshed_queue(QueueScope::Local, items);
            }
            LibEvent::Error(e) => {
                self.flash_status_high(format!("Error: {e}"));
            }
        }
    }

    /// `q` (and every keyboard/mouse path that routes here). In stay-alive
    /// mode this is a **detach** only while the current session still has
    /// `Stay alive on exit` enabled: diverted before `player.stop()`, the
    /// player keeps running and the run loop keeps going (returns `false`).
    /// If the user disables that setting mid-session, the next `q` becomes
    /// a real quit for the current attached app instance. `mbv -q` / tray-Quit
    /// remain real quits regardless (see `crate::app::stay_alive` / T3's
    /// graceful SIGTERM path).
    ///
    /// In bare mode this is a real quit. Any dirty saved-playlist queue is
    /// saved/discarded **silently** per `save_playlist_on_quit` — no
    /// interactive modal (that modal is reserved for the attended
    /// ClearQueue/PlayItems cases; see issue #156).
    pub(super) fn try_quit(&mut self) -> bool {
        let stay_alive_on_exit = self.client.lock().unwrap().config.stay_alive;
        if stay_alive_on_exit {
            if let Some(ctrl) = &self.stay_alive_ctrl {
                match ctrl.send_detach() {
                    Ok(()) => {
                        self.flash_status("Detached — mbv keeps playing in the background".into());
                        // #156: no terminal-client left to answer the run loop's
                        // terminal.clear()/draw() calls until the next reattach
                        // sets this back via take_attach_pending(); see the
                        // `attached` field doc for why that matters.
                        self.attached = false;
                    }
                    Err(e) => {
                        self.flash_status_high(format!(
                        "Detach failed ({e}) — still attached; try again or `mbv -q` from another shell"
                    ));
                    }
                }
                return false;
            }
        }
        if self.queue_dirty && self.queue_is_saved_playlist() {
            let save_on_quit = self.client.lock().unwrap().config.save_playlist_on_quit;
            if save_on_quit {
                // non-blocking: enqueues save in a spawned thread, does not block quit
                self.save_playlist_to_emby();
                self.queue_dirty = false;
            } else {
                self.on_queue_replace_silent();
            }
        }
        self.save_prefs();
        if !self.player.is_remote() {
            self.player.stop();
        }
        true
    }

    /// Called when a video item is removed from the queue because "consume" is enabled.
    /// Marks the queue dirty (matching how other queue-mutating actions behave, so the
    /// user is prompted to save on quit/replace), and — if the user has opted in via
    /// `save_playlist_on_consume` and the current queue is a saved Emby playlist — pushes
    /// the shorter item list back to Emby immediately, so other devices loading this
    /// playlist see the consumed items already removed instead of stale, longer state.
    ///
    /// Both checks are gated on `local_queue_metadata_applies`: `save_playlist_to_emby`
    /// always pushes `player_tab.items` (the *local* queue), so if the consume actually
    /// happened on a direct-remote/daemon queue, autosaving here would push an unrelated,
    /// unmodified local playlist to Emby instead of the queue that actually changed.
    pub(super) fn on_video_consumed(&mut self) {
        let scope = self.playback_target_queue_scope();
        log::info!(target: "consume", "on_video_consumed: scope={scope:?} has_local_metadata={}",
            self.local_queue_metadata_applies(scope));
        if !self.local_queue_metadata_applies(scope) {
            return;
        }
        self.queue_dirty = true;
        let save_on_consume = self.client.lock().unwrap().config.save_playlist_on_consume;
        let is_saved_playlist = self.queue_is_saved_playlist();
        log::info!(target: "consume", "on_video_consumed: queue_dirty=true save_playlist_on_consume={save_on_consume} \
            is_saved_playlist={is_saved_playlist}");
        if save_on_consume && is_saved_playlist {
            self.queue_dirty = false;
            self.save_playlist_to_emby();
        }
    }

    /// Called when an audio item is removed from the queue because "consume" is enabled.
    /// Mirrors `on_video_consumed`, but is gated on the audio-specific
    /// `save_playlist_on_consume_audio` flag instead — kept as a separate method (rather
    /// than a shared helper with a boolean parameter) so the video and audio consume paths
    /// stay independently readable and don't require the caller to track which flag applies.
    pub(super) fn on_audio_consumed(&mut self) {
        let scope = self.playback_target_queue_scope();
        log::info!(target: "consume", "on_audio_consumed: scope={scope:?} has_local_metadata={}",
            self.local_queue_metadata_applies(scope));
        if !self.local_queue_metadata_applies(scope) {
            return;
        }
        self.queue_dirty = true;
        let save_on_consume = self
            .client
            .lock()
            .unwrap()
            .config
            .save_playlist_on_consume_audio;
        let is_saved_playlist = self.queue_is_saved_playlist();
        log::info!(target: "consume", "on_audio_consumed: queue_dirty=true \
            save_playlist_on_consume_audio={save_on_consume} is_saved_playlist={is_saved_playlist}");
        if save_on_consume && is_saved_playlist {
            self.queue_dirty = false;
            self.save_playlist_to_emby();
        }
    }

    /// Number of selectable left-panel tabs in Power View: Home/CW + all libraries.
    pub(super) fn library_tab_count(&self) -> usize {
        1 + self.libs.len()
    }

    /// Jump directly to left-panel tab `idx` (0 = Home, 1..=libs.len() =
    /// library index `idx - 1`), e.g. from a tab-bar click or a digit key.
    pub(super) fn set_library_tab(&mut self, idx: usize) {
        if idx >= self.library_tab_count() {
            return;
        }
        self.library_tab = idx;
        self.last_card_height = 0; // reset stale image height for new view
        if idx > 0 {
            self.set_panel_focus(PanelFocus::Library);
            self.activate_library_position(idx - 1);
        }
        self.ensure_tab_visible();
        self.save_prefs();
    }

    /// Advance the left-panel tab (wrapping); load the library if needed.
    pub(super) fn library_tab_next(&mut self) {
        let n = self.library_tab_count();
        self.library_tab = (self.library_tab + 1) % n;
        self.last_card_height = 0; // reset stale image height for new view
        if self.library_tab > 0 {
            self.set_panel_focus(PanelFocus::Library);
            self.activate_library_position(self.library_tab - 1);
        }
        self.ensure_tab_visible();
        self.save_prefs();
    }

    /// Retreat the left-panel tab (wrapping); load the library if needed.
    pub(super) fn library_tab_prev(&mut self) {
        let n = self.library_tab_count();
        self.library_tab = (self.library_tab + n - 1) % n;
        self.last_card_height = 0;
        if self.library_tab > 0 {
            self.set_panel_focus(PanelFocus::Library);
            self.activate_library_position(self.library_tab - 1);
        }
        self.ensure_tab_visible();
        self.save_prefs();
    }

    /// Move the cursor in the Continue Watching power column, clamped to its bounds.
    pub(super) fn power_cw_move_cursor(&mut self, delta: i64) {
        let n = self.home.continue_items.len();
        if n == 0 {
            return;
        }
        let cur = self.home.continue_cursor.min(n - 1) as i64;
        self.home.continue_cursor = (cur + delta).clamp(0, n as i64 - 1) as usize;
    }

    // The Continue Watching power column shares state with the Home tab's
    // Continue Watching section, so these reuse the Home actions by briefly
    // pointing the Home context at that section.
    pub(super) fn power_cw_play(&mut self) {
        let Some(item) = self
            .home
            .continue_items
            .get(self.home.continue_cursor)
            .cloned()
        else {
            return;
        };
        if item.is_folder {
            return;
        }
        let saved_sec = self.home.section;
        self.home.section = 0;
        self.select_home();
        self.home.section = saved_sec;
    }

    pub(super) fn power_cw_enqueue(&mut self) {
        let saved_sec = self.home.section;
        self.home.section = 0;
        self.enqueue_selected();
        self.home.section = saved_sec;
    }

    pub(super) fn power_cw_toggle_watched(&mut self) {
        let saved_sec = self.home.section;
        self.home.section = 0;
        self.toggle_watched_home();
        self.home.section = saved_sec;
    }

    // ── Power-view home flat list ────────────────────────────────────────────

    /// The MediaItem at the current flat `home_cursor`, or None.
    pub(super) fn power_home_current_item(&self) -> Option<MediaItem> {
        let cursor = self.home.home_cursor;
        let mut pos = 0usize;
        for item in &self.home.continue_items {
            if pos == cursor {
                return Some(item.clone());
            }
            pos += 1;
        }
        for (_, _, items, _) in &self.home.latest {
            for item in items {
                if pos == cursor {
                    return Some(item.clone());
                }
                pos += 1;
            }
        }
        None
    }

    /// Flat cursor range for a power-home section. Section 0 is Keep Watching;
    /// non-empty latest sections keep their regular Home section index.
    fn power_home_section_range(&self, section_idx: usize) -> Option<(usize, usize)> {
        let mut pos = 0usize;
        if section_idx == 0 {
            return Some((0, self.home.continue_items.len()));
        }
        pos += self.home.continue_items.len();
        for (idx, (_, _, items, _)) in self.home.latest.iter().enumerate() {
            let current_section = idx + 1;
            if current_section == section_idx {
                return if items.is_empty() {
                    None
                } else {
                    Some((pos, items.len()))
                };
            }
            pos += items.len();
        }
        None
    }

    fn power_home_new_sections(&self) -> Vec<usize> {
        let mut sections = Vec::new();
        for (idx, (_, _, items, _)) in self.home.latest.iter().enumerate() {
            if !items.is_empty() {
                sections.push(idx + 1);
            }
        }
        sections
    }

    /// Whether `section_idx` is a selectable Home pill: section 0 (Continue
    /// Watching) is always valid, and any other index is valid iff it has a
    /// non-empty Newest section.
    pub(super) fn power_home_section_is_valid(&self, section_idx: usize) -> bool {
        section_idx == 0 || self.power_home_new_sections().contains(&section_idx)
    }

    pub(super) fn power_home_select_section(&mut self, section_idx: usize) {
        let section_idx = if self.power_home_section_is_valid(section_idx) {
            section_idx
        } else if let Some(first) = self.power_home_new_sections().first() {
            *first
        } else {
            self.home.section = 0;
            return;
        };
        self.home.section = section_idx;
        self.home.home_scroll = 0;
        if let Some((start, len)) = self.power_home_section_range(section_idx) {
            self.home.home_cursor = if len == 0 {
                start
            } else {
                self.home.home_cursor.clamp(start, start + len - 1)
            };
        }
    }

    fn power_home_visible_indices(&self) -> Vec<usize> {
        let mut indices = Vec::new();
        let selected = if self.power_home_section_is_valid(self.home.section) {
            self.home.section
        } else {
            self.power_home_new_sections().first().copied().unwrap_or(0)
        };
        if let Some((start, len)) = self.power_home_section_range(selected) {
            indices.extend(start..start + len);
        }
        indices
    }

    /// Move the flat power-home cursor by `delta`, clamped to the selected
    /// power-home section.
    pub(super) fn power_home_move_cursor(&mut self, delta: i64) {
        let indices = self.power_home_visible_indices();
        if indices.is_empty() {
            self.home.home_cursor = 0;
            return;
        };
        let pos = indices
            .iter()
            .position(|idx| *idx == self.home.home_cursor)
            .unwrap_or(0);
        let next = (pos as i64 + delta).clamp(0, indices.len() as i64 - 1) as usize;
        self.home.home_cursor = indices[next];
    }

    pub(super) fn power_home_select_start(&mut self) {
        if let Some(first) = self.power_home_visible_indices().first() {
            self.home.home_cursor = *first;
        }
    }

    pub(super) fn power_home_select_end(&mut self) {
        if let Some(last) = self.power_home_visible_indices().last() {
            self.home.home_cursor = *last;
        }
    }

    pub(super) fn power_home_move_down(&mut self) {
        self.power_home_move_cursor(1);
    }

    pub(super) fn power_home_move_up(&mut self) {
        self.power_home_move_cursor(-1);
    }

    /// Cycle the selected home section, wrapping at the ends. `dir` = -1 previous,
    /// +1 next.
    pub(super) fn power_home_move_section(&mut self, dir: i64) {
        let sections = self.power_home_new_sections();
        if sections.is_empty() {
            return;
        }
        let pos = sections
            .iter()
            .position(|&section_idx| section_idx == self.home.section);
        let next_pos = match pos {
            Some(p) => {
                let n = sections.len() as i64;
                (((p as i64 + dir) % n + n) % n) as usize
            }
            None => 0,
        };
        self.power_home_select_section(sections[next_pos]);
    }

    /// Play the item under the flat power-home cursor.
    pub(super) fn power_home_play(&mut self) {
        let Some(item) = self.power_home_current_item() else {
            return;
        };
        if item.is_folder {
            return;
        }
        let cursor = self.home.home_cursor;
        let cw_len = self.home.continue_items.len();
        if cursor < cw_len {
            // CW items: use select_home for proper resume handling.
            let (saved_sec, saved_cursor) = (self.home.section, self.home.continue_cursor);
            self.home.section = 0;
            self.home.continue_cursor = cursor;
            self.select_home();
            self.home.section = saved_sec;
            self.home.continue_cursor = saved_cursor;
        } else {
            self.play_item(item);
        }
    }

    /// Enqueue the item under the flat power-home cursor.
    pub(super) fn power_home_enqueue(&mut self) {
        let cursor = self.home.home_cursor;
        let cw_len = self.home.continue_items.len();
        if cursor < cw_len {
            let (saved_sec, saved_cursor) = (self.home.section, self.home.continue_cursor);
            self.home.section = 0;
            self.home.continue_cursor = cursor;
            self.enqueue_selected();
            self.home.section = saved_sec;
            self.home.continue_cursor = saved_cursor;
        } else {
            let Some(item) = self.power_home_current_item() else {
                return;
            };
            self.do_enqueue_folder(item);
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
        if !self.show_save_playlist_modal {
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
                        })
                        .collect()
                })
                .unwrap_or_default();
            let feed_home_video = saved.and_then(|s| s.feed_home_video.clone());
            self.libs.push(super::LibraryTab {
                library: view.clone(),
                nav_stack: stack,
                search: None,
                feed_home_video,

                album_track_focus: None,
                artist_header_focus: None,
                series_selection: None,
                series_season_cursor: 0,
                library_total: None,
            });
        }
    }

    pub(super) fn fetch_home(&mut self) -> Result<(), String> {
        let (continue_items, all_views, user_views) = {
            let client = self.client.lock().unwrap();
            (
                client.get_continue_watching(10).unwrap_or_default(),
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
                client.get_latest_episodes(&v.id, 15).unwrap_or_default()
            } else {
                client.get_latest(&v.id, 15).unwrap_or_default()
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

    pub(super) fn handle_ws_event(&mut self, ev: WsEvent) {
        match ev {
            WsEvent::Play {
                item_ids,
                play_now,
                start_position_ticks,
                start_index,
            } => {
                log::info!(target: "ws", "Play: {} id(s), play_now={play_now}", item_ids.len());
                if !play_now {
                    return;
                }
                self.on_queue_replace_silent();
                let items = {
                    let c = self.client.lock().unwrap();
                    match c.get_items_by_ids(&item_ids) {
                        Ok(v) => v,
                        Err(e) => {
                            let msg = format!("WS play error: {e}");
                            drop(c);
                            self.flash_status_high(msg);
                            return;
                        }
                    }
                };
                if items.is_empty() {
                    log::warn!(target: "ws", "Play: no items found for ids={}", item_ids.join(","));
                    return;
                }
                let start_idx = start_index.min(items.len().saturating_sub(1));
                self.set_panel_focus(PanelFocus::Queue);
                self.queue_source = crate::config::QueueSource::Remote;
                if items.len() == 1 {
                    let mut item = items[0].clone();
                    if start_position_ticks > 0 {
                        item.playback_position_ticks = start_position_ticks;
                    }
                    self.player_tab.set_items(vec![item.clone()], 0);
                    self.flash_status(item.playback_label());
                    let c = Arc::new(self.client.lock().unwrap().clone());
                    self.player
                        .play(&item, self.queue_source.clone(), c, self.ui_volume);
                } else {
                    let count = items.len();
                    self.player_tab.set_items(items.clone(), start_idx);
                    self.flash_status(format!("Playing {count} items"));
                    let c = Arc::new(self.client.lock().unwrap().clone());
                    log::info!(target: "ws", "Play multi: count={count}, start_idx={start_idx}");
                    // Always hand the whole list to play_queue (not just the clicked
                    // item) so the remote-controlled queue continues past start_idx.
                    // play_queue already handles the "something is already playing"
                    // case in place via ReplaceQueue.
                    let mut items_with_pos = items.clone();
                    if start_position_ticks > 0 {
                        items_with_pos[start_idx].playback_position_ticks = start_position_ticks;
                    }
                    self.player.play_queue(
                        items_with_pos,
                        start_idx,
                        self.queue_source.clone(),
                        c,
                        self.ui_volume,
                    );
                }
                self.save_queue_state();
            }
            WsEvent::Stop => {
                self.player.stop();
            }
            WsEvent::Pause => {
                self.player.set_paused(true);
            }
            WsEvent::Unpause => {
                self.player.set_paused(false);
            }
            WsEvent::NextTrack => {
                self.player.next();
            }
            WsEvent::PreviousTrack => {
                self.player.previous();
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
                self.player
                    .send_command(PlayerCommand::SetVolume(v.clamp(0, vol_max)));
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
            WsEvent::SetMute(muted) => {
                self.mute_on = muted;
                self.player.send_command(PlayerCommand::SetMute(muted));
                self.save_prefs();
            }
            WsEvent::ToggleMute => {
                let muted = !self.player.status.lock().unwrap().muted;
                self.mute_on = muted;
                self.player.send_command(PlayerCommand::SetMute(muted));
                self.save_prefs();
            }
            WsEvent::SetAudio(index) => {
                self.player.send_command(PlayerCommand::SetAudio(index));
            }
            WsEvent::SetSub(index) => {
                let sid = self
                    .player
                    .status
                    .lock()
                    .unwrap()
                    .subtitle_stream_index_to_mpv_id(index);
                if let Some(sid) = sid {
                    self.player.send_command(PlayerCommand::SetSub(sid));
                }
            }
            WsEvent::UserDataChanged => {
                let _ = self.fetch_home();
            }
        }
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

    pub(super) fn recursive_album_display_item(
        &self,
        lib_idx: usize,
        item_idx: usize,
        mut item: MediaItem,
    ) -> MediaItem {
        let Some(AlbumIndexState::Ready(entries)) = self
            .libs
            .get(lib_idx)
            .and_then(|lib| self.album_indexes.get(&lib.library.id))
        else {
            return item;
        };
        if let Some(entry) = entries
            .get(item_idx)
            .filter(|entry| entry.album.id == item.id)
        {
            item.name = entry.display_label.clone();
        }
        item
    }
}

#[cfg(test)]
#[path = "actions_tests.rs"]
mod tests;
#[test]
fn enqueue_action_context_names_action_item_and_thin_client_bypass() {
    assert_eq!(
            enqueue_action_context("item-42", "Track", "library-view", true),
            "user action=enqueue item_id=\"item-42\" item_name=\"Track\" source=library-view reason=non-library thin-client owns playback"
        );
}
