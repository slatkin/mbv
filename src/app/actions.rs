use super::ui_util::{is_playable, natural_sort_key, sort_audio_tracks, sort_episodes, take_chars};
use super::{
    AlbumIndexState, AlbumPathPart, AlbumSearchEntry, App, ArtistHeaderSelection, BrowseLevel,
    ContextAction, FeedHomeVideoGroup, FeedHomeVideoState, LibEvent, LibraryPositionScope,
    LibraryTab, LocalPlaybackTarget, PendingQueueAction, PlaybackTarget, PowerFocus, QueueScope,
    RemotePlaybackTarget, SessionEvent, UndoEntry, ViewMode, PAGE_SIZE, PREFETCH_AHEAD,
};
use crate::app::images::NAV_IMAGE_FETCH_IDLE_DELAY;
use crate::app::render::indicators::IndicatorData;
use mbv_core::api::{EmbyClient, MediaItem, TICKS_PER_SECOND};
use mbv_core::player::PlayerCommand;
use mbv_core::ws::WsEvent;
use rand::seq::SliceRandom;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

type BrowseRefresh = (
    usize,
    String,
    Option<String>,
    bool,
    String,
    String,
    usize,
    Option<super::render::power::LetterFilter>,
);
type AlbumIndexFetch<'a> =
    dyn FnMut(&str, usize, usize) -> Result<(Vec<MediaItem>, usize), String> + 'a;
const ALBUM_INDEX_PAGE_SIZE: usize = 200;

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

fn recursive_album_search_eligible(collection_type: &str, levels: &[String]) -> bool {
    collection_type == "music"
        && levels.len() > 1
        && levels.last().is_some_and(|level| level == "album")
}

/// The correct fetch `limit` for an unfiltered whole-library fetch, used by
/// `spawn_all_items_prefetch`/`spawn_search_items_load` so `all_items` (the
/// set `/`-search runs over) always spans the entire library. `lvl.total_count`
/// alone is NOT enough: with a letter-range pill active it's the FILTERED
/// range's count (e.g. ~40 for `A–C` out of a 3,000-item library), which
/// would silently truncate `all_items` to the active range and make search
/// miss everything outside it. `lib.library_total` (the true count captured
/// on the library's first, unfiltered load) is the right number; `.max` is
/// just a defensive fallback for the moment before it's been captured.
fn full_library_fetch_limit(lib: &LibraryTab, lvl: &BrowseLevel) -> usize {
    lib.library_total
        .unwrap_or(lvl.total_count)
        .max(lvl.total_count)
}

fn fetch_all_album_index_items(
    parent_id: &str,
    fetch: &mut AlbumIndexFetch<'_>,
) -> Result<Vec<MediaItem>, String> {
    let mut items = Vec::new();
    loop {
        let (page, total) = fetch(parent_id, items.len(), ALBUM_INDEX_PAGE_SIZE)?;
        if page.is_empty() {
            break;
        }
        items.extend(page);
        if items.len() >= total {
            break;
        }
    }
    Ok(items)
}

fn build_album_index_with(
    library_id: &str,
    levels: &[String],
    fetch: &mut AlbumIndexFetch<'_>,
) -> Result<Vec<AlbumSearchEntry>, String> {
    fn visit(
        parent_id: &str,
        depth: usize,
        levels: &[String],
        ancestors: &mut Vec<AlbumPathPart>,
        entries: &mut Vec<AlbumSearchEntry>,
        fetch: &mut AlbumIndexFetch<'_>,
    ) -> Result<(), String> {
        let items = fetch_all_album_index_items(parent_id, fetch)?;
        if depth + 1 == levels.len() {
            for album in items
                .into_iter()
                .filter(|item| item.item_type == "MusicAlbum")
            {
                let mut labels: Vec<String> =
                    ancestors.iter().map(|part| part.name.clone()).collect();
                labels.push(album.display_name());
                let display_label = labels.join(" / ");
                entries.push(AlbumSearchEntry {
                    album,
                    ancestors: ancestors.clone(),
                    search_text: display_label.clone(),
                    display_label,
                });
            }
            return Ok(());
        }

        for item in items.into_iter().filter(|item| item.is_folder) {
            ancestors.push(AlbumPathPart {
                id: item.id.clone(),
                name: item.display_name(),
            });
            visit(&item.id, depth + 1, levels, ancestors, entries, fetch)?;
            ancestors.pop();
        }
        Ok(())
    }

    let mut entries = Vec::new();
    visit(library_id, 0, levels, &mut Vec::new(), &mut entries, fetch)?;
    Ok(entries)
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
    fn log_feed_home_video_state(&self, lib_idx: usize, context: &str) {
        let Some(lib) = self.libs.get(lib_idx) else {
            log::debug!(target: "feedhv", "{context}: lib_idx={lib_idx} missing");
            return;
        };
        let root = lib.nav_stack.first();
        let feed = lib.feed_home_video.as_ref();
        log::debug!(
            target: "feedhv",
            "{context}: lib_idx={lib_idx} lib={} nav_len={} root_parent={} root_items={} root_loading={} root_cursor={} search={} feed_present={} feed_loading={} selected_group={} groups={} all_items={} video_cursor={} video_scroll={} group_view={}",
            lib.library.name,
            lib.nav_stack.len(),
            root.map(|lvl| lvl.parent_id.as_str()).unwrap_or(""),
            root.map(|lvl| lvl.items.len()).unwrap_or(0),
            root.map(|lvl| lvl.loading).unwrap_or(false),
            root.map(|lvl| lvl.cursor).unwrap_or(0),
            lib.search.is_some(),
            feed.is_some(),
            feed.map(|state| state.loading).unwrap_or(false),
            feed.map(|state| state.selected_group).unwrap_or(0),
            feed.map(|state| state.groups.len()).unwrap_or(0),
            feed.map(|state| state.all_items.len()).unwrap_or(0),
            feed.map(|state| state.video_cursor).unwrap_or(0),
            feed.map(|state| state.video_scroll).unwrap_or(0),
            self.is_feed_home_video_group_view(lib_idx),
        );
    }

    fn feed_home_video_visible_group_count(&self, lib_idx: usize) -> usize {
        self.libs
            .get(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_ref())
            .map(|state| state.groups.len())
            .unwrap_or(0)
    }

    pub(super) fn feed_home_video_selected_group_index(&self, lib_idx: usize) -> usize {
        self.libs
            .get(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_ref())
            .map(|state| state.selected_group_index())
            .unwrap_or(0)
    }

    pub(super) fn feed_home_video_selected_items(&self, lib_idx: usize) -> Vec<MediaItem> {
        let Some(state) = self
            .libs
            .get(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_ref())
        else {
            return Vec::new();
        };
        let selected_group = state.selected_group_index();
        if selected_group == 0 {
            state.all_items.clone()
        } else {
            state
                .groups
                .get(selected_group - 1)
                .map(|group| group.items.clone())
                .unwrap_or_default()
        }
    }

    fn feed_home_video_selected_parent_id(&self, lib_idx: usize) -> Option<String> {
        let lib = self.libs.get(lib_idx)?;
        let root = lib.nav_stack.first()?;
        let state = lib.feed_home_video.as_ref()?;
        let selected_group = state.selected_group_index();
        if selected_group == 0 {
            Some(root.parent_id.clone())
        } else {
            state
                .groups
                .get(selected_group - 1)
                .map(|group| group.folder.id.clone())
        }
    }

    /// Returns the item currently under the cursor without cloning the whole
    /// selected-group item list (see `feed_home_video_selected_items`, which
    /// does clone the full list and remains the right choice for callers that
    /// actually need it).
    pub(super) fn selected_feed_home_video_item(&self, lib_idx: usize) -> Option<MediaItem> {
        let state = self
            .libs
            .get(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_ref())?;
        let idx = state
            .video_cursor
            .min(state.selected_len().saturating_sub(1));
        let group = state.selected_group_index();
        if group == 0 {
            state.all_items.get(idx).cloned()
        } else {
            state
                .groups
                .get(group - 1)
                .and_then(|g| g.items.get(idx))
                .cloned()
        }
    }

    fn clamp_feed_home_video_state(&mut self, lib_idx: usize) {
        let Some(state) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_mut())
        else {
            return;
        };
        state.selected_group = state.selected_group_index();
        let items_len = state.selected_len();
        if items_len == 0 {
            state.video_cursor = 0;
            state.video_scroll = 0;
        } else {
            state.video_cursor = state.video_cursor.min(items_len.saturating_sub(1));
            state.video_scroll = state.video_scroll.min(state.video_cursor);
        }
    }

    fn remove_item_from_feed_home_video_cache(&mut self, lib_idx: usize, item_id: &str) {
        let Some(state) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_mut())
        else {
            return;
        };
        state.all_items.retain(|item| item.id != item_id);
        for group in &mut state.groups {
            group.items.retain(|item| item.id != item_id);
        }
        state.groups.retain(|group| !group.items.is_empty());
        self.clamp_feed_home_video_state(lib_idx);
        self.log_feed_home_video_state(lib_idx, "remove_from_cache");
    }

    pub(super) fn ensure_feed_home_video_group_level(&mut self, lib_idx: usize) {
        let Some(lib) = self.libs.get(lib_idx) else {
            return;
        };
        if lib.nav_stack.len() != 1 || lib.search.is_some() {
            return;
        }
        let ready = lib
            .feed_home_video
            .as_ref()
            .is_some_and(|state| !state.loading);
        if !ready || !(self.is_feed_home_video_library(lib_idx) || self.is_podcast_library(lib_idx))
        {
            return;
        }
        self.clamp_feed_home_video_state(lib_idx);
        self.log_feed_home_video_state(lib_idx, "ensure_group_level");
    }

    /// Common guard for kicking off `spawn_feed_home_video_aggregate` (or the
    /// podcast equivalent) once a grouped library's root folder listing has
    /// fully paginated: Power View is showing this library's tab, it's a
    /// feed-home-video or podcast library, and its root nav level has loaded
    /// every item. `extra_ok` carries the caller-specific condition (e.g.
    /// which event/level this check is reacting to).
    fn should_aggregate_feed(
        &self,
        lib_idx: usize,
        extra_ok: impl FnOnce(&BrowseLevel) -> bool,
    ) -> bool {
        self.view_mode == ViewMode::Power
            && self.power_left_tab == lib_idx + 1
            && (self.is_feed_home_video_library(lib_idx) || self.is_podcast_library(lib_idx))
            && self
                .libs
                .get(lib_idx)
                .map(|lib| {
                    lib.nav_stack.len() == 1
                        && lib.nav_stack[0].is_fully_loaded()
                        && extra_ok(&lib.nav_stack[0])
                })
                .unwrap_or(false)
    }

    fn spawn_feed_home_video_aggregate(&self, lib_idx: usize) {
        if !self.is_feed_home_video_library(lib_idx) {
            return;
        }
        let Some(lib) = self.libs.get(lib_idx) else {
            return;
        };
        let Some(root) = lib.nav_stack.first() else {
            return;
        };
        if root.loading {
            return;
        }
        let parent_id = root.parent_id.clone();
        let candidate_folders = root.items.clone();
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let (mut all_items, total_count) = match client.get_items_sorted(
                &parent_id,
                Some("Video"),
                true,
                0,
                PAGE_SIZE,
                "DateCreated",
                "Ascending",
            ) {
                Ok(items) => items,
                Err(e) => {
                    let _ = tx.send(LibEvent::Error(e));
                    return;
                }
            };
            if total_count > all_items.len() {
                match client.get_items_sorted(
                    &parent_id,
                    Some("Video"),
                    true,
                    0,
                    total_count,
                    "DateCreated",
                    "Ascending",
                ) {
                    Ok((items, _)) => all_items = items,
                    Err(e) => {
                        let _ = tx.send(LibEvent::Error(e));
                        return;
                    }
                }
            }

            let folder_ids: HashSet<String> = candidate_folders
                .iter()
                .map(|folder| folder.id.clone())
                .collect();
            let mut grouped: HashMap<String, Vec<MediaItem>> = HashMap::new();
            for video in &all_items {
                if folder_ids.is_empty() {
                    break;
                }
                let ancestors = match client.get_ancestors(&video.id) {
                    Ok(ancestors) => ancestors,
                    Err(e) => {
                        let _ = tx.send(LibEvent::Error(e));
                        return;
                    }
                };
                if let Some(folder) = ancestors
                    .iter()
                    .find(|ancestor| folder_ids.contains(&ancestor.id))
                {
                    grouped
                        .entry(folder.id.clone())
                        .or_default()
                        .push(video.clone());
                }
            }

            let groups = candidate_folders
                .into_iter()
                .filter_map(|folder| {
                    let items = grouped.remove(&folder.id).unwrap_or_default();
                    if items.is_empty() {
                        None
                    } else {
                        Some(FeedHomeVideoGroup { folder, items })
                    }
                })
                .collect();
            let _ = tx.send(LibEvent::FeedHomeVideoAggregated {
                lib_idx,
                parent_id,
                all_items,
                groups,
            });
        });
    }

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
        // In Power View the library list is rendered into the right panel, and the
        // normal-view per-row height map (`layout.library.lib_row_heights`) is never populated,
        // so it would fall back to 1. Use the panel height directly (rows are single-line;
        // subtract 1 for the count/search header line).
        if self.view_mode == ViewMode::Power {
            return (self.layout.power.left_area.height as usize)
                .saturating_sub(1)
                .max(1);
        }
        let lib_idx = if self.tab_idx >= self.lib_tab_offset() {
            self.tab_idx - self.lib_tab_offset()
        } else {
            0
        };
        self.layout
            .library
            .lib_row_heights
            .get(lib_idx)
            .map(|v| v.len().saturating_sub(1).max(1))
            .unwrap_or(1)
    }

    pub(super) fn queue_page_size(&self) -> usize {
        self.layout.queue.inner.height.saturating_sub(2).max(1) as usize
    }

    pub(super) fn move_lib_cursor(&mut self, delta: i64) {
        let now = Instant::now();
        let idle = now.duration_since(self.last_nav_at) >= NAV_IMAGE_FETCH_IDLE_DELAY;
        self.last_nav_at = now;
        if self.view_mode == ViewMode::Power {
            self.mark_power_library_navigation(now);
        }
        let lib_off = self.lib_tab_offset();
        let lib_idx = self.tab_idx - lib_off;

        if self.view_mode == ViewMode::Power
            && matches!(self.power_focus, PowerFocus::Left)
            && self.power_left_tab == lib_idx + 1
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

        // In Power View with letter-grouped display, navigate in sorted display order so
        // the cursor follows what the user sees (articles stripped) rather than raw item order.
        if self.view_mode == ViewMode::Power && !self.layout.power.left_sorted_indices.is_empty() {
            let needs_sorted = self.libs[lib_idx].search.is_none()
                && self.libs[lib_idx].nav_stack.last().is_some();
            if needs_sorted {
                let current = self.libs[lib_idx].nav_stack.last().unwrap().cursor;
                let sorted_n = self.layout.power.left_sorted_indices.len();
                let pos = self
                    .layout
                    .power
                    .left_sorted_indices
                    .iter()
                    .position(|&i| i == current)
                    .unwrap_or(0);
                let new_pos = (pos as i64 + delta).clamp(0, sorted_n as i64 - 1) as usize;
                let new_cursor = self.layout.power.left_sorted_indices[new_pos];
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
        let lib_off = self.lib_tab_offset();
        let lib_idx = self.tab_idx - lib_off;

        if self.view_mode == ViewMode::Power
            && matches!(self.power_focus, PowerFocus::Left)
            && self.power_left_tab == lib_idx + 1
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

        // In Power View with letter-grouped display, Home/End jump to the first/last item
        // in sorted display order (article-stripped), not raw item order.
        if self.view_mode == ViewMode::Power && !self.layout.power.left_sorted_indices.is_empty() {
            let needs_sorted = self.libs[lib_idx].search.is_none()
                && !self.layout.power.left_sorted_indices.is_empty();
            if needs_sorted {
                let n = self.layout.power.left_sorted_indices.len();
                let new_cursor =
                    self.layout.power.left_sorted_indices[if to_end { n - 1 } else { 0 }];
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

    pub(super) fn move_home_cursor(&mut self, delta: i64) {
        let sec = self.home.section;
        let (len, cur) = self.home_section_len_cur(sec);
        if delta > 0 {
            if cur + 1 < len {
                self.set_home_cursor(sec, cur + 1);
            }
        } else {
            if cur > 0 {
                self.set_home_cursor(sec, cur - 1);
            }
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
            let max_h_full = if panel_h < 12 {
                panel_h
            } else {
                ((panel_h as u32 * 24 / 25) as u16).min(24)
            }
            .max(4);
            let side_h_full = ((max_h_full as u32 * 4 / 5) as u16).max(3);
            let center_h_full = if compact {
                side_h_full
            } else {
                side_h_full + 2
            };
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

        let n_rows = 1 + n_latest.div_ceil(2);
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

    pub(super) fn home_scrollbar_seek(&mut self, row: u16) {
        let sb = self.layout.home.home_scrollbar;
        if sb.height == 0 {
            return;
        }
        let active = self.player.status.lock().unwrap().active;
        let chrome: u16 = if active { 6 } else { 3 };
        let panel_h = self.terminal_height.saturating_sub(chrome);
        let n_latest = self.home.latest.len();
        let n_rows = 1 + n_latest.div_ceil(2);
        let visible_rows = ((panel_h / super::HOME_MIN_SECTION_H) as usize)
            .max(1)
            .min(n_rows);
        let max_offset = n_rows.saturating_sub(visible_rows);
        if max_offset == 0 {
            return;
        }
        let frac = (row.saturating_sub(sb.y)) as f64 / sb.height as f64;
        let new_offset = ((frac * max_offset as f64).round() as usize).min(max_offset);
        self.home_panel_section_offset = new_offset;
    }

    pub(super) fn home_section_len_cur(&self, sec: usize) -> (usize, usize) {
        if sec == 0 {
            (self.home.continue_items.len(), self.home.continue_cursor)
        } else {
            self.home
                .latest
                .get(sec - 1)
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
        let lib_idx = self.tab_idx - self.lib_tab_offset();
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
            // holds, so this branch is unreachable from every other tab,
            // every other nav level, and the legacy `is_album_level`
            // drilldown.
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

    pub(super) fn is_album_level(&self, lib_idx: usize) -> bool {
        let lib = &self.libs[lib_idx];
        if lib.library.collection_type != "music" {
            return false;
        }
        if self.music_levels.is_empty() {
            return false;
        }
        let stack_len = lib.nav_stack.len();
        if stack_len < 2 {
            return false;
        }
        self.music_levels
            .get(stack_len - 2)
            .map(|s| s == "album")
            .unwrap_or(false)
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

    /// True when Power View should show the combined music group view:
    /// a group-selector bar at top with the album list below.
    /// Activated when `music.levels` starts with `"group"` and the nav stack
    /// has a group level plus an album level above it.
    pub(super) fn is_music_group_view(&self, lib_idx: usize) -> bool {
        let lib = &self.libs[lib_idx];
        if lib.library.collection_type != "music" {
            return false;
        }
        if lib.search.is_some() {
            return false;
        }
        // Only when the first configured level is "group".
        if self
            .music_levels
            .first()
            .map(|s| s != "group")
            .unwrap_or(true)
        {
            return false;
        }
        // Need at least a group level and an album level on the stack.
        if lib.nav_stack.len() < 2 {
            return false;
        }
        // The top nav level must be the album-folder level.
        self.is_viewing_album_folders(lib_idx)
    }

    pub(super) fn is_home_video_view(&self, lib_idx: usize) -> bool {
        let lib = &self.libs[lib_idx];
        lib.library.collection_type == "homevideos"
    }

    pub(super) fn is_feed_home_video_group_view(&self, lib_idx: usize) -> bool {
        let lib = &self.libs[lib_idx];
        if lib.search.is_some() {
            return false;
        }
        let has_state = lib.feed_home_video.as_ref().is_some_and(|state| {
            state.loading || !state.groups.is_empty() || !state.all_items.is_empty()
        });
        if !has_state {
            return false;
        }
        // Podcast channels always use the group view.
        if self.is_podcast_library(lib_idx) {
            return true;
        }
        // Feed home-video libraries use the group view when configured.
        if lib.library.collection_type != "homevideos" {
            return false;
        }
        let client = self.client.lock().unwrap();
        client
            .config
            .feed_view_libraries
            .contains(&lib.library.name.to_lowercase())
            && lib
                .nav_stack
                .first()
                .is_some_and(|lvl| lvl.item_types.is_none())
    }

    pub(super) fn ensure_feed_home_video_root_loaded(&mut self, lib_idx: usize) {
        if !self.is_feed_home_video_library(lib_idx) {
            return;
        }
        let needs_reload = self
            .libs
            .get(lib_idx)
            .map(|lib| {
                lib.nav_stack.is_empty()
                    || (!lib.nav_stack[0].loading
                        && lib.nav_stack[0]
                            .items
                            .first()
                            .map(|item| !item.is_folder)
                            .unwrap_or(true))
            })
            .unwrap_or(false);
        if !needs_reload {
            return;
        }
        let lib_id = self.libs[lib_idx].library.id.clone();
        let lib_name = self.libs[lib_idx].library.name.clone();
        self.libs[lib_idx].nav_stack.clear();
        self.libs[lib_idx].search = None;
        self.libs[lib_idx].feed_home_video = Some(FeedHomeVideoState {
            loading: true,
            ..self.libs[lib_idx]
                .feed_home_video
                .take()
                .unwrap_or_default()
        });
        self.libs[lib_idx].nav_stack.push(BrowseLevel {
            parent_id: lib_id.clone(),
            title: lib_name.clone(),
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
        self.spawn_browse(
            lib_idx,
            lib_id,
            lib_name,
            None,
            false,
            "SortName".into(),
            "Ascending".into(),
        );
        self.log_feed_home_video_state(lib_idx, "root_reload");
    }

    pub(crate) fn is_feed_home_video_library(&self, lib_idx: usize) -> bool {
        let lib = &self.libs[lib_idx];
        if lib.library.collection_type != "homevideos" {
            return false;
        }
        let client = self.client.lock().unwrap();
        client
            .config
            .feed_view_libraries
            .contains(&lib.library.name.to_lowercase())
    }

    pub(crate) fn is_podcast_library(&self, lib_idx: usize) -> bool {
        let lib = &self.libs[lib_idx];
        lib.library.item_type == "Channel"
            || lib.library.collection_type == "podcasts"
            || lib.library.name.to_lowercase().contains("podcast")
    }

    /// Whether the currently focused library tab is a podcast channel.
    pub(super) fn is_in_podcast_library(&self) -> bool {
        let lib_off = self.lib_tab_offset();
        if self.tab_idx < lib_off {
            return false;
        }
        let lib_idx = self.tab_idx - lib_off;
        lib_idx < self.libs.len() && self.is_podcast_library(lib_idx)
    }

    fn ensure_podcast_root_loaded(&mut self, lib_idx: usize) {
        if !self.is_podcast_library(lib_idx) {
            return;
        }
        let needs_reload = self
            .libs
            .get(lib_idx)
            .map(|lib| {
                lib.nav_stack.is_empty()
                    || (!lib.nav_stack[0].loading
                        && lib.nav_stack[0]
                            .items
                            .first()
                            .map(|item| !item.is_folder)
                            .unwrap_or(true))
            })
            .unwrap_or(false);
        if !needs_reload {
            return;
        }
        let lib_id = self.libs[lib_idx].library.id.clone();
        let lib_name = self.libs[lib_idx].library.name.clone();
        self.libs[lib_idx].nav_stack.clear();
        self.libs[lib_idx].search = None;
        self.libs[lib_idx].feed_home_video = Some(FeedHomeVideoState {
            loading: true,
            ..self.libs[lib_idx]
                .feed_home_video
                .take()
                .unwrap_or_default()
        });
        self.libs[lib_idx].nav_stack.push(BrowseLevel {
            parent_id: lib_id.clone(),
            title: lib_name.clone(),
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
        self.spawn_browse(
            lib_idx,
            lib_id,
            lib_name,
            None,
            false,
            "SortName".into(),
            "Ascending".into(),
        );
    }

    /// Fetch episodes for each podcast show folder, sorted newest-first.
    /// Much simpler than feed-home-video aggregation: episodes are direct
    /// children of each show folder, no ancestor lookups needed.
    fn spawn_podcast_aggregate(&self, lib_idx: usize) {
        if !self.is_podcast_library(lib_idx) {
            return;
        }
        let Some(lib) = self.libs.get(lib_idx) else {
            return;
        };
        let Some(root) = lib.nav_stack.first() else {
            return;
        };
        if root.loading {
            return;
        }
        let parent_id = root.parent_id.clone();
        let show_folders = root.items.clone();
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let mut all_items: Vec<MediaItem> = Vec::new();
            let mut groups: Vec<FeedHomeVideoGroup> = Vec::new();
            for folder in show_folders {
                let episodes = match client.get_items_sorted(
                    &folder.id,
                    None,
                    false,
                    0,
                    10000, // fetch all episodes
                    "PremiereDate",
                    "Descending",
                ) {
                    Ok((items, _)) => items,
                    Err(e) => {
                        let _ = tx.send(LibEvent::Error(e));
                        return;
                    }
                };
                all_items.extend(episodes.clone());
                if !episodes.is_empty() {
                    groups.push(FeedHomeVideoGroup {
                        folder,
                        items: episodes,
                    });
                }
            }
            // Sort the combined "All" list newest-first by premiere_date.
            all_items.sort_by(|a, b| b.premiere_date.cmp(&a.premiere_date));
            let _ = tx.send(LibEvent::FeedHomeVideoAggregated {
                lib_idx,
                parent_id,
                all_items,
                groups,
            });
        });
    }

    pub(super) fn select_feed_folder_group(&mut self, lib_idx: usize, group_idx: usize) {
        if self.libs[lib_idx].nav_stack.is_empty() {
            return;
        }
        let n = self.feed_home_video_visible_group_count(lib_idx);
        if group_idx > n {
            return;
        }
        if let Some(state) = self.libs[lib_idx].feed_home_video.as_mut() {
            state.selected_group = group_idx;
            state.video_cursor = 0;
            state.video_scroll = 0;
        }
        self.clamp_feed_home_video_state(lib_idx);
        self.log_feed_home_video_state(lib_idx, "select_group");
    }

    pub(super) fn switch_feed_folder_group(&mut self, lib_idx: usize, delta: i64) {
        let n = self.feed_home_video_visible_group_count(lib_idx) + 1;
        if n == 0 {
            return;
        }
        let cur = self.feed_home_video_selected_group_index(lib_idx);
        let next = (cur as i64 + delta).rem_euclid(n as i64) as usize;
        self.select_feed_folder_group(lib_idx, next);
    }

    /// Switch to the previous (`delta == -1`) or next (`delta == 1`) group
    /// while in the combined music group view. Pops the current album level,
    /// adjusts the group cursor (wraps around), then kicks off a fetch for
    /// the new group's albums.
    pub(super) fn switch_music_group(&mut self, lib_idx: usize, delta: i64) {
        let stack_len = self.libs[lib_idx].nav_stack.len();
        if stack_len < 2 {
            return;
        }

        // Verify count before popping so we never lose the album level.
        let n = self.libs[lib_idx].nav_stack[stack_len - 2].items.len();
        if n == 0 {
            return;
        }

        self.libs[lib_idx].clear_power_music_focus();

        // Pop the album level.
        self.libs[lib_idx].nav_stack.pop();
        let cur = self.libs[lib_idx]
            .nav_stack
            .last()
            .map(|l| l.cursor)
            .unwrap_or(0);
        // Wrap-around navigation (unlike seasons which clamp).
        let new_cursor = (cur as i64 + delta).rem_euclid(n as i64) as usize;
        if let Some(group_lvl) = self.libs[lib_idx].nav_stack.last_mut() {
            group_lvl.cursor = new_cursor;
        }

        // Collect new group's identity.
        let (group_id, group_name) = self.libs[lib_idx]
            .nav_stack
            .last()
            .and_then(|l| l.items.get(new_cursor))
            .map(|g| (g.id.clone(), g.name.clone()))
            .unwrap_or_default();
        if group_id.is_empty() {
            return;
        }

        // Push a loading placeholder so the Loaded handler can fill it in.
        self.libs[lib_idx].nav_stack.push(BrowseLevel {
            parent_id: group_id.clone(),
            title: group_name.clone(),
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
        self.spawn_browse(
            lib_idx,
            group_id,
            group_name,
            None,
            false,
            "SortName".into(),
            "Ascending".into(),
        );
    }

    pub(super) fn select_music_group(&mut self, lib_idx: usize, group_cursor: usize) {
        let stack_len = self.libs[lib_idx].nav_stack.len();
        if stack_len < 2 {
            return;
        }
        let n = self.libs[lib_idx].nav_stack[stack_len - 2].items.len();
        if group_cursor >= n {
            return;
        }
        self.libs[lib_idx].clear_power_music_focus();
        self.libs[lib_idx].nav_stack.pop();
        if let Some(group_lvl) = self.libs[lib_idx].nav_stack.last_mut() {
            group_lvl.cursor = group_cursor;
        }
        let (group_id, group_name) = self.libs[lib_idx]
            .nav_stack
            .last()
            .and_then(|l| l.items.get(group_cursor))
            .map(|g| (g.id.clone(), g.name.clone()))
            .unwrap_or_default();
        if group_id.is_empty() {
            return;
        }
        self.libs[lib_idx].nav_stack.push(BrowseLevel {
            parent_id: group_id.clone(),
            title: group_name.clone(),
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
        self.spawn_browse(
            lib_idx,
            group_id,
            group_name,
            None,
            false,
            "SortName".into(),
            "Ascending".into(),
        );
    }

    /// Whether the Power View letter-range pill row applies to `lib_idx`
    /// right now: a non-music library, not searching, at the top browse
    /// level of its nav stack (`nav_stack.len() == 1`), with a captured true
    /// total (`LibraryTab.library_total`) over `LIBRARY_PILL_THRESHOLD`. See
    /// `render::power::LetterFilter` and
    /// `maybe_capture_library_total_and_apply_default_pill`, which populates
    /// `library_total` on a library's first load.
    pub(super) fn should_show_letter_pills(&self, lib_idx: usize) -> bool {
        let Some(lib) = self.libs.get(lib_idx) else {
            return false;
        };
        if lib.library.collection_type == "music" {
            return false;
        }
        if lib.search.is_some() {
            return false;
        }
        if lib.nav_stack.len() != 1 {
            return false;
        }
        lib.library_total
            .is_some_and(|total| total > super::render::power::LIBRARY_PILL_THRESHOLD)
    }

    /// Selects letter-range pill `pill_index` for `lib_idx`'s top level (a
    /// direct precedent: `select_music_group`). Resets cursor/scroll, marks
    /// the level loading, and spawns a scoped refresh fetching only that
    /// range from Emby (`get_items_sorted_ranged`) -- the existing in-list
    /// letter headers (`list.rs`) then bucket the smaller slice per-letter.
    /// Persists the choice so it survives a restart (`LibraryPositionLevel`).
    pub(super) fn select_letter_pill(&mut self, lib_idx: usize, pill_index: usize) {
        if !self.should_show_letter_pills(lib_idx) {
            return;
        }
        let Some(filter) = super::render::power::LetterFilter::for_index(pill_index) else {
            return;
        };
        let Some(lvl) = self.libs[lib_idx].nav_stack.last() else {
            return;
        };
        if lvl.letter_filter.as_ref() == Some(&filter) {
            return;
        }
        let parent_id = lvl.parent_id.clone();
        let item_types = lvl.item_types.clone();
        let unplayed_only = lvl.unplayed_only;
        let sort_by = lvl.sort_by.clone();
        let sort_order = lvl.sort_order.clone();
        if let Some(last) = self.libs[lib_idx].nav_stack.last_mut() {
            last.letter_filter = Some(filter.clone());
            last.cursor = 0;
            last.scroll = 0;
            last.loading = true;
            last.items.clear();
            last.all_items = None;
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
        self.save_default_library_position(lib_idx);
    }

    /// Cycles the letter-range pill row by `delta` (`[`/`]` keyboard
    /// bindings), wrapping around -- the established pattern from
    /// `switch_music_group`.
    pub(super) fn cycle_letter_pill(&mut self, lib_idx: usize, delta: i64) {
        if !self.should_show_letter_pills(lib_idx) {
            return;
        }
        let n = super::render::power::LetterFilter::count();
        if n == 0 {
            return;
        }
        let current = self.libs[lib_idx]
            .nav_stack
            .last()
            .and_then(|l| l.letter_filter.as_ref())
            .map(|f| f.index)
            .unwrap_or(0);
        let next = (current as i64 + delta).rem_euclid(n as i64) as usize;
        self.select_letter_pill(lib_idx, next);
    }

    /// If the music-group library's nav_stack was truncated back to just the
    /// group level (e.g., by a stale breadcrumb click), immediately re-push the
    /// current group's album level so the combined view stays intact.
    pub(super) fn ensure_music_group_album_level(&mut self, lib_idx: usize) {
        if lib_idx >= self.libs.len() {
            return;
        }
        let should_push = self.libs[lib_idx].library.collection_type == "music"
            && self
                .music_levels
                .first()
                .map(|s| s == "group")
                .unwrap_or(false)
            && self.libs[lib_idx].nav_stack.len() == 1
            && !self.libs[lib_idx].nav_stack[0].items.is_empty();
        if !should_push {
            return;
        }
        let cur = self.libs[lib_idx].nav_stack[0].cursor;
        let n = self.libs[lib_idx].nav_stack[0].items.len();
        if cur >= n {
            return;
        }
        let (group_id, group_name) = {
            let g = &self.libs[lib_idx].nav_stack[0].items[cur];
            (g.id.clone(), g.name.clone())
        };
        self.libs[lib_idx].nav_stack.push(BrowseLevel {
            parent_id: group_id.clone(),
            title: group_name.clone(),
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
        self.spawn_browse(
            lib_idx,
            group_id,
            group_name,
            None,
            false,
            "SortName".into(),
            "Ascending".into(),
        );
    }

    /// Whether the item currently playing is audio-only, used to decide
    /// `a`'s mute-vs-cycle branch (`Action::ToggleMuteOrCycleAudio`). When a
    /// remote session is connected, reads the same `media_info.audio_only`
    /// flag the render layer already uses to pick audio-only vs. video
    /// indicators for that session (see #88), rather than the local
    /// playlist/cursor state, which doesn't reflect what the session is
    /// playing.
    pub(super) fn is_audio_item(&self) -> bool {
        self.playback_target().is_audio_item(self)
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

    pub(super) fn remove_from_queue(&mut self, pos: usize) {
        let scope = self.visible_queue_scope();
        let controls_playback_queue = self.queue_scope_is_playback(scope);
        let (active, current_idx) = {
            let s = self.player.status.lock().unwrap();
            (s.active, s.current_idx)
        };
        if scope == QueueScope::Remote && !active {
            self.flash_status_high("Remote queue can only be edited while active".into());
            return;
        }
        if pos >= self.queue_for_scope(scope).items.len() {
            let queue = self.queue_for_scope_mut(scope);
            queue.clamp_cursor();
            return;
        }
        if controls_playback_queue && active && current_idx == pos {
            self.confirm_remove_idx = Some(pos);
            self.status = "Remove now-playing item and stop playback? (y/N)".into();
            self.status_expires = None;
            return;
        }
        let Some(item) = self.queue_for_scope_mut(scope).remove_slot_at(pos) else {
            return;
        };
        if self.local_queue_metadata_applies(scope) {
            self.queue_dirty = true;
        }
        self.undo_stack_for_scope_mut(scope)
            .push(UndoEntry::Remove(pos, Box::new(item)));
        self.persist_local_queue_state_if_needed(scope);
        if controls_playback_queue && active {
            self.player.send_command(PlayerCommand::QueueRemove(pos));
            // Player thread adjusts current_idx when it processes the command.
            // No eager adjustment here — doing so races with the player thread
            // and can cause index mismatches during rapid removals.
        }
        let queue = self.queue_for_scope_mut(scope);
        queue.clamp_cursor();
    }

    /// Moves the item at the displayed queue's cursor one position earlier.
    /// No-op at the start of the queue.
    pub(super) fn move_queue_item_up(&mut self) {
        self.move_queue_item_by(-1);
    }

    /// Moves the item at the displayed queue's cursor one position later.
    /// No-op at the end of the queue.
    pub(super) fn move_queue_item_down(&mut self) {
        self.move_queue_item_by(1);
    }

    fn move_queue_item_by(&mut self, delta: isize) {
        let scope = self.visible_queue_scope();
        let active = self.player.status.lock().unwrap().active;
        if scope == QueueScope::Remote && !active {
            self.flash_status_high("Remote queue can only be edited while active".into());
            return;
        }
        let queue = self.queue_for_scope(scope);
        let from = queue.queue_cursor;
        let len = queue.items.len();
        let to = if delta < 0 {
            match from.checked_sub(1) {
                Some(t) => t,
                None => return,
            }
        } else {
            let t = from + 1;
            if t >= len {
                return;
            }
            t
        };
        let Some(slot_id) = self.queue_for_scope_mut(scope).slot_id_at(from) else {
            return;
        };
        if self.apply_queue_move_by_slot(scope, slot_id, from, to) {
            if scope == QueueScope::Remote {
                self.pending_remote_move_cursor = Some(to);
            }
            self.undo_stack_for_scope_mut(scope)
                .push(UndoEntry::Move { from, to, slot_id });
        }
    }

    /// Swaps the item at `from` to `to` within `scope`'s queue, moves the
    /// cursor to follow it, and — if this queue is also the live playback
    /// queue — tells the player to make the same move in its own internal
    /// queue copy (mirroring how active-playback removals keep that copy in
    /// sync). Returns
    /// `false` (no-op) if `from`/`to` are out of bounds or equal.
    pub(super) fn apply_queue_move(&mut self, scope: QueueScope, from: usize, to: usize) -> bool {
        let Some(slot_id) = self.queue_for_scope_mut(scope).slot_id_at(from) else {
            return false;
        };
        self.apply_queue_move_by_slot(scope, slot_id, from, to)
    }

    fn apply_queue_move_by_slot(
        &mut self,
        scope: QueueScope,
        slot_id: mbv_core::playback_queue::QueueSlotId,
        from: usize,
        to: usize,
    ) -> bool {
        let len = self.queue_for_scope(scope).items.len();
        if from >= len || to >= len || from == to {
            return false;
        }
        let controls_playback_queue = self.queue_scope_is_playback(scope);
        let active = self.player.status.lock().unwrap().active;
        if !self.queue_for_scope_mut(scope).move_slot(slot_id, to) {
            return false;
        }
        if self.local_queue_metadata_applies(scope) {
            self.queue_dirty = true;
        }
        self.persist_local_queue_state_if_needed(scope);
        if controls_playback_queue && active {
            self.player.send_command(PlayerCommand::QueueMove(from, to));
        }
        true
    }

    /// Pops and reverses the most recent undoable edit in `scope`'s queue —
    /// re-inserting a removed item, or swapping a moved item back to where it
    /// came from. No-op if the undo stack for that scope is empty.
    pub(super) fn undo_last_queue_edit(&mut self, scope: QueueScope) {
        let Some(entry) = self.undo_stack_for_scope_mut(scope).pop() else {
            return;
        };
        match entry {
            UndoEntry::Remove(idx, item) => {
                let queue = self.queue_for_scope_mut(scope);
                let idx = idx.min(queue.items.len());
                queue.insert_item_at(idx, *item);
                if self.local_queue_metadata_applies(scope) {
                    self.queue_dirty = true;
                }
                self.persist_local_queue_state_if_needed(scope);
            }
            UndoEntry::Move { from, to, slot_id } => {
                let still_in_place = self.queue_for_scope(scope).slot_id_matches_at(to, slot_id);
                if !still_in_place || !self.apply_queue_move(scope, to, from) {
                    self.flash_status_high("Can't undo move: queue changed since then".into());
                    return;
                }
            }
        }
        self.set_queue_scope(scope);
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
        let scope = self.library_position_scope_for(lib_idx);
        self.clear_saved_library_position(lib_idx, scope);
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
        if !(self.view_mode == ViewMode::Power && matches!(self.power_focus, PowerFocus::Left)) {
            self.set_power_focus(PowerFocus::Queue);
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
        if !(self.view_mode == ViewMode::Power && matches!(self.power_focus, PowerFocus::Left)) {
            self.set_power_focus(PowerFocus::Queue);
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
        if self.tab_idx == 0 {
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
        } else if self.tab_idx >= 2 {
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
            let lib_idx = self.tab_idx - self.lib_tab_offset();
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

    fn power_artist_header_action_lib_idx(&self) -> Option<usize> {
        if self.view_mode == ViewMode::Power
            && matches!(self.power_focus, PowerFocus::Left)
            && self.power_left_tab > 0
        {
            Some(self.power_left_tab - 1)
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

    fn enqueue_artist_header_selection(
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

    fn play_artist_header_selection(
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
        self.tab_idx = 1;
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
                        self.set_tab(lib_idx + 2);
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
            let lib_idx = self.tab_idx - self.lib_tab_offset();
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
            if let Some(v) = self.layout.library.lib_scroll.get_mut(lib_idx) {
                *v = 0;
            }
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
            let lib_idx = self.tab_idx - self.lib_tab_offset();
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
                if let Some(v) = self.layout.library.lib_scroll.get_mut(lib_idx) {
                    *v = 0;
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
            if self.libs[lib_idx].search.is_none()
                && (self.is_album_level(lib_idx) || in_track_focus_mode)
            {
                // Legacy `is_album_level` drilldown sources its track list
                // from the pushed nav_stack level (unchanged); the new
                // inline track-selection mode (#145 task 4) sources it from
                // the proactively-fetched `album_tracks_cache` instead,
                // keyed by the selected album's id. Kept as a separate
                // if/else picking the source `Vec<MediaItem>` rather than
                // merging the two paths, so the well-tested legacy path
                // stays byte-for-byte unchanged.
                let level_items = if self.is_album_level(lib_idx) {
                    self.libs[lib_idx]
                        .nav_stack
                        .last()
                        .map(|l| l.items.clone())
                        .unwrap_or_default()
                } else {
                    self.selected_album_item(lib_idx)
                        .and_then(|album| self.album_tracks_cache.get(&album.id).cloned())
                        .unwrap_or_default()
                };
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

    pub(super) fn go_back(&mut self) {
        if self.tab_idx > 1 {
            let lib_off = self.lib_tab_offset();
            let lib_idx = self.tab_idx - lib_off;

            // Guard: don't pop when already at the root of a synthetic "group" view
            // (music groups: nav_stack[0]=groups, nav_stack[1]=albums; feed home
            // videos: nav_stack[0]=folders, nav_stack[1]=grouped videos) -- there is
            // no list above to go back to. Search-clearing still falls through
            // because this guard only fires when search is None.
            if self.view_mode == ViewMode::Power
                && self.power_left_tab == lib_idx + 1
                && self.libs[lib_idx].search.is_none()
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
                if let Some(v) = self.layout.library.lib_scroll.get_mut(lib_idx) {
                    *v = 0;
                }
                self.save_default_library_position(lib_idx);

                // In Power View, skip past the auto-pushed Season level so
                // a single Escape takes the user back to the series list.
                if self.view_mode == ViewMode::Power && self.power_left_tab == lib_idx + 1 {
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
            }
            self.save_default_library_position(lib_idx);
        }
    }
    pub(super) fn execute_context_action(&mut self, action: Option<ContextAction>) {
        match action {
            Some(ContextAction::Play) => {
                if self.view_mode == ViewMode::Power
                    && matches!(self.power_focus, PowerFocus::Left)
                    && self.power_left_tab == 0
                {
                    self.power_cw_play();
                } else if self.tab_idx == 0 {
                    self.select_home();
                } else if self.tab_idx == 1 {
                    // Was its own third copy of queue-cursor activation, with
                    // a subtly narrower `else` branch than the keyboard/mouse
                    // paths (no seek-to-start for an already-playing audio
                    // item) -- now the same seam as Enter on the queue tab
                    // and a queue-row double-click (see #134's follow-up).
                    self.dispatch(super::action::Command::QueuePlayCursor);
                } else {
                    self.select();
                }
            }
            Some(ContextAction::PlayFolder(id)) => {
                let ct = if self.tab_idx > 1 {
                    self.libs[self.tab_idx - self.lib_tab_offset()]
                        .library
                        .collection_type
                        .clone()
                } else {
                    String::new()
                };
                self.queue_source = crate::config::QueueSource::Collection {
                    collection_type: ct,
                };
                self.play_folder(&id);
                self.save_queue_state();
            }
            Some(ContextAction::ShuffleFolder(id)) => {
                self.shuffle_folder(&id);
            }
            Some(ContextAction::PlayArtistHeader(selection)) => {
                if let Some(lib_idx) = self.power_artist_header_action_lib_idx() {
                    self.play_artist_header_selection(lib_idx, &selection, false);
                }
            }
            Some(ContextAction::ShuffleArtistHeader(selection)) => {
                if let Some(lib_idx) = self.power_artist_header_action_lib_idx() {
                    self.play_artist_header_selection(lib_idx, &selection, true);
                }
            }
            Some(ContextAction::Enqueue) => {
                if self.view_mode == ViewMode::Power
                    && matches!(self.power_focus, PowerFocus::Left)
                    && self.power_left_tab == 0
                {
                    self.power_cw_enqueue();
                } else {
                    self.enqueue_selected();
                }
            }
            Some(ContextAction::EnqueueArtistHeader(selection)) => {
                if let Some(lib_idx) = self.power_artist_header_action_lib_idx() {
                    self.enqueue_artist_header_selection(lib_idx, &selection);
                }
            }
            Some(ContextAction::EnqueueFolder(item)) => self.do_enqueue_folder((*item).clone()),
            Some(ContextAction::MarkPlayed(id)) => self.context_set_played(&id, true),
            Some(ContextAction::MarkItemsPlayed(ids)) => self.context_set_many_played(&ids),
            Some(ContextAction::MarkUnplayed(id)) => self.context_set_played(&id, false),
            Some(ContextAction::MarkItemsUnplayed(ids)) => self.context_set_many_unplayed(&ids),
            Some(ContextAction::RemoveFromContinueWatching) => self.remove_from_continue_watching(),
            Some(ContextAction::RemoveFromQueue(pos)) => self.remove_from_queue(pos),
            Some(ContextAction::GoToLibrary(item_id, item_type)) => {
                let libs: Vec<(usize, String, String)> = self
                    .libs
                    .iter()
                    .enumerate()
                    .map(|(i, lib)| {
                        (
                            i,
                            lib.library.id.clone(),
                            lib.library.collection_type.clone(),
                        )
                    })
                    .collect();
                self.spawn_navigate_to_item(item_id, item_type, libs);
            }
            None => {}
        }
    }

    fn context_set_many_played(&mut self, item_ids: &[String]) {
        let client = self.client.lock().unwrap();
        let result = item_ids
            .iter()
            .try_for_each(|item_id| client.mark_played(item_id));
        drop(client);
        match result {
            Ok(()) => self.refresh_lib(),
            Err(e) => self.flash_status_high(format!("Error: {e}")),
        }
    }

    fn context_set_many_unplayed(&mut self, item_ids: &[String]) {
        let client = self.client.lock().unwrap();
        let result = item_ids
            .iter()
            .try_for_each(|item_id| client.mark_unplayed(item_id));
        drop(client);
        match result {
            Ok(()) => self.refresh_lib(),
            Err(e) => self.flash_status_high(format!("Error: {e}")),
        }
    }

    fn context_set_played(&mut self, item_id: &str, played: bool) {
        let client = self.client.lock().unwrap();
        let result = if played {
            client.mark_played(item_id)
        } else {
            client.mark_unplayed(item_id)
        };
        drop(client);
        match result {
            Ok(()) => {
                if played {
                    let lib_idx_opt = if self.tab_idx >= self.lib_tab_offset() {
                        Some(self.tab_idx - self.lib_tab_offset())
                    } else if self.view_mode == ViewMode::Power
                        && matches!(self.power_focus, PowerFocus::Left)
                        && self.power_left_tab > 0
                    {
                        Some(self.power_left_tab - 1)
                    } else {
                        None
                    };
                    if let Some(lib_idx) = lib_idx_opt {
                        if self.is_feed_home_video_group_view(lib_idx) {
                            if let Some(state) = self
                                .libs
                                .get_mut(lib_idx)
                                .and_then(|lib| lib.feed_home_video.as_mut())
                            {
                                state.loading = true;
                            }
                            self.remove_item_from_feed_home_video_cache(lib_idx, item_id);
                            self.log_feed_home_video_state(lib_idx, "context_set_played_feed");
                        } else if let Some(lvl) = self
                            .libs
                            .get_mut(lib_idx)
                            .and_then(|l| l.nav_stack.last_mut())
                        {
                            if lvl.unplayed_only {
                                let id = item_id.to_string();
                                lvl.items.retain(|i| i.id != id);
                                lvl.total_count = lvl.total_count.saturating_sub(1);
                                lvl.cursor = lvl.cursor.min(lvl.items.len().saturating_sub(1));
                            }
                        }
                    }
                }
                if self.tab_idx == 0 {
                    let _ = self.fetch_home();
                } else {
                    self.refresh_lib();
                }
            }
            Err(e) => self.flash_status_high(format!("Error: {e}")),
        }
    }

    pub(super) fn remove_from_continue_watching(&mut self) {
        let Some(item) = self
            .home
            .continue_items
            .get(self.home.continue_cursor)
            .cloned()
        else {
            return;
        };
        let client = self.client.lock().unwrap();
        let result = client.hide_from_resume(&item.id);
        drop(client);
        match result {
            Ok(()) => {
                let _ = self.fetch_home();
            }
            Err(e) => self.flash_status_high(format!("Error: {e}")),
        }
    }

    pub(super) fn toggle_watched_home(&mut self) {
        let Some(item) = self.current_home_item() else {
            return;
        };
        if item.is_folder || item.is_audio() {
            return;
        }
        let client = self.client.lock().unwrap();
        let result = if item.played {
            client.mark_unplayed(&item.id)
        } else {
            client.mark_played(&item.id)
        };
        drop(client);
        match result {
            Ok(()) => {
                let _ = self.fetch_home();
            }
            Err(e) => self.flash_status_high(format!("Error: {e}")),
        }
    }

    pub(super) fn toggle_watched(&mut self) {
        let Some(item) = self.current_lib_item() else {
            return;
        };
        if item.is_folder || item.is_audio() {
            return;
        }
        let client = self.client.lock().unwrap();
        let result = if item.played {
            client.mark_unplayed(&item.id)
        } else {
            client.mark_played(&item.id)
        };
        drop(client);
        match result {
            Ok(()) => {
                if !item.played {
                    let lib_idx = self.tab_idx - self.lib_tab_offset();
                    if self.is_feed_home_video_group_view(lib_idx) {
                        if let Some(state) = self.libs[lib_idx].feed_home_video.as_mut() {
                            state.loading = true;
                        }
                        self.remove_item_from_feed_home_video_cache(lib_idx, &item.id);
                        self.log_feed_home_video_state(lib_idx, "toggle_watched_feed");
                    } else if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut() {
                        if lvl.unplayed_only {
                            lvl.items.remove(lvl.cursor);
                            lvl.total_count = lvl.total_count.saturating_sub(1);
                            lvl.cursor = lvl.cursor.min(lvl.items.len().saturating_sub(1));
                        }
                    }
                }
                self.refresh_lib();
            }
            Err(e) => self.flash_status_high(format!("Error: {e}")),
        }
    }

    pub(super) fn refresh_lib(&mut self) {
        let lib_idx = if self.tab_idx > 1 {
            self.tab_idx - self.lib_tab_offset()
        } else if self.view_mode == ViewMode::Power
            && matches!(self.power_focus, PowerFocus::Left)
            && self.power_left_tab > 0
        {
            self.power_left_tab - 1
        } else {
            return;
        };
        self.start_album_index(lib_idx, true);
        let scope = self.library_position_scope_for(lib_idx);
        self.clear_saved_library_position(lib_idx, scope);
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
        if self.tab_idx == 0 {
            if let Err(e) = self.fetch_home() {
                self.flash_status_high(format!("Refresh error: {e}"));
            }
        } else if self.tab_idx == 1 {
            self.refresh_queue();
        } else {
            self.refresh_lib();
        }
    }

    pub(super) fn shuffle_play(&mut self) {
        if self.tab_idx <= 1 {
            return;
        }
        if self.play_selected_artist_header(true) {
            return;
        }
        let lib_idx = self.tab_idx - self.lib_tab_offset();
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
                self.tab_idx = 1;
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
    /// Same bounds-check-then-delegate shape as `is_in_podcast_library`,
    /// and correct in both the standard and Power views since
    /// `lib_tab_offset()` is view-mode-independent.
    ///
    /// Caveat: this reads the *active tab*, not the folder actually being
    /// shuffled -- `shuffle_folder`'s `folder_id` argument is not consulted
    /// here. That's fine for its two current callers (`shuffle_play`, only
    /// reachable once `tab_idx` is already on a library tab; and the
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
        let lib_off = self.lib_tab_offset();
        if self.tab_idx < lib_off {
            return false;
        }
        let lib_idx = self.tab_idx - lib_off;
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
                self.tab_idx = 1;
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

    pub(super) fn set_tab(&mut self, idx: usize) {
        if idx != self.tab_idx && !self.card_image_states.is_empty() {
            self.force_clear = true;
        }
        self.tab_idx = idx;
        self.ensure_tab_visible();
        if self.tab_idx == 0 {
            self.home.section = 0;
            let _ = self.fetch_home();
        } else if self.tab_idx == 1 {
            if self.view_mode == ViewMode::Power && self.power_left_tab > 0 {
                self.activate_library_position_scope(
                    self.power_left_tab - 1,
                    LibraryPositionScope::Power,
                );
            }
        } else {
            self.activate_library_position_scope(
                self.tab_idx - self.lib_tab_offset(),
                LibraryPositionScope::Default,
            );
        }
    }

    /// Single owner of every *runtime* view-mode transition (issue #275):
    /// both the `'v'` key and the F2 Settings "View mode" row call this, so
    /// entry/exit side effects (focus, scope restoration, config
    /// persistence) happen exactly once, in exactly one place, regardless of
    /// which entry point triggered the switch.
    pub(super) fn set_view_mode(&mut self, mode: ViewMode) {
        if mode == self.view_mode {
            return;
        }
        match mode {
            ViewMode::Power => {
                self.pre_power_tab = self.tab_idx;
                self.set_power_focus(PowerFocus::Left);
                if self.power_left_tab > 0 {
                    self.activate_library_position_scope(
                        self.power_left_tab - 1,
                        LibraryPositionScope::Power,
                    );
                }
            }
            ViewMode::Standard => {
                // set_tab already owns library-position-scope restoration.
                self.set_tab(self.pre_power_tab);
            }
        }
        self.view_mode = mode;
        if !self.card_image_states.is_empty() {
            self.force_clear = true;
        }
        self.save_config_view_mode();
    }

    /// Persists the current `view_mode` into `config.toml` (issue #275),
    /// replacing the old `prefs.json["playlist_view"]` scheme.
    fn save_config_view_mode(&mut self) {
        let cfg = {
            let mut c = self.client.lock().unwrap();
            c.config.view_mode = match self.view_mode {
                ViewMode::Power => "power".to_string(),
                ViewMode::Standard => "standard".to_string(),
            };
            c.config.clone()
        };
        if let Err(e) = crate::config::save_config_settings(&cfg) {
            log::warn!(target: "config", "view_mode config save failed: {e}");
        }
    }

    pub(super) fn ensure_lib_loaded_for(&mut self, idx: usize) {
        if idx >= self.libs.len() {
            return;
        }
        let scope = self.library_position_scope_for(idx);
        if self.view_mode == ViewMode::Power
            && self.power_left_tab == idx + 1
            && self.is_feed_home_video_library(idx)
        {
            self.ensure_feed_home_video_root_loaded(idx);
            return;
        }
        if self.view_mode == ViewMode::Power
            && self.power_left_tab == idx + 1
            && self.is_podcast_library(idx)
        {
            self.ensure_podcast_root_loaded(idx);
            return;
        }
        if self.libs[idx].nav_stack.is_empty() {
            if let Some(saved) = self.saved_library_position(idx, scope) {
                if let Some(root) = saved.levels.first() {
                    self.libs[idx].nav_stack.push(BrowseLevel {
                        parent_id: root.parent_id.clone(),
                        title: root.title.clone(),
                        items: Vec::new(),
                        total_count: 0,
                        cursor: 0,
                        item_types: root.item_types.clone(),
                        unplayed_only: root.unplayed_only,
                        sort_by: root.sort_by.clone(),
                        sort_order: root.sort_order.clone(),
                        loading: true,
                        scroll: 0,
                        all_items: None,
                        letter_filter: None,
                    });
                    self.spawn_restore_library_position(idx, scope, saved);
                    return;
                }
            }
            let lib_id = self.libs[idx].library.id.clone();
            let lib_name = self.libs[idx].library.name.clone();
            let is_feed_view = {
                let c = self.client.lock().unwrap();
                c.config
                    .feed_view_libraries
                    .contains(&lib_name.to_lowercase())
            };
            let (item_types, unplayed_only, sort_by, sort_order) =
                match self.libs[idx].library.collection_type.as_str() {
                    "movies" => (Some("Movie".to_string()), false, "SortName", "Ascending"),
                    _ if is_feed_view => {
                        (Some("Video".to_string()), true, "DateCreated", "Ascending")
                    }
                    _ => (None, false, "SortName", "Ascending"),
                };
            self.libs[idx].nav_stack.push(BrowseLevel {
                parent_id: lib_id.clone(),
                title: lib_name.clone(),
                items: vec![],
                total_count: 0,
                cursor: 0,
                item_types: item_types.clone(),
                unplayed_only,
                sort_by: sort_by.into(),
                sort_order: sort_order.into(),
                loading: true,
                scroll: 0,
                all_items: None,
                letter_filter: None,
            });
            self.spawn_browse(
                idx,
                lib_id,
                lib_name,
                item_types,
                unplayed_only,
                sort_by.into(),
                sort_order.into(),
            );
        }
    }

    pub(super) fn spawn_restore_library_position(
        &self,
        lib_idx: usize,
        scope: LibraryPositionScope,
        saved: crate::config::LibraryPosition,
    ) {
        let visible_rows = self.lib_page_size();
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let restored = super::restore_library_position(&saved, visible_rows, |saved_level| {
                let letter_filter = saved_level
                    .letter_filter_index
                    .and_then(super::render::power::LetterFilter::for_index);
                let (name_ge, name_lt) = letter_filter
                    .as_ref()
                    .map(|f| (f.name_ge, f.name_lt))
                    .unwrap_or((None, None));
                let (items, total_count) = client.get_items_sorted_ranged(
                    &saved_level.parent_id,
                    saved_level.item_types.as_deref(),
                    saved_level.unplayed_only,
                    0,
                    PAGE_SIZE,
                    &saved_level.sort_by,
                    &saved_level.sort_order,
                    name_ge,
                    name_lt,
                )?;
                if total_count > items.len() {
                    client.get_items_sorted_ranged(
                        &saved_level.parent_id,
                        saved_level.item_types.as_deref(),
                        saved_level.unplayed_only,
                        0,
                        total_count,
                        &saved_level.sort_by,
                        &saved_level.sort_order,
                        name_ge,
                        name_lt,
                    )
                } else {
                    Ok((items, total_count))
                }
            });
            match restored {
                Ok(Some((position, nav_stack))) => {
                    let _ = tx.send(LibEvent::RestoreLibraryPosition {
                        lib_idx,
                        scope,
                        requested_position: saved,
                        position,
                        nav_stack,
                    });
                }
                Ok(None) => {}
                Err(e) => {
                    let _ = tx.send(LibEvent::Error(e));
                }
            }
        });
    }

    pub(super) fn refresh_after_stop(&mut self) {
        let _ = self.fetch_home();
        if self.last_played_completed {
            if let Some(ref item_id) = self.last_played_item_id.clone() {
                for lib_idx in 0..self.libs.len() {
                    if self.is_feed_home_video_group_view(lib_idx)
                        || self.is_feed_home_video_library(lib_idx)
                    {
                        self.remove_item_from_feed_home_video_cache(lib_idx, item_id);
                        if let Some(state) = self.libs[lib_idx].feed_home_video.as_mut() {
                            state.loading = true;
                        }
                        self.log_feed_home_video_state(lib_idx, "refresh_after_stop_completed");
                    }
                }
            }
        }
        let fetches: Vec<BrowseRefresh> = self
            .libs
            .iter()
            .enumerate()
            .filter_map(|(i, lib)| {
                lib.nav_stack.last().map(|lvl| {
                    (
                        i,
                        lvl.parent_id.clone(),
                        lvl.item_types.clone(),
                        lvl.unplayed_only,
                        lvl.sort_by.clone(),
                        lvl.sort_order.clone(),
                        lvl.items.len(),
                        lvl.letter_filter.clone(),
                    )
                })
            })
            .collect();
        for (
            lib_idx,
            parent_id,
            item_types,
            unplayed_only,
            sort_by,
            sort_order,
            loaded_count,
            letter_filter,
        ) in fetches
        {
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

    pub(super) fn spawn_browse(
        &self,
        lib_idx: usize,
        parent_id: String,
        title: String,
        item_types: Option<String>,
        unplayed_only: bool,
        sort_by: String,
        sort_order: String,
    ) {
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        let spawn_started = std::time::Instant::now();
        std::thread::spawn(move || {
            match client.get_items_sorted(
                &parent_id,
                item_types.as_deref(),
                unplayed_only,
                0,
                PAGE_SIZE,
                &sort_by,
                &sort_order,
            ) {
                Ok((items, total_count)) => {
                    log::info!(target: "browse", "Loaded lib_idx={lib_idx} parent={parent_id} total={total_count} got={} thread_total={}ms first3={:?}",
                        items.len(),
                        spawn_started.elapsed().as_millis(),
                        items.iter().take(3).map(|i| format!("{}:{}", i.id, i.name)).collect::<Vec<_>>());
                    let _ = tx.send(LibEvent::Loaded {
                        lib_idx,
                        parent_id: parent_id.clone(),
                        level: BrowseLevel {
                            parent_id,
                            title,
                            items,
                            total_count,
                            cursor: 0,
                            item_types,
                            unplayed_only,
                            sort_by,
                            sort_order,
                            loading: false,
                            scroll: 0,
                            all_items: None,
                            letter_filter: None,
                        },
                    });
                }
                Err(e) => {
                    let _ = tx.send(LibEvent::Error(e));
                }
            }
        });
    }

    pub(super) fn spawn_navigate_to_item(
        &self,
        item_id: String,
        item_type: String,
        libs: Vec<(usize, String, String)>,
    ) {
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            // Match library by collection_type since CollectionFolder IDs never appear in ancestors
            let target_ctype = match item_type.as_str() {
                "Series" | "Episode" | "Season" => "tvshows",
                "Movie" => "movies",
                "Audio" | "MusicAlbum" | "MusicArtist" => "music",
                _ => "",
            };
            let (lib_idx, lib_id) = match libs.iter().find(|(_, _, ctype)| ctype == target_ctype) {
                Some((idx, id, _)) => (*idx, id.clone()),
                None => {
                    let _ = tx.send(LibEvent::Error(
                        "No matching library for this item type".into(),
                    ));
                    return;
                }
            };

            // Ancestors are ordered nearest→root: [Season, Series, physical_folder, AggregateFolder]
            let ancestors = match client.get_ancestors(&item_id) {
                Ok(a) => a,
                Err(e) => {
                    log::error!(target:"navigate", "get_ancestors failed: {e}");
                    let _ = tx.send(LibEvent::Error(e));
                    return;
                }
            };
            log::debug!(target:"navigate", "ancestors: {:?}", ancestors.iter().map(|a| format!("{}({})", a.name, a.id)).collect::<Vec<_>>());

            // Drop the last two ancestors (physical library folder + AggregateFolder root);
            // everything before those is navigable content inside the library.
            let inside = if ancestors.len() >= 2 {
                &ancestors[..ancestors.len() - 2]
            } else {
                &ancestors[..0]
            };

            // Build nav levels: lib_id first, then inside ancestors from root→item, then item itself.
            // inside is nearest→root order; we need root→item, so iterate reversed.
            let mut parents: Vec<String> = vec![lib_id];
            for a in inside.iter().rev() {
                parents.push(a.id.clone());
            }

            // targets[i] is the item we want the cursor on inside parents[i]
            let mut targets: Vec<String> =
                inside.iter().rev().skip(1).map(|a| a.id.clone()).collect();
            if let Some(a) = inside.first() {
                targets.push(a.id.clone());
            } // last inside level → first inside ancestor
            targets.push(item_id.clone()); // deepest level → the item itself

            let mut nav_stack: Vec<BrowseLevel> = Vec::new();
            for (parent_id, target_id) in parents.into_iter().zip(targets) {
                let (mut items, total_count) = match client.get_items_sorted(
                    &parent_id,
                    None,
                    false,
                    0,
                    500,
                    "SortName",
                    "Ascending",
                ) {
                    Ok(x) => x,
                    Err(e) => {
                        let _ = tx.send(LibEvent::Error(e));
                        return;
                    }
                };
                if items
                    .first()
                    .map(|it| it.item_type == "Episode")
                    .unwrap_or(false)
                {
                    sort_episodes(&mut items);
                }
                let cursor = items.iter().position(|it| it.id == target_id).unwrap_or(0);
                log::debug!(target:"navigate", "level parent={parent_id} target={target_id} cursor={cursor}/{}", items.len());
                nav_stack.push(BrowseLevel {
                    parent_id: parent_id.clone(),
                    title: String::new(),
                    items,
                    total_count,
                    cursor,
                    item_types: None,
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    loading: false,
                    scroll: 0,
                    all_items: None,
                    letter_filter: None,
                });
            }
            let _ = tx.send(LibEvent::NavigateTo {
                lib_idx,
                nav_stack,
                switch_tab: true,
            });
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_browse_page(
        &self,
        lib_idx: usize,
        parent_id: String,
        start_index: usize,
        item_types: Option<String>,
        unplayed_only: bool,
        sort_by: String,
        sort_order: String,
        letter_filter: Option<super::render::power::LetterFilter>,
    ) {
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        let (name_ge, name_lt) = letter_filter
            .as_ref()
            .map(|f| (f.name_ge, f.name_lt))
            .unwrap_or((None, None));
        std::thread::spawn(move || {
            match client.get_items_sorted_ranged(
                &parent_id,
                item_types.as_deref(),
                unplayed_only,
                start_index,
                PAGE_SIZE,
                &sort_by,
                &sort_order,
                name_ge,
                name_lt,
            ) {
                Ok((items, total_count)) => {
                    let _ = tx.send(LibEvent::PageAppended {
                        lib_idx,
                        parent_id,
                        items,
                        total_count,
                    });
                }
                Err(e) => {
                    let _ = tx.send(LibEvent::Error(e));
                }
            }
        });
    }

    fn spawn_all_items_prefetch(&self, lib_idx: usize) {
        let lib = &self.libs[lib_idx];
        let lvl = match lib.nav_stack.last() {
            Some(l) => l,
            None => return,
        };
        // `lvl.is_fully_loaded()` compares `items.len()` against
        // `lvl.total_count` -- with a letter-range pill active, that count
        // is the FILTERED range's total, not the whole library's, so a
        // fully-loaded small range (e.g. 40 items in `A–C`) would wrongly
        // read as "nothing more to prefetch". `all_items` backs whole-library
        // search (see `input.rs`'s `/` handler and `spawn_search_items_load`
        // below), so it must never be satisfied by just the active range.
        if lvl.letter_filter.is_none() && lvl.is_fully_loaded() {
            return;
        }
        let parent_id = lvl.parent_id.clone();
        let total_count = full_library_fetch_limit(lib, lvl);
        let item_types = lvl.item_types.clone();
        let unplayed_only = lvl.unplayed_only;
        let sort_by = lvl.sort_by.clone();
        let sort_order = lvl.sort_order.clone();
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            if let Ok((items, _)) = client.get_items_sorted(
                &parent_id,
                item_types.as_deref(),
                unplayed_only,
                0,
                total_count,
                &sort_by,
                &sort_order,
            ) {
                let _ = tx.send(LibEvent::AllItemsPrefetched {
                    lib_idx,
                    parent_id,
                    items,
                });
            }
        });
    }

    pub(super) fn spawn_search_items_load(&self, lib_idx: usize) {
        let lib = &self.libs[lib_idx];
        let lvl = match lib.nav_stack.last() {
            Some(l) => l,
            None => return,
        };
        let parent_id = lvl.parent_id.clone();
        // See `spawn_all_items_prefetch` above: always fetch the WHOLE
        // library unfiltered so search covers everything, not just an
        // active letter-range pill's slice.
        let total_count = full_library_fetch_limit(lib, lvl);
        let item_types = lvl.item_types.clone();
        let unplayed_only = lvl.unplayed_only;
        let sort_by = lvl.sort_by.clone();
        let sort_order = lvl.sort_order.clone();
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            if let Ok((items, _)) = client.get_items_sorted(
                &parent_id,
                item_types.as_deref(),
                unplayed_only,
                0,
                total_count,
                &sort_by,
                &sort_order,
            ) {
                let _ = tx.send(LibEvent::SearchItemsLoaded {
                    lib_idx,
                    parent_id,
                    items,
                });
            }
        });
    }

    pub(super) fn recursive_album_search_enabled(&self, lib_idx: usize) -> bool {
        self.libs.get(lib_idx).is_some_and(|lib| {
            recursive_album_search_eligible(&lib.library.collection_type, &self.music_levels)
        })
    }

    pub(super) fn start_album_index(&mut self, lib_idx: usize, refresh: bool) {
        if !self.recursive_album_search_enabled(lib_idx) {
            return;
        }
        let library_id = self.libs[lib_idx].library.id.clone();
        let should_spawn = match self.album_indexes.get_mut(&library_id) {
            None => {
                self.album_indexes.insert(
                    library_id.clone(),
                    AlbumIndexState::Loading {
                        rebuild_pending: false,
                    },
                );
                true
            }
            Some(AlbumIndexState::Loading { rebuild_pending }) if refresh => {
                *rebuild_pending = true;
                false
            }
            Some(state) if refresh => {
                *state = AlbumIndexState::Loading {
                    rebuild_pending: false,
                };
                true
            }
            Some(_) => false,
        };
        if refresh {
            self.sync_recursive_album_search(lib_idx);
        }
        if should_spawn {
            self.spawn_album_index_build(library_id);
        }
    }

    fn spawn_album_index_build(&self, library_id: String) {
        let client = self.client.lock().unwrap().clone();
        let levels = self.music_levels.clone();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let mut fetch = |parent_id: &str, start: usize, limit: usize| {
                client.get_items_sorted(
                    parent_id,
                    None,
                    false,
                    start,
                    limit,
                    "SortName",
                    "Ascending",
                )
            };
            let result = build_album_index_with(&library_id, &levels, &mut fetch);
            let _ = tx.send(LibEvent::AlbumIndexBuilt { library_id, result });
        });
    }

    pub(super) fn open_recursive_album_search(&mut self, lib_idx: usize) -> bool {
        if !self.recursive_album_search_enabled(lib_idx) {
            return false;
        }
        self.libs[lib_idx].search = Some(super::LibSearch {
            query: String::new(),
            items: Vec::new(),
            results: Vec::new(),
            cursor: 0,
            scroll: 0,
            loading: false,
        });
        self.sync_recursive_album_search(lib_idx);
        true
    }

    fn sync_recursive_album_search(&mut self, lib_idx: usize) {
        if !self.recursive_album_search_enabled(lib_idx) || self.libs[lib_idx].search.is_none() {
            return;
        }
        let library_id = self.libs[lib_idx].library.id.clone();
        let (items, loading) = match self.album_indexes.get(&library_id) {
            Some(AlbumIndexState::Ready(entries)) => (
                entries.iter().map(|entry| entry.album.clone()).collect(),
                false,
            ),
            Some(AlbumIndexState::Loading { .. }) => (Vec::new(), true),
            _ => (Vec::new(), false),
        };
        if let Some(search) = self.libs[lib_idx].search.as_mut() {
            search.items = items;
            search.loading = loading;
        }
        self.update_lib_search(lib_idx);
    }

    pub(super) fn recursive_album_search_entry(&self, lib_idx: usize) -> Option<AlbumSearchEntry> {
        if !self.recursive_album_search_enabled(lib_idx) {
            return None;
        }
        let lib = self.libs.get(lib_idx)?;
        let search = lib.search.as_ref()?;
        let item_idx = *search.results.get(search.cursor)?;
        let entries = match self.album_indexes.get(&lib.library.id)? {
            AlbumIndexState::Ready(entries) => entries,
            _ => return None,
        };
        entries.get(item_idx).cloned()
    }

    pub(super) fn activate_recursive_album(
        &mut self,
        lib_idx: usize,
        scope: LibraryPositionScope,
    ) -> bool {
        let Some(entry) = self.recursive_album_search_entry(lib_idx) else {
            return false;
        };
        let library_id = self.libs[lib_idx].library.id.clone();
        let library_name = self.libs[lib_idx].library.display_name();
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let fetch = |parent_id: &str| {
                let mut call = |id: &str, start: usize, limit: usize| {
                    client.get_items_sorted(id, None, false, start, limit, "SortName", "Ascending")
                };
                fetch_all_album_index_items(parent_id, &mut call)
            };
            let mut parents = vec![(library_id.clone(), library_name)];
            parents.extend(
                entry
                    .ancestors
                    .iter()
                    .map(|part| (part.id.clone(), part.name.clone())),
            );
            let mut targets: Vec<String> =
                entry.ancestors.iter().map(|part| part.id.clone()).collect();
            targets.push(entry.album.id.clone());
            let mut nav_stack = Vec::new();
            for ((parent_id, title), target_id) in parents.into_iter().zip(targets) {
                let items = match fetch(&parent_id) {
                    Ok(items) => items,
                    Err(error) => {
                        let _ = tx.send(LibEvent::Error(error));
                        return;
                    }
                };
                let total_count = items.len();
                let Some(cursor) = items.iter().position(|item| item.id == target_id) else {
                    let _ = tx.send(LibEvent::Error(format!(
                        "Album path changed before activation: missing {target_id}"
                    )));
                    return;
                };
                nav_stack.push(BrowseLevel {
                    parent_id,
                    title,
                    items,
                    total_count,
                    cursor,
                    item_types: None,
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    loading: false,
                    scroll: 0,
                    all_items: None,
                    letter_filter: None,
                });
            }
            if scope == LibraryPositionScope::Default {
                let items = match fetch(&entry.album.id) {
                    Ok(items) => items,
                    Err(error) => {
                        let _ = tx.send(LibEvent::Error(error));
                        return;
                    }
                };
                nav_stack.push(BrowseLevel {
                    parent_id: entry.album.id.clone(),
                    title: entry.album.display_name(),
                    total_count: items.len(),
                    items,
                    cursor: 0,
                    item_types: None,
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    loading: false,
                    scroll: 0,
                    all_items: None,
                    letter_filter: None,
                });
            }
            let _ = tx.send(LibEvent::RecursiveAlbumActivated {
                library_id,
                scope,
                nav_stack,
            });
        });
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_refresh(
        &self,
        lib_idx: usize,
        parent_id: String,
        item_types: Option<String>,
        unplayed_only: bool,
        sort_by: String,
        sort_order: String,
        loaded_count: usize,
        letter_filter: Option<super::render::power::LetterFilter>,
    ) {
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        let limit = loaded_count.max(PAGE_SIZE);
        let (name_ge, name_lt) = letter_filter
            .as_ref()
            .map(|f| (f.name_ge, f.name_lt))
            .unwrap_or((None, None));
        std::thread::spawn(move || {
            match client.get_items_sorted_ranged(
                &parent_id,
                item_types.as_deref(),
                unplayed_only,
                0,
                limit,
                &sort_by,
                &sort_order,
                name_ge,
                name_lt,
            ) {
                Ok((items, total_count)) => {
                    log::info!(target: "browse", "Refreshed lib_idx={lib_idx} parent={parent_id} total={total_count} got={} first3={:?}",
                        items.len(),
                        items.iter().take(3).map(|i| format!("{}:{}", i.id, i.name)).collect::<Vec<_>>());
                    let _ = tx.send(LibEvent::Refreshed {
                        lib_idx,
                        parent_id,
                        item_types,
                        unplayed_only,
                        items,
                        total_count,
                    });
                }
                Err(e) => {
                    let _ = tx.send(LibEvent::Error(e));
                }
            }
        });
    }

    pub(in crate::app) fn maybe_fetch_next_page(&mut self, lib_idx: usize) {
        let lib = &self.libs[lib_idx];
        if lib.search.is_some() {
            return;
        }
        let lvl = match lib.nav_stack.last() {
            Some(l) => l,
            None => return,
        };
        if lvl.loading {
            return;
        }
        if lvl.is_fully_loaded() {
            return;
        }
        // The root folder listing of a feed-home-video library isn't scrolled by
        // the user directly -- it's aggregated in the background into grouped
        // sections, and that aggregation can't start until every page has
        // loaded. Waiting for the cursor to approach the loaded edge (as normal
        // browse levels do) would stall pagination forever for libraries with
        // more folders than PAGE_SIZE + PREFETCH_AHEAD, since nothing moves the
        // cursor on that hidden level. Paginate it to completion unconditionally.
        let is_feed_home_video_root =
            lib.nav_stack.len() == 1 && self.is_feed_home_video_library(lib_idx);
        if !is_feed_home_video_root && lvl.cursor + PREFETCH_AHEAD < lvl.items.len() {
            return;
        }
        let start_index = lvl.items.len();
        let parent_id = lvl.parent_id.clone();
        let item_types = lvl.item_types.clone();
        let unplayed_only = lvl.unplayed_only;
        let sort_by = lvl.sort_by.clone();
        let sort_order = lvl.sort_order.clone();
        let letter_filter = lvl.letter_filter.clone();
        if let Some(last) = self.libs[lib_idx].nav_stack.last_mut() {
            last.loading = true;
        }
        self.spawn_browse_page(
            lib_idx,
            parent_id,
            start_index,
            item_types,
            unplayed_only,
            sort_by,
            sort_order,
            letter_filter,
        );
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

    fn normalize_current_browse_level_items(&mut self, lib_idx: usize, log_album_entry: bool) {
        let is_album = self.is_album_level(lib_idx);
        if is_album && log_album_entry {
            let title = self
                .libs
                .get(lib_idx)
                .and_then(|lib| lib.nav_stack.last())
                .map(|level| level.title.clone())
                .unwrap_or_default();
            log::debug!(target: "app", "album: entered «{title}»");
        }
        if let Some(last) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.nav_stack.last_mut())
        {
            if is_album {
                sort_audio_tracks(&mut last.items);
            }
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
                order.sort_by_key(|&i| {
                    super::render::power::initial_group_artist_sort_key(&last.items[i])
                });
                last.cursor = order[0];
            }
        }
    }

    fn handle_loaded_level(&mut self, lib_idx: usize, parent_id: String, level: BrowseLevel) {
        let mut level = Some(level);
        self.update_current_browse_level(lib_idx, &parent_id, true, |last| {
            *last = level.take().unwrap();
        });
        self.normalize_current_browse_level_items(lib_idx, true);
        self.snap_grouped_album_cursor_to_display_order(lib_idx);
    }

    fn maybe_auto_push_power_tv_season_level(&mut self, lib_idx: usize) {
        // In Power View: when a season list arrives for a TV library,
        // automatically push a loading placeholder and fetch the first season's
        // episodes so the user lands directly in the combined series view.
        let should_auto_push = self.view_mode == ViewMode::Power
            && self.power_left_tab == lib_idx + 1
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

    fn maybe_auto_push_power_music_group_level(&mut self, lib_idx: usize) {
        // In Power View: when the group list loads for a music library with
        // levels = ["group", …], automatically push the first group's album
        // level so the user lands directly in the combined group view.
        let should_auto_push_music = self.view_mode == ViewMode::Power
            && self.power_left_tab == lib_idx + 1
            && self
                .libs
                .get(lib_idx)
                .map(|lib| {
                    lib.library.collection_type == "music"
                        && self
                            .music_levels
                            .first()
                            .map(|s| s == "group")
                            .unwrap_or(false)
                        && lib.nav_stack.len() == 1
                        && !lib.nav_stack[0].items.is_empty()
                })
                .unwrap_or(false);

        if should_auto_push_music {
            let (group_id, group_name) = self
                .libs
                .get(lib_idx)
                .and_then(|lib| lib.nav_stack.last())
                .and_then(|l| l.items.get(l.cursor))
                .map(|g| (g.id.clone(), g.name.clone()))
                .unwrap_or_default();
            if !group_id.is_empty() {
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    lib.nav_stack.push(BrowseLevel {
                        parent_id: group_id.clone(),
                        title: group_name.clone(),
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
                }
                self.spawn_browse(
                    lib_idx,
                    group_id,
                    group_name,
                    None,
                    false,
                    "SortName".into(),
                    "Ascending".into(),
                );
            }
        }
    }

    fn maybe_aggregate_feed_after_loaded(&self, lib_idx: usize) {
        let should_aggregate_feed = self.should_aggregate_feed(lib_idx, |root| {
            root.item_types.is_none() && !root.unplayed_only
        });
        if should_aggregate_feed {
            self.log_feed_home_video_state(lib_idx, "loaded_before_aggregate");
            self.spawn_feed_home_video_aggregate(lib_idx);
            self.spawn_podcast_aggregate(lib_idx);
        }
    }

    fn maybe_aggregate_feed_after_page_append(&self, lib_idx: usize, parent_id: &str) {
        let should_aggregate_feed =
            self.should_aggregate_feed(lib_idx, |root| root.parent_id == parent_id);
        if should_aggregate_feed {
            self.log_feed_home_video_state(lib_idx, "page_appended_before_aggregate");
            self.spawn_feed_home_video_aggregate(lib_idx);
            self.spawn_podcast_aggregate(lib_idx);
        }
    }

    pub(super) fn maybe_refresh_feed_groups_after_refresh(&mut self, lib_idx: usize) {
        let should_refresh_feed_groups = self
            .libs
            .get(lib_idx)
            .map(|lib| {
                self.view_mode == ViewMode::Power
                    && self.power_left_tab == lib_idx + 1
                    && (self.is_feed_home_video_library(lib_idx)
                        || self.is_podcast_library(lib_idx))
                    && lib
                        .nav_stack
                        .first()
                        .is_some_and(BrowseLevel::is_fully_loaded)
            })
            .unwrap_or(false);
        if should_refresh_feed_groups {
            if let Some(lib) = self.libs.get_mut(lib_idx) {
                let state = lib
                    .feed_home_video
                    .get_or_insert_with(FeedHomeVideoState::default);
                state.loading = true;
            }
            self.log_feed_home_video_state(lib_idx, "refreshed_before_aggregate");
            self.spawn_feed_home_video_aggregate(lib_idx);
            self.spawn_podcast_aggregate(lib_idx);
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
        if total <= super::render::power::LIBRARY_PILL_THRESHOLD {
            return;
        }
        let filter = super::render::power::LetterFilter::default_filter();
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
        self.normalize_current_browse_level_items(lib_idx, false);
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
        self.normalize_current_browse_level_items(lib_idx, false);
        self.maybe_refresh_feed_groups_after_refresh(lib_idx);
        self.spawn_all_items_prefetch(lib_idx);
    }

    fn handle_restored_library_position(
        &mut self,
        lib_idx: usize,
        scope: LibraryPositionScope,
        requested_position: crate::config::LibraryPosition,
        position: crate::config::LibraryPosition,
        nav_stack: Vec<BrowseLevel>,
    ) {
        if self.saved_library_position(lib_idx, scope).as_ref() != Some(&requested_position) {
            return;
        }
        if self.active_library_position_scope_for(lib_idx) != Some(scope) {
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
        if restored.as_ref() != self.saved_library_position(lib_idx, scope).as_ref() {
            if let Some(restored) = restored {
                self.replace_saved_library_position(lib_idx, scope, restored);
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
                scope,
                requested_position,
                position,
                nav_stack,
            } => self.handle_restored_library_position(
                lib_idx,
                scope,
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
                scope,
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
                    lib.album_track_focus = if scope == LibraryPositionScope::Power {
                        Some(0)
                    } else {
                        None
                    };
                }
                self.with_library_position_scope_override(lib_idx, scope, |app| {
                    app.save_default_library_position(lib_idx);
                });
                if scope == LibraryPositionScope::Default {
                    self.search.close();
                    self.set_tab(lib_idx + self.lib_tab_offset());
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
            LibEvent::AlbumYearFetched { album_id, year } => {
                self.album_year_loading.remove(&album_id);
                self.album_year_cache.insert(album_id, year);
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
                    let target_tab = lib_idx + self.lib_tab_offset();
                    self.set_tab(target_tab);
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

    pub(super) fn on_queue_replace_silent(&mut self) {
        self.queue_source = crate::config::QueueSource::Unknown;
        self.queue_dirty = false;
    }

    pub(super) fn replace_queue_or_prompt(&mut self, action: PendingQueueAction) {
        if self.action_touches_local_queue(&action)
            && self.queue_dirty
            && self.queue_is_saved_playlist()
        {
            self.pending_queue_action = Some(action);
            self.show_save_playlist_modal = true;
        } else {
            self.execute_pending_queue_action(action);
        }
    }

    pub(super) fn execute_pending_queue_action(&mut self, action: PendingQueueAction) {
        if self.action_touches_local_queue(&action) {
            self.queue_dirty = false;
        }
        match action {
            PendingQueueAction::PlayItems {
                items,
                start_idx,
                source,
            } => {
                let direct_remote = self.has_direct_remote_queue();
                if self.local_queue_metadata_applies(self.playback_target_queue_scope()) {
                    self.queue_source = source;
                }
                self.replace_playback_queue(items.clone(), start_idx);
                self.set_queue_scope(self.playback_target_queue_scope());
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
                } else {
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
                if !direct_remote {
                    self.save_queue_state();
                }
            }
            PendingQueueAction::ClearQueue => {
                let scope = self.visible_queue_scope();
                if self.local_queue_metadata_applies(scope) {
                    self.clear_local_queue_metadata();
                } else {
                    self.remote_queue_undo_stack.clear();
                }
                if scope == QueueScope::Remote {
                    self.replace_direct_remote_queue(Vec::new(), 0);
                } else if self.queue_scope_is_playback(scope) {
                    self.player.stop();
                }
                if scope != QueueScope::Remote {
                    let queue = self.queue_for_scope_mut(scope);
                    queue.clear();
                }
                self.persist_local_queue_state_if_needed(scope);
                self.flash_status("Queue cleared".into());
            }
        }
    }

    pub(super) fn queue_is_saved_playlist(&self) -> bool {
        matches!(
            &self.queue_source,
            crate::config::QueueSource::Playlist { id: Some(_), .. }
        )
    }

    fn queue_playlist_id(&self) -> Option<&str> {
        if let crate::config::QueueSource::Playlist {
            id: Some(ref id), ..
        } = self.queue_source
        {
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

    pub(super) fn save_playlist_to_emby(&self) {
        let Some(playlist_id) = self.queue_playlist_id() else {
            return;
        };
        let item_ids: Vec<String> = self.player_tab.items.iter().map(|i| i.id.clone()).collect();
        let client = self.client.lock().unwrap().clone();
        let playlist_id = playlist_id.to_string();
        std::thread::spawn(move || {
            if let Err(e) = client.update_playlist_items(&playlist_id, &item_ids) {
                log::error!(target: "playlist", "Failed to save playlist: {e}");
            }
        });
    }

    /// Number of selectable left-panel tabs in Power View: Home/CW + all libraries.
    pub(super) fn power_left_tab_count(&self) -> usize {
        1 + self.libs.len()
    }

    /// Advance the left-panel tab (wrapping); load the library if needed.
    pub(super) fn power_left_tab_next(&mut self) {
        let n = self.power_left_tab_count();
        self.power_left_tab = (self.power_left_tab + 1) % n;
        self.last_card_height = 0; // reset stale image height for new view
        if self.power_left_tab > 0 {
            self.set_power_focus(PowerFocus::Left);
            self.activate_library_position_scope(
                self.power_left_tab - 1,
                LibraryPositionScope::Power,
            );
        }
        self.save_prefs();
    }

    /// Retreat the left-panel tab (wrapping); load the library if needed.
    pub(super) fn power_left_tab_prev(&mut self) {
        let n = self.power_left_tab_count();
        self.power_left_tab = (self.power_left_tab + n - 1) % n;
        self.last_card_height = 0;
        if self.power_left_tab > 0 {
            self.set_power_focus(PowerFocus::Left);
            self.activate_library_position_scope(
                self.power_left_tab - 1,
                LibraryPositionScope::Power,
            );
        }
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
        let (saved_tab, saved_sec) = (self.tab_idx, self.home.section);
        self.tab_idx = 0;
        self.home.section = 0;
        self.select_home();
        self.tab_idx = saved_tab;
        self.home.section = saved_sec;
    }

    pub(super) fn power_cw_enqueue(&mut self) {
        let (saved_tab, saved_sec) = (self.tab_idx, self.home.section);
        self.tab_idx = 0;
        self.home.section = 0;
        self.enqueue_selected();
        self.tab_idx = saved_tab;
        self.home.section = saved_sec;
    }

    pub(super) fn power_cw_toggle_watched(&mut self) {
        let saved_sec = self.home.section;
        self.home.section = 0;
        self.toggle_watched_home();
        self.home.section = saved_sec;
    }

    // ── Power-view home flat list ────────────────────────────────────────────

    /// The MediaItem at the current flat `power_home_cursor`, or None.
    pub(super) fn power_home_current_item(&self) -> Option<MediaItem> {
        let cursor = self.home.power_home_cursor;
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
        self.home.power_home_scroll = 0;
        if let Some((start, len)) = self.power_home_section_range(section_idx) {
            self.home.power_home_cursor = if len == 0 {
                start
            } else {
                self.home.power_home_cursor.clamp(start, start + len - 1)
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
            self.home.power_home_cursor = 0;
            return;
        };
        let pos = indices
            .iter()
            .position(|idx| *idx == self.home.power_home_cursor)
            .unwrap_or(0);
        let next = (pos as i64 + delta).clamp(0, indices.len() as i64 - 1) as usize;
        self.home.power_home_cursor = indices[next];
    }

    pub(super) fn power_home_select_start(&mut self) {
        if let Some(first) = self.power_home_visible_indices().first() {
            self.home.power_home_cursor = *first;
        }
    }

    pub(super) fn power_home_select_end(&mut self) {
        if let Some(last) = self.power_home_visible_indices().last() {
            self.home.power_home_cursor = *last;
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
        let cursor = self.home.power_home_cursor;
        let cw_len = self.home.continue_items.len();
        if cursor < cw_len {
            // CW items: use select_home for proper resume handling.
            let (saved_tab, saved_sec, saved_cursor) =
                (self.tab_idx, self.home.section, self.home.continue_cursor);
            self.tab_idx = 0;
            self.home.section = 0;
            self.home.continue_cursor = cursor;
            self.select_home();
            self.tab_idx = saved_tab;
            self.home.section = saved_sec;
            self.home.continue_cursor = saved_cursor;
        } else {
            self.play_item(item);
        }
    }

    /// Enqueue the item under the flat power-home cursor.
    pub(super) fn power_home_enqueue(&mut self) {
        let cursor = self.home.power_home_cursor;
        let cw_len = self.home.continue_items.len();
        if cursor < cw_len {
            let (saved_tab, saved_sec, saved_cursor) =
                (self.tab_idx, self.home.section, self.home.continue_cursor);
            self.tab_idx = 0;
            self.home.section = 0;
            self.home.continue_cursor = cursor;
            self.enqueue_selected();
            self.tab_idx = saved_tab;
            self.home.section = saved_sec;
            self.home.continue_cursor = saved_cursor;
        } else {
            let Some(item) = self.power_home_current_item() else {
                return;
            };
            self.do_enqueue_folder(item);
        }
    }

    fn build_queue_state(&self) -> crate::config::QueueState {
        let positions: std::collections::HashMap<String, i64> = self
            .player_tab
            .items
            .iter()
            .filter(|i| i.playback_position_ticks > 0 && !i.is_audio())
            .map(|i| (i.id.clone(), i.playback_position_ticks))
            .collect();
        crate::config::QueueState {
            source: self.queue_source.clone(),
            items: self.player_tab.items.clone(),
            cursor: self.player_tab.queue_cursor,
            last_played_item_id: self.last_played_item_id.clone(),
            last_played_completed: self.last_played_completed,
            positions,
        }
    }

    pub(super) fn save_queue_state(&self) {
        let state = self.build_queue_state();
        if state.items.is_empty() {
            // Don't nuke the on-disk queue just because the local tab happens to be
            // empty while attached to a remote session — that reflects remote-control
            // UI state, not the user intentionally clearing their local queue.
            if self.connected_session_id.is_none() {
                crate::config::clear_queue_state();
            }
        } else {
            crate::config::save_queue_state(&state);
        }
    }

    /// Like `save_queue_state`, but never deletes the on-disk snapshot when the
    /// in-memory queue happens to be empty. Quit is not a genuine "user cleared
    /// the queue" signal — an empty `player_tab.items` at quit time can equally
    /// mean this session never touched the local queue at all, and unconditionally
    /// deleting in that case wipes a perfectly valid snapshot from an earlier
    /// session with no recovery path. Only an explicit `ClearQueue` action (which
    /// goes through `save_queue_state`) should ever delete the file.
    pub(super) fn save_queue_state_no_clear(&self) {
        let state = self.build_queue_state();
        if !state.items.is_empty() {
            crate::config::save_queue_state(&state);
        }
    }

    /// Restore the queue from disk immediately and synchronously — the file
    /// already holds full `MediaItem`s, so this is a local read, no network
    /// round-trip, no in-flight window where the queue could be superseded
    /// by a real user action before it lands. See `spawn_enrich_queue_state`
    /// for the separate, best-effort refresh of played/position state.
    pub(super) fn restore_queue_state(&mut self) {
        let Some(state) = crate::config::load_queue_state() else {
            log::info!(target: "queue", "restore: no queue_state.json found, nothing to restore");
            return;
        };
        if state.items.is_empty() {
            log::info!(target: "queue", "restore: queue_state.json has no items, nothing to restore");
            return;
        }
        let cursor = queue_restore_cursor(
            &state.items,
            state.cursor,
            state.last_played_item_id.as_deref(),
            state.last_played_completed,
        );
        let restored_count = state.items.len();
        self.last_played_item_id = state.last_played_item_id;
        self.last_played_completed = state.last_played_completed;
        self.queue_source = state.source;
        self.player_tab.set_items(state.items, cursor);
        self.queue_dirty = false;
        if self.client.lock().unwrap().config.start_on_queue {
            self.tab_idx = 1;
        }
        log::info!(target: "queue", "restore: restored {restored_count} item(s), cursor={cursor}");
        self.spawn_enrich_queue_state(state.positions);
    }

    /// Best-effort background refresh of played/position state for whatever
    /// is currently in `player_tab.items` (populated by `restore_queue_state`
    /// just before this is called). Merges by item ID into the *current*
    /// queue when it resolves, so it can never resurrect an item the user
    /// has since consumed, nor clobber a queue they've since replaced —
    /// unlike a wholesale overwrite, an ID that's no longer present is simply
    /// skipped.
    pub(super) fn spawn_enrich_queue_state(
        &self,
        positions: std::collections::HashMap<String, i64>,
    ) {
        let item_ids: Vec<String> = self.player_tab.items.iter().map(|i| i.id.clone()).collect();
        if item_ids.is_empty() {
            return;
        }
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let mut items = match client.get_items_by_ids(&item_ids) {
                Ok(items) => items,
                Err(e) => {
                    log::warn!(target: "queue", "restore: enrichment fetch failed: {e}");
                    return;
                }
            };
            // Apply locally-saved positions where they are fresher than what Emby returned.
            // Emby's UserData may lag by up to a few seconds after a Stopped report.
            for item in &mut items {
                if let Some(&saved_pos) = positions.get(&item.id) {
                    if saved_pos > item.playback_position_ticks {
                        log::info!(target: "player", "restore: applying saved pos={}s (Emby had {}s) for item={}",
                            saved_pos / mbv_core::api::TICKS_PER_SECOND,
                            item.playback_position_ticks / mbv_core::api::TICKS_PER_SECOND,
                            item.id);
                        item.playback_position_ticks = saved_pos;
                    }
                }
            }
            let _ = tx.send(LibEvent::QueueEnriched { items });
        });
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
            self.set_tab(1);
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
        let n = self.libs.len();
        let lib = &mut self.layout.library;
        lib.lib_scroll.resize(n, 0);
        lib.lib_row_heights.resize_with(n, Vec::new);
        lib.lib_table_area
            .resize(n, ratatui::layout::Rect::default());
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
        self.ensure_home_section_visible();
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
                self.tab_idx = 1;
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
mod tests {
    use super::*;
    use crate::app::tests::{make_app_stub, make_item, make_items};
    use crate::app::{AlbumIndexState, BrowseLevel, LibraryTab};
    use mbv_core::player::PlayerEvent;
    use std::sync::mpsc;

    fn folder(id: &str, name: &str) -> MediaItem {
        let mut item = make_item(name, "Folder");
        item.id = id.into();
        item.is_folder = true;
        item
    }

    fn album(id: &str, name: &str) -> MediaItem {
        let mut item = make_item(name, "MusicAlbum");
        item.id = id.into();
        item.is_folder = true;
        item.media_type = "Audio".into();
        item
    }

    fn recursive_music_app() -> App {
        let mut app = make_app_stub();
        app.music_levels = vec!["group".into(), "artist".into(), "album".into()];
        let mut library = make_item("Music", "CollectionFolder");
        library.id = "music-lib".into();
        library.collection_type = "music".into();
        library.is_folder = true;
        app.libs.push(LibraryTab {
            library,
            nav_stack: Vec::new(),
            search: None,
            feed_home_video: None,
            album_track_focus: None,
            artist_header_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: None,
        });
        app
    }

    #[test]
    fn album_index_eligibility_requires_grouped_music_ending_in_album() {
        assert!(recursive_album_search_eligible(
            "music",
            &["group".into(), "album".into()]
        ));
        assert!(recursive_album_search_eligible(
            "music",
            &["group".into(), "artist".into(), "album".into()]
        ));
        assert!(!recursive_album_search_eligible("music", &[]));
        assert!(!recursive_album_search_eligible("music", &["album".into()]));
        assert!(!recursive_album_search_eligible(
            "music",
            &["group".into(), "artist".into()]
        ));
        assert!(!recursive_album_search_eligible(
            "movies",
            &["group".into(), "album".into()]
        ));
    }

    #[test]
    fn album_index_traverses_deep_branches_pages_and_ignores_non_albums() {
        let mut tree = HashMap::new();
        tree.insert(
            "music-lib".to_string(),
            vec![folder("group-a", "A"), folder("group-b", "B")],
        );
        tree.insert(
            "group-a".to_string(),
            vec![
                folder("artist-empty", "Empty"),
                folder("artist-a", "Artist A"),
            ],
        );
        tree.insert("artist-empty".to_string(), Vec::new());
        let mut many_albums: Vec<MediaItem> = (0..201)
            .map(|index| album(&format!("album-a-{index}"), &format!("Record {index}")))
            .collect();
        many_albums.push(make_item("Not an album", "Audio"));
        tree.insert("artist-a".to_string(), many_albums);
        tree.insert("group-b".to_string(), vec![folder("artist-b", "Artist B")]);
        tree.insert("artist-b".to_string(), vec![album("album-b", "Record 0")]);
        let mut calls = Vec::new();
        let mut fetch = |parent: &str, start: usize, limit: usize| {
            calls.push((parent.to_string(), start));
            let all = tree.get(parent).cloned().unwrap_or_default();
            let page = all.iter().skip(start).take(limit).cloned().collect();
            Ok((page, all.len()))
        };

        let entries = build_album_index_with(
            "music-lib",
            &["group".into(), "artist".into(), "album".into()],
            &mut fetch,
        )
        .unwrap();

        assert_eq!(entries.len(), 202);
        assert_eq!(
            entries.last().unwrap().display_label,
            "B / Artist B / Record 0"
        );
        assert_eq!(
            entries.last().unwrap().ancestors,
            vec![
                AlbumPathPart {
                    id: "group-b".into(),
                    name: "B".into()
                },
                AlbumPathPart {
                    id: "artist-b".into(),
                    name: "Artist B".into()
                }
            ]
        );
        assert!(calls.contains(&("artist-a".into(), 200)));
        assert!(entries
            .iter()
            .all(|entry| entry.album.item_type == "MusicAlbum"));
    }

    #[test]
    fn recursive_album_search_matches_ancestor_labels() {
        let mut app = recursive_music_app();
        let target = album("album-1", "Needle Record");
        app.album_indexes.insert(
            "music-lib".into(),
            AlbumIndexState::Ready(vec![AlbumSearchEntry {
                album: target,
                ancestors: vec![AlbumPathPart {
                    id: "group-a".into(),
                    name: "Deep Group".into(),
                }],
                display_label: "Deep Group / Needle Record".into(),
                search_text: "Deep Group / Needle Record".into(),
            }]),
        );

        assert!(app.open_recursive_album_search(0));
        app.libs[0].search.as_mut().unwrap().query = "deep grp".into();
        app.update_lib_search(0);

        assert_eq!(app.libs[0].search.as_ref().unwrap().results, vec![0]);
        assert_eq!(
            app.recursive_album_display_item(0, 0, album("album-1", "Needle Record"))
                .name,
            "Deep Group / Needle Record"
        );

        app.libs[0].search.as_mut().unwrap().query = "needle rec".into();
        app.update_lib_search(0);
        assert_eq!(app.libs[0].search.as_ref().unwrap().results, vec![0]);
    }

    #[test]
    fn album_only_music_keeps_visible_list_search() {
        let mut app = recursive_music_app();
        app.music_levels = vec!["album".into()];
        app.libs[0].search = Some(super::super::LibSearch {
            query: "visible rec".into(),
            items: vec![album("album-1", "Visible Record")],
            results: Vec::new(),
            cursor: 0,
            scroll: 0,
            loading: false,
        });

        assert!(!app.open_recursive_album_search(0));
        app.update_lib_search(0);

        assert_eq!(app.libs[0].search.as_ref().unwrap().results, vec![0]);
    }

    #[test]
    fn album_index_completion_updates_the_open_current_query() {
        let mut app = recursive_music_app();
        app.album_indexes.insert(
            "music-lib".into(),
            AlbumIndexState::Loading {
                rebuild_pending: false,
            },
        );
        assert!(app.open_recursive_album_search(0));
        app.libs[0].search.as_mut().unwrap().query = "remote group".into();

        app.handle_lib_event(LibEvent::AlbumIndexBuilt {
            library_id: "music-lib".into(),
            result: Ok(vec![AlbumSearchEntry {
                album: album("album-1", "Record"),
                ancestors: vec![AlbumPathPart {
                    id: "group-a".into(),
                    name: "Remote Group".into(),
                }],
                display_label: "Remote Group / Record".into(),
                search_text: "Remote Group / Record".into(),
            }]),
        });

        let search = app.libs[0].search.as_ref().unwrap();
        assert!(!search.loading);
        assert_eq!(search.query, "remote group");
        assert_eq!(search.results, vec![0]);
    }

    #[test]
    fn failed_album_index_becomes_unavailable_and_clears_search_loading() {
        let mut app = recursive_music_app();
        app.album_indexes.insert(
            "music-lib".into(),
            AlbumIndexState::Loading {
                rebuild_pending: false,
            },
        );
        assert!(app.open_recursive_album_search(0));

        app.handle_lib_event(LibEvent::AlbumIndexBuilt {
            library_id: "music-lib".into(),
            result: Err("index failed".into()),
        });

        assert!(matches!(
            app.album_indexes.get("music-lib"),
            Some(AlbumIndexState::Unavailable)
        ));
        assert!(!app.libs[0].search.as_ref().unwrap().loading);
        assert!(app.status.contains("index failed"));
    }

    #[test]
    fn refresh_while_album_index_loads_coalesces_one_replacement() {
        let mut app = recursive_music_app();
        app.album_indexes.insert(
            "music-lib".into(),
            AlbumIndexState::Loading {
                rebuild_pending: false,
            },
        );

        app.start_album_index(0, true);
        app.start_album_index(0, true);

        assert!(matches!(
            app.album_indexes.get("music-lib"),
            Some(AlbumIndexState::Loading {
                rebuild_pending: true
            })
        ));
    }

    #[test]
    fn power_recursive_activation_keeps_power_view_and_enters_inline_tracks() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut app = recursive_music_app();
        app.tab_idx = 1;
        app.view_mode = ViewMode::Power;
        app.power_left_tab = 1;
        app.power_focus = PowerFocus::Left;
        app.libs[0].nav_stack.push(BrowseLevel {
            parent_id: "group-a".into(),
            title: "Group A".into(),
            items: vec![folder("artist-a", "Artist A")],
            total_count: 1,
            cursor: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            scroll: 0,
            all_items: None,
            letter_filter: None,
        });
        let default_position = app.libs[0].library_position_snapshot();
        app.library_position_state.libraries.insert(
            "music-lib".into(),
            mbv_core::config::LibraryViewPositions {
                default: Some(default_position.clone()),
                power: None,
            },
        );
        app.libs[0].search = Some(super::super::LibSearch {
            query: String::new(),
            items: Vec::new(),
            results: Vec::new(),
            cursor: 0,
            scroll: 0,
            loading: false,
        });
        let level = BrowseLevel {
            parent_id: "artist-c".into(),
            title: "Artist C".into(),
            items: vec![album("album-1", "Record")],
            total_count: 1,
            cursor: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            scroll: 0,
            all_items: None,
            letter_filter: None,
        };

        app.handle_lib_event(LibEvent::RecursiveAlbumActivated {
            library_id: "music-lib".into(),
            scope: LibraryPositionScope::Power,
            nav_stack: vec![level],
        });

        assert_eq!(app.tab_idx, 1);
        assert!(app.libs[0].search.is_none());
        assert_eq!(app.libs[0].album_track_focus, Some(0));
        assert_eq!(app.libs[0].nav_stack.last().unwrap().parent_id, "artist-c");
        let positions = app
            .library_position_state
            .libraries
            .get("music-lib")
            .unwrap();
        assert_eq!(positions.default.as_ref(), Some(&default_position));
        assert_eq!(
            positions
                .power
                .as_ref()
                .and_then(|position| position.levels.last())
                .map(|level| level.parent_id.as_str()),
            Some("artist-c")
        );
    }

    // ── remote_seek_ticks: asymmetric clamp (rewind only) ───────────────────

    #[test]
    fn remote_seek_rewind_clamps_at_zero() {
        // 3s in, rewind 5s: would go negative, must clamp to 0.
        assert_eq!(App::remote_seek_ticks(3, -5.0), 0);
    }

    #[test]
    fn remote_seek_rewind_does_not_clamp_when_unnecessary() {
        assert_eq!(App::remote_seek_ticks(20, -5.0), 15 * TICKS_PER_SECOND);
    }

    #[test]
    fn remote_seek_forward_has_no_clamp() {
        // Fast-forward has no lower-bound clamp in the original code; a small
        // pos_s plus a large forward delta simply goes wherever the math
        // says, same as rewind's clamp being absent here.
        assert_eq!(App::remote_seek_ticks(3, 5.0), 8 * TICKS_PER_SECOND);
    }

    // ── execute_context_action(Play) on the queue tab (issue #134 follow-up) ─
    // This used to be a third, independent copy of queue-cursor activation
    // that had drifted from the keyboard `Enter`/mouse double-click paths
    // (no seek-to-start for an already-playing audio item); it now shares
    // `Command::QueuePlayCursor` with both of them.

    #[test]
    fn context_menu_play_on_queue_tab_seeks_to_start_for_current_playing_audio_item() {
        use crate::app::tests::make_item;

        let mut app = crate::app::tests::make_app_stub();
        app.tab_idx = 1;
        app.player_tab
            .set_items(vec![make_item("Track One", "Audio")], 0);
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.current_idx = 0;
        }
        let rx = app.player.spy_on_commands();

        app.execute_context_action(Some(ContextAction::Play));

        assert!(matches!(
            rx.try_recv(),
            Ok(PlayerCommand::SeekAbsolute(pos)) if pos == 0.0
        ));
    }

    #[test]
    fn power_view_enqueue_then_queue_play_cursor_syncs_and_jumps_to_new_item() {
        use crate::app::action::Command;
        use crate::app::tests::make_item;
        use crate::app::{BrowseLevel, LibraryTab, PowerFocus, ViewMode};
        use crate::player::PlayerCommand;

        let mut app = crate::app::tests::make_app_stub();
        app.tab_idx = app.lib_tab_offset();
        app.view_mode = ViewMode::Power;
        app.power_focus = PowerFocus::Left;
        app.power_left_tab = 1;
        app.player_tab
            .set_items(vec![make_item("Queued First", "Movie")], 0);
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.current_idx = 0;
            st.queue_len = 1;
        }

        let mut library = make_item("Movies", "CollectionFolder");
        library.id = "lib-movies".into();
        library.is_folder = true;
        library.collection_type = "movies".into();

        let mut queued = make_item("Queued Second", "Movie");
        queued.id = "movie-2".into();

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-movies".into(),
                title: "Movies".into(),
                items: vec![queued.clone()],
                total_count: 1,
                cursor: 0,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
            }],
            search: None,
            feed_home_video: None,
            album_track_focus: None,
            artist_header_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: None,
        });

        assert_eq!(
            app.current_lib_item().as_ref().map(|i| i.id.as_str()),
            Some("movie-2")
        );

        let rx = app.player.spy_on_commands();
        app.execute_context_action(Some(crate::app::ContextAction::Enqueue));

        assert_eq!(app.player_tab.items.len(), 2);
        assert_eq!(app.player_tab.items[1].id, queued.id);
        assert!(matches!(
            rx.try_recv(),
            Ok(PlayerCommand::QueueAppend { items }) if items.len() == 1 && items[0].id == queued.id
        ));

        app.power_focus = PowerFocus::Queue;
        app.player_tab.queue_cursor = 1;

        app.dispatch(Command::QueuePlayCursor);

        assert!(matches!(rx.try_recv(), Ok(PlayerCommand::JumpTo(1))));
    }

    // ── next_subtitle_entry: shared cycling math (remote/local parity, #86) ─

    #[test]
    fn next_subtitle_entry_advances_from_off() {
        assert_eq!(App::next_subtitle_entry(&[0, 5, 7], 0), 5);
    }

    #[test]
    fn next_subtitle_entry_wraps_from_last_back_to_off() {
        assert_eq!(App::next_subtitle_entry(&[0, 5, 7], 7), 0);
    }

    #[test]
    fn next_subtitle_entry_unknown_current_restarts_at_first() {
        // A stale/unrecognized current selection (e.g. a track that
        // disappeared) is treated as if it were at position 0, matching the
        // pre-existing `.unwrap_or(0)` fallback in both the remote and local
        // branches -- so the *next* entry advances to position 1.
        assert_eq!(App::next_subtitle_entry(&[0, 5, 7], 99), 5);
    }

    #[test]
    fn next_subtitle_entry_empty_returns_current_unchanged() {
        assert_eq!(App::next_subtitle_entry(&[], 3), 3);
    }

    #[test]
    fn next_subtitle_entry_matches_remote_sentinel_convention() {
        // Remote sessions use -1 as the "off" sentinel (vs. 0 for local
        // playback) -- same wraparound math, different sentinel value.
        assert_eq!(App::next_subtitle_entry(&[-1, 2, 4], -1), 2);
        assert_eq!(App::next_subtitle_entry(&[-1, 2, 4], 4), -1);
    }

    // ── cycle_sub: local branch (#86 unification + idle fallback) ───────────

    // `XDG_CONFIG_HOME`/`MBV_SYSTEM` are process-global env vars, so tests
    // that touch them must not run concurrently with each other -- or with
    // any other test in the crate that touches env vars.
    // Reuse config.rs's `SYS_ENV_LOCK` rather than a second, independent
    // mutex: two separate locks over the same global state don't exclude
    // each other and previously caused flaky cross-test env-var races.
    use crate::config::tests::SYS_ENV_LOCK as XDG_HOME_LOCK;

    /// RAII guard that points `XDG_CONFIG_HOME` (subtitle-mode saves) and
    /// test-only state-dir lookups (prefs/queue saves) at a fresh tempdir,
    /// restoring and cleaning up on drop -- including on panic.
    struct XdgHomeGuard {
        dir: std::path::PathBuf,
        _state_dir: crate::config::TestStateDirGuard,
    }

    impl XdgHomeGuard {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("mbv-test-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            std::env::set_var("XDG_CONFIG_HOME", &dir);
            std::env::remove_var("MBV_SYSTEM");
            let state_dir = crate::config::TestStateDirGuard::new_at(dir.join("mbv"));
            Self {
                dir,
                _state_dir: state_dir,
            }
        }
    }

    impl Drop for XdgHomeGuard {
        fn drop(&mut self) {
            std::env::remove_var("XDG_CONFIG_HOME");
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    // ── queue_restore_cursor: last-played-id lookup + drift fallback ────────

    #[test]
    fn queue_restore_cursor_finds_last_played_by_id() {
        let items = crate::app::tests::make_items(3);
        let cursor = queue_restore_cursor(&items, 0, Some("id1"), false);
        assert_eq!(cursor, 1);
    }

    #[test]
    fn queue_restore_cursor_advances_past_a_completed_last_played_item() {
        let items = crate::app::tests::make_items(3);
        let cursor = queue_restore_cursor(&items, 0, Some("id1"), true);
        assert_eq!(cursor, 2);
    }

    #[test]
    fn queue_restore_cursor_falls_back_to_saved_cursor_when_last_played_id_missing() {
        let items = crate::app::tests::make_items(3);
        // "id5" isn't in the restored list (e.g. it was removed from the
        // queue before quitting) — must fall back to the saved cursor, not
        // silently snap back to the front of the queue.
        let cursor = queue_restore_cursor(&items, 2, Some("id5"), false);
        assert_eq!(cursor, 2);
    }

    #[test]
    fn queue_restore_cursor_falls_back_to_saved_cursor_clamped_to_len() {
        let items = crate::app::tests::make_items(3);
        let cursor = queue_restore_cursor(&items, 99, Some("id5"), false);
        #[rustfmt::skip]
        assert_eq!(
            cursor, 2,
            "out-of-range saved cursor must clamp to the last valid index"
        );
    }

    #[test]
    fn queue_restore_cursor_uses_saved_cursor_when_no_last_played_id() {
        let items = crate::app::tests::make_items(3);
        let cursor = queue_restore_cursor(&items, 1, None, false);
        assert_eq!(cursor, 1);
    }

    // ── queue_state persistence: restore + attached-session guards ──────────

    #[test]
    fn restore_queue_state_with_no_saved_file_does_nothing() {
        let _g = XDG_HOME_LOCK.lock().unwrap();
        let _xdg = XdgHomeGuard::new();

        let mut app = crate::app::tests::make_app_stub();
        app.restore_queue_state();

        assert!(app.player_tab.items.is_empty());
    }

    #[test]
    fn restore_queue_state_with_no_items_does_nothing() {
        let _g = XDG_HOME_LOCK.lock().unwrap();
        let _xdg = XdgHomeGuard::new();

        crate::config::save_queue_state(&crate::config::QueueState {
            source: crate::config::QueueSource::Unknown,
            items: vec![],
            cursor: 0,
            last_played_item_id: None,
            last_played_completed: false,
            positions: Default::default(),
        });

        let mut app = crate::app::tests::make_app_stub();
        app.restore_queue_state();

        assert!(app.player_tab.items.is_empty());
    }

    #[test]
    fn restore_queue_state_populates_queue_synchronously_from_disk() {
        let _g = XDG_HOME_LOCK.lock().unwrap();
        let _xdg = XdgHomeGuard::new();

        let items = crate::app::tests::make_items(3);
        crate::config::save_queue_state(&crate::config::QueueState {
            source: crate::config::QueueSource::Unknown,
            items: items.clone(),
            cursor: 1,
            last_played_item_id: None,
            last_played_completed: false,
            positions: Default::default(),
        });

        let mut app = crate::app::tests::make_app_stub();
        app.restore_queue_state();

        // No network call is needed for the queue to already be correct —
        // this is a synchronous, local read, not a spawned background fetch.
        assert_eq!(app.player_tab.items.len(), 3);
        assert_eq!(app.player_tab.queue_cursor, 1);
    }

    #[test]
    fn restore_queue_state_clears_a_stale_dirty_flag() {
        let _g = XDG_HOME_LOCK.lock().unwrap();
        let _xdg = XdgHomeGuard::new();

        crate::config::save_queue_state(&crate::config::QueueState {
            source: crate::config::QueueSource::Unknown,
            items: crate::app::tests::make_items(1),
            cursor: 0,
            last_played_item_id: None,
            last_played_completed: false,
            positions: Default::default(),
        });

        let mut app = crate::app::tests::make_app_stub();
        app.queue_dirty = true;
        app.restore_queue_state();

        assert!(
            !app.queue_dirty,
            "restoring a queue from disk is not a local edit — it must not \
             leave a stale dirty flag that could trigger an unwanted \
             save_playlist_to_emby() push on the next consume"
        );
    }

    #[test]
    fn quit_preserves_saved_playlist_source_for_restart_restore() {
        let _g = XDG_HOME_LOCK.lock().unwrap();
        let _xdg = XdgHomeGuard::new();

        let mut app = crate::app::tests::make_app_stub();
        app.player_tab.items = crate::app::tests::make_items(2);
        app.queue_source = crate::config::QueueSource::Playlist {
            id: Some("playlist-id".into()),
            name: "Saved Queue".into(),
        };
        app.queue_dirty = true;

        assert!(app.try_quit());
        app.save_queue_state_no_clear();

        let state = crate::config::load_queue_state().expect("queue state should be saved");
        assert_eq!(
            state.source,
            crate::config::QueueSource::Playlist {
                id: Some("playlist-id".into()),
                name: "Saved Queue".into(),
            },
            "shutdown persistence must keep the saved-playlist association so \
             a restart can still autosave/consume against the playlist"
        );
    }

    #[test]
    fn handle_loaded_level_replaces_the_matching_loading_level() {
        let mut app = crate::app::tests::make_app_stub();
        let mut library = crate::app::tests::make_item("Movies", "CollectionFolder");
        library.id = "lib-movies".into();
        library.is_folder = true;
        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "parent".into(),
                title: "Loading".into(),
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
            }],
            search: None,
            feed_home_video: None,

            album_track_focus: None,
            artist_header_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: None,
        });

        let level = BrowseLevel {
            parent_id: "parent".into(),
            title: "Loaded".into(),
            items: crate::app::tests::make_items(2),
            total_count: 2,
            cursor: 1,
            item_types: None,
            unplayed_only: false,
            sort_by: "DateCreated".into(),
            sort_order: "Descending".into(),
            loading: false,
            scroll: 3,
            all_items: None,
            letter_filter: None,
        };

        app.handle_loaded_level(0, "parent".into(), level);

        let last = app.libs[0].nav_stack.last().unwrap();
        assert_eq!(last.title, "Loaded");
        assert_eq!(last.items.len(), 2);
        assert_eq!(last.total_count, 2);
        assert_eq!(last.cursor, 1);
        assert_eq!(last.sort_by, "DateCreated");
        assert_eq!(last.sort_order, "Descending");
        assert!(!last.loading);
    }

    #[test]
    fn normalize_current_browse_level_items_sorts_episode_lists() {
        let mut app = crate::app::tests::make_app_stub();
        let mut second = crate::app::tests::make_item("Episode 2", "Episode");
        second.index_number = 2;
        let mut first = crate::app::tests::make_item("Episode 1", "Episode");
        first.index_number = 1;
        let mut library = crate::app::tests::make_item("TV", "CollectionFolder");
        library.id = "lib-tv".into();
        library.is_folder = true;
        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "series".into(),
                title: "Season 1".into(),
                items: vec![second, first],
                total_count: 2,
                cursor: 0,
                item_types: Some("Episode".into()),
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                scroll: 0,
                all_items: None,
                letter_filter: None,
            }],
            search: None,
            feed_home_video: None,

            album_track_focus: None,
            artist_header_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: None,
        });

        app.normalize_current_browse_level_items(0, false);

        let last = app.libs[0].nav_stack.last().unwrap();
        let names: Vec<&str> = last.items.iter().map(|item| item.name.as_str()).collect();
        assert_eq!(names, vec!["Episode 1", "Episode 2"]);
    }

    #[test]
    fn ensure_power_feed_library_preserves_saved_feed_position() {
        let mut app = crate::app::tests::make_app_stub();
        app.view_mode = ViewMode::Power;
        app.power_left_tab = 1;
        app.client.lock().unwrap().config.feed_view_libraries = vec!["youtube".into()];

        let mut library = crate::app::tests::make_item("YouTube", "CollectionFolder");
        library.id = "lib-feed".into();
        library.is_folder = true;
        library.collection_type = "homevideos".into();
        app.libs.push(LibraryTab {
            library,
            nav_stack: Vec::new(),
            search: None,
            feed_home_video: Some(FeedHomeVideoState {
                selected_group: 2,
                video_cursor: 3,
                video_scroll: 4,
                ..Default::default()
            }),

            album_track_focus: None,
            artist_header_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: None,
        });

        app.ensure_lib_loaded_for(0);

        let state = app.libs[0].feed_home_video.as_ref().unwrap();
        assert!(state.loading);
        assert_eq!(state.selected_group, 2);
        assert_eq!(state.video_cursor, 3);
        assert_eq!(state.video_scroll, 4);
    }

    #[test]
    fn ensure_power_podcast_library_preserves_saved_feed_position() {
        let mut app = crate::app::tests::make_app_stub();
        app.view_mode = ViewMode::Power;
        app.power_left_tab = 1;

        let mut library = crate::app::tests::make_item("Podcasts", "CollectionFolder");
        library.id = "lib-podcasts".into();
        library.is_folder = true;
        library.collection_type = "podcasts".into();
        app.libs.push(LibraryTab {
            library,
            nav_stack: Vec::new(),
            search: None,
            feed_home_video: Some(FeedHomeVideoState {
                selected_group: 1,
                video_cursor: 5,
                video_scroll: 6,
                ..Default::default()
            }),

            album_track_focus: None,
            artist_header_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: None,
        });

        app.ensure_lib_loaded_for(0);

        let state = app.libs[0].feed_home_video.as_ref().unwrap();
        assert!(state.loading);
        assert_eq!(state.selected_group, 1);
        assert_eq!(state.video_cursor, 5);
        assert_eq!(state.video_scroll, 6);
    }

    #[test]
    fn queue_enriched_prunes_items_the_server_no_longer_returns() {
        let mut app = crate::app::tests::make_app_stub();
        app.player_tab.items = crate::app::tests::make_items(3); // id0, id1, id2
        app.player_tab.queue_cursor = 0;

        // The background fetch no longer returns id1 (e.g. deleted server-side).
        #[rustfmt::skip]
        let fresh = vec![app.player_tab.items[0].clone(), app.player_tab.items[2].clone()];
        app.handle_lib_event(LibEvent::QueueEnriched { items: fresh });

        let ids: Vec<&str> = app.player_tab.items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["id0", "id2"],
            "an item missing from the fresh fetch must be pruned from the \
             restored queue, not left stale forever"
        );
        assert_eq!(
            app.player_tab.queue_cursor, 0,
            "removing an item after the cursor must not shift the cursor"
        );
    }

    #[test]
    fn queue_enriched_prunes_live_playback_slots_and_resyncs_player_queue() {
        let mut app = crate::app::tests::make_app_stub();
        app.player_tab.items = crate::app::tests::make_items(3);
        let cmd_rx = app.player.spy_on_commands();
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.current_idx = 0;
        }

        let fresh = vec![
            app.player_tab.items[0].clone(),
            app.player_tab.items[2].clone(),
        ];
        app.handle_lib_event(LibEvent::QueueEnriched { items: fresh });

        assert!(
            matches!(
                cmd_rx.try_recv(),
                Ok(crate::player::PlayerCommand::QueueRemove(1))
            ),
            "pruning a live playback queue slot must also remove it from the player's private queue copy"
        );
    }

    #[test]
    fn queue_enriched_never_prunes_or_merges_the_active_slot_even_with_a_duplicate_id() {
        let mut app = crate::app::tests::make_app_stub();
        let mut items = crate::app::tests::make_items(2); // id0, id1
        items[1].id = "id0".to_string(); // duplicate of the active item's id
        app.player_tab.items = items;
        app.player_tab.items[0].playback_position_ticks = 3 * mbv_core::api::TICKS_PER_SECOND;
        app.player_tab.sync_queue_model_from_items_if_needed();
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.current_idx = 0;
        }

        // The fetch confirms id0 still exists, so slot 1's duplicate id0 would
        // also match by id alone if the skip weren't by-slot.
        let mut fresh = app.player_tab.items[0].clone();
        fresh.name = "Refreshed Name".to_string();
        app.handle_lib_event(LibEvent::QueueEnriched {
            items: vec![fresh.clone()],
        });

        assert_eq!(
            app.player_tab.items[0].playback_position_ticks,
            3 * mbv_core::api::TICKS_PER_SECOND,
            "the active slot must keep its authoritative local progress even though its id matched"
        );
        assert_eq!(
            app.player_tab.items[1].name, "Refreshed Name",
            "the non-active duplicate-id slot must still be enriched from the fresh fetch"
        );
    }

    #[test]
    fn queue_enriched_skips_player_active_idx_not_queue_cursor() {
        let mut app = crate::app::tests::make_app_stub();
        app.player_tab.items = crate::app::tests::make_items(2);
        app.player_tab.queue_cursor = 1;
        app.player_tab.items[0].playback_position_ticks = 3 * mbv_core::api::TICKS_PER_SECOND;
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.current_idx = 0;
        }
        let mut stale = app.player_tab.items[0].clone();
        stale.playback_position_ticks = 46 * mbv_core::api::TICKS_PER_SECOND;

        app.handle_lib_event(LibEvent::QueueEnriched { items: vec![stale] });

        assert_eq!(
            app.player_tab.items[0].playback_position_ticks,
            3 * mbv_core::api::TICKS_PER_SECOND,
            "stale enrichment must not overwrite the actively playing slot"
        );
    }

    #[test]
    fn queue_enriched_preserves_pending_sync_until_server_confirms_it() {
        let mut app = crate::app::tests::make_app_stub();
        app.player_tab.items = crate::app::tests::make_items(1);
        app.player_tab.sync_queue_model_from_items_if_needed();
        app.handle_player_event(mbv_core::player::PlayerEvent::TrackChanged(0));
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.current_idx = 0;
        }
        app.handle_player_event(mbv_core::player::PlayerEvent::Stopped {
            idx: 0,
            position_ticks: 6 * mbv_core::api::TICKS_PER_SECOND,
            played: false,
            consume: false,
            progress_report_accepted: true,
            error: None,
        });
        let mut stale = app.player_tab.items[0].clone();
        stale.playback_position_ticks = mbv_core::api::TICKS_PER_SECOND;

        app.handle_lib_event(LibEvent::QueueEnriched { items: vec![stale] });

        assert_eq!(
            app.player_tab.items[0].playback_position_ticks,
            6 * mbv_core::api::TICKS_PER_SECOND,
            "stale enrichment must not overwrite accepted local stopped progress while sync is pending"
        );
        assert!(app.player_tab.queue.slots()[0]
            .progress_state
            .pending_sync
            .is_some());
    }

    #[test]
    fn manual_refresh_merge_uses_queue_model_active_slot_protection() {
        let mut app = crate::app::tests::make_app_stub();
        app.player_tab.items = crate::app::tests::make_items(2);
        app.player_tab.sync_queue_model_from_items_if_needed();
        let active_slot = app.player_tab.queue.slots()[0].slot_id;
        let _ = app.player_tab.queue.apply_progress(
            active_slot,
            9 * mbv_core::api::TICKS_PER_SECOND,
            false,
        );
        app.player_tab.sync_items_from_queue_model();
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.current_idx = 0;
        }
        let mut stale_active = app.player_tab.items[0].clone();
        stale_active.playback_position_ticks = mbv_core::api::TICKS_PER_SECOND;
        let mut fresh_inactive = app.player_tab.items[1].clone();
        fresh_inactive.playback_position_ticks = 4 * mbv_core::api::TICKS_PER_SECOND;

        let _ = app.merge_refreshed_queue(QueueScope::Local, vec![stale_active, fresh_inactive]);

        assert_eq!(
            app.player_tab.items[0].playback_position_ticks,
            9 * mbv_core::api::TICKS_PER_SECOND
        );
        assert_eq!(
            app.player_tab.items[1].playback_position_ticks,
            4 * mbv_core::api::TICKS_PER_SECOND
        );
    }

    #[test]
    fn save_queue_state_does_not_delete_file_while_attached_to_remote_session() {
        let _g = XDG_HOME_LOCK.lock().unwrap();
        let _xdg = XdgHomeGuard::new();

        // Seed an on-disk queue as if a previous local session left one behind.
        crate::config::save_queue_state(&crate::config::QueueState {
            source: crate::config::QueueSource::Unknown,
            items: crate::app::tests::make_items(2),
            cursor: 0,
            last_played_item_id: None,
            last_played_completed: false,
            positions: Default::default(),
        });

        let mut app = crate::app::tests::make_app_stub();
        app.player_tab.items.clear();
        app.connected_session_id = Some("session-1".into());

        app.save_queue_state();

        assert!(
            crate::config::load_queue_state().is_some(),
            "an empty local tab while attached to a remote session must not delete the \
             saved queue — that emptiness reflects remote-control UI state, not the user \
             clearing their queue"
        );
    }

    #[test]
    fn save_queue_state_still_clears_file_when_locally_empty_and_not_attached() {
        let _g = XDG_HOME_LOCK.lock().unwrap();
        let _xdg = XdgHomeGuard::new();

        crate::config::save_queue_state(&crate::config::QueueState {
            source: crate::config::QueueSource::Unknown,
            items: crate::app::tests::make_items(1),
            cursor: 0,
            last_played_item_id: None,
            last_played_completed: false,
            positions: Default::default(),
        });

        let mut app = crate::app::tests::make_app_stub();
        app.player_tab.items.clear();
        app.connected_session_id = None;

        app.save_queue_state();

        assert!(
            crate::config::load_queue_state().is_none(),
            "a genuinely empty local queue with no remote session attached should still clear"
        );
    }

    #[test]
    fn save_queue_state_no_clear_preserves_file_when_locally_empty_and_not_attached() {
        let _g = XDG_HOME_LOCK.lock().unwrap();
        let _xdg = XdgHomeGuard::new();

        // Seed an on-disk queue as if a previous session left one behind — this
        // session never touched the local queue tab (e.g. only browsed Home).
        crate::config::save_queue_state(&crate::config::QueueState {
            source: crate::config::QueueSource::Unknown,
            items: crate::app::tests::make_items(1),
            cursor: 0,
            last_played_item_id: None,
            last_played_completed: false,
            positions: Default::default(),
        });

        let mut app = crate::app::tests::make_app_stub();
        app.player_tab.items.clear();
        app.connected_session_id = None;

        app.save_queue_state_no_clear();

        assert!(
            crate::config::load_queue_state().is_some(),
            "quitting with a transiently-empty in-memory queue must not delete an \
             existing on-disk snapshot — only an explicit user-initiated clear should"
        );
    }

    #[test]
    fn save_queue_state_no_clear_still_saves_when_queue_has_items() {
        let _g = XDG_HOME_LOCK.lock().unwrap();
        let _xdg = XdgHomeGuard::new();

        let mut app = crate::app::tests::make_app_stub();
        app.player_tab.items = crate::app::tests::make_items(2);

        app.save_queue_state_no_clear();

        let state = crate::config::load_queue_state().expect("queue should be saved");
        assert_eq!(state.items.len(), 2);
    }

    #[test]
    fn cycle_sub_local_idle_cycles_subtitle_mode_not_a_track() {
        let _g = XDG_HOME_LOCK.lock().unwrap();
        let _xdg = XdgHomeGuard::new();

        let mut app = crate::app::tests::make_app_stub();
        app.player.status.lock().unwrap().active = false;
        let before = app.client.lock().unwrap().config.subtitle_mode.clone();

        app.cycle_sub();

        let after = app.client.lock().unwrap().config.subtitle_mode.clone();
        assert_ne!(
            before, after,
            "idle z has no session equivalent, so it should still cycle the default subtitle mode"
        );
    }

    #[test]
    fn cycle_sub_local_active_does_not_fall_back_to_subtitle_mode() {
        let _g = XDG_HOME_LOCK.lock().unwrap();
        let _xdg = XdgHomeGuard::new();

        let mut app = crate::app::tests::make_app_stub();
        {
            let mut status = app.player.status.lock().unwrap();
            status.active = true;
            status.sub_tracks = vec![(1, "English".to_string(), false)];
            status.sub_id = 0;
        }
        let before = app.client.lock().unwrap().config.subtitle_mode.clone();

        // #86: local `z` while active now cycles every track (like the
        // remote path) instead of the old on/off `toggle_sub()` -- assert at
        // minimum that it does *not* take the idle subtitle-mode fallback.
        app.cycle_sub();

        let after = app.client.lock().unwrap().config.subtitle_mode.clone();
        assert_eq!(
            before, after,
            "an active player has tracks to cycle and must not touch the idle subtitle-mode fallback"
        );
    }

    // ── is_audio_item / toggle_mute: remote-session awareness (#88) ─────────

    fn make_remote_session(audio_only: bool) -> mbv_core::api::SessionInfo {
        mbv_core::api::SessionInfo {
            media_info: mbv_core::api::SessionMediaInfo {
                audio_only,
                ..Default::default()
            },
            ..crate::app::tests::make_session("device", "Emby")
        }
    }

    #[test]
    fn is_audio_item_reads_remote_session_audio_only_flag_when_true() {
        let mut app = crate::app::tests::make_app_stub();
        app.connected_session_id = Some("sess-1".into());
        app.connected_session_state = Some(make_remote_session(true));

        assert!(
            app.is_audio_item(),
            "a connected session's audio_only flag should decide is_audio_item(), \
             not local playlist/cursor state"
        );
    }

    #[test]
    fn is_audio_item_reads_remote_session_audio_only_flag_when_false() {
        let mut app = crate::app::tests::make_app_stub();
        app.connected_session_id = Some("sess-1".into());
        app.connected_session_state = Some(make_remote_session(false));

        assert!(!app.is_audio_item());
    }

    #[test]
    fn is_audio_item_falls_back_to_local_state_when_no_session() {
        let mut app = crate::app::tests::make_app_stub();
        assert!(app.connected_session_id.is_none());
        app.player_tab.items = vec![crate::app::tests::make_item("song", "Audio")];
        app.player_tab.queue_cursor = 0;

        assert!(app.is_audio_item());
    }

    #[test]
    fn toggle_mute_falls_back_to_cycle_audio_when_remote_session_connected() {
        // No session-level mute primitive exists (#88), so toggle_mute()
        // must hand off to cycle_audio()'s session-aware branch instead of
        // touching local ui_volume/pre_mute_volume state, which wouldn't
        // reflect a remote session's audio-only playback anyway.
        let mut app = crate::app::tests::make_app_stub();
        app.connected_session_id = Some("sess-1".into());
        app.connected_session_state = Some(make_remote_session(true));
        let ui_volume_before = app.ui_volume;

        app.toggle_mute();

        assert_eq!(
            app.ui_volume, ui_volume_before,
            "remote toggle_mute() must not touch local ui_volume state"
        );
        assert_eq!(
            app.connected_session_state.as_ref().unwrap().audio_index,
            2,
            "toggle_mute() should have delegated to cycle_audio()'s remote branch, \
             which advances the session's audio_index"
        );
    }

    // ── album_tracks_cache / LibEvent::AlbumTracksFetched (#145) ────────────
    // Proactive track-list fetch/cache for the Power View inline album
    // detail pane, mirroring the existing `album_artist_cache` pattern.

    #[test]
    fn album_tracks_fetched_event_populates_cache_and_clears_loading() {
        use crate::app::tests::make_item;

        let mut app = crate::app::tests::make_app_stub();
        app.album_tracks_loading.insert("album-1".into());

        let mut track = make_item("Opening Track", "Audio");
        track.id = "track-1".into();
        app.handle_lib_event(LibEvent::AlbumTracksFetched {
            album_id: "album-1".into(),
            tracks: vec![track],
        });

        assert!(
            !app.album_tracks_loading.contains("album-1"),
            "the loading marker must be cleared once the fetch resolves"
        );
        let cached = app
            .album_tracks_cache
            .get("album-1")
            .expect("fetched tracks must be cached under the album id");
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].id, "track-1");
    }

    #[test]
    fn fetch_album_tracks_is_a_no_op_when_already_cached() {
        let mut app = crate::app::tests::make_app_stub();
        app.album_tracks_cache.insert("album-1".into(), Vec::new());

        app.fetch_album_tracks("album-1".into());

        assert!(
            !app.album_tracks_loading.contains("album-1"),
            "a cache hit must return before marking the album as loading \
             (and before spawning a redundant network fetch)"
        );
    }

    #[test]
    fn fetch_album_tracks_is_a_no_op_when_already_loading() {
        let mut app = crate::app::tests::make_app_stub();
        app.album_tracks_loading.insert("album-1".into());

        app.fetch_album_tracks("album-1".into());

        assert!(
            !app.album_tracks_cache.contains_key("album-1"),
            "a duplicate call while a fetch is already in flight must not \
             spawn a second fetch or fabricate a cache entry"
        );
    }

    // #286: this used to redirect the process-wide STDERR_FILENO fd to
    // capture the bell byte, which raced against any other test ringing the
    // bell concurrently on a different thread (flash_status/flash_status_high
    // also ring it) and produced flaky doubled "\x07\x07" captures. Reading
    // `TEST_BELL_LOG` (thread-local, cleared per test thread) instead avoids
    // touching real stderr at all, so there's nothing left to race against.
    #[test]
    fn notify_with_actions_rings_terminal_bell_even_without_system_notifications() {
        TEST_BELL_LOG.with(|log| log.borrow_mut().clear());

        let app = crate::app::tests::make_app_stub();
        app.notify_with_actions("mbv", "Next up?", &[("next_up:play", "Play Now")]);

        let rung = TEST_BELL_LOG.with(|log| log.borrow().clone());
        assert_eq!(rung, b"\x07");
    }

    #[test]
    fn enqueue_selected_rejects_item_from_a_different_route_than_active_queue() {
        let mut app = make_app_stub();
        app.library_routes
            .insert("music".to_string(), "living-room-pc".to_string());
        app.active_route = Some("music".to_string());
        let mut movies_item = make_item("Movies", "CollectionFolder");
        movies_item.id = "lib-movies".to_string();
        app.libs.push(LibraryTab {
            library: movies_item,
            nav_stack: Vec::new(),
            search: None,
            feed_home_video: None,
            album_track_focus: None,
            artist_header_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: None,
        });
        app.tab_idx = app.lib_tab_offset();

        app.enqueue_selected();

        // `PlayerTab`/`PlaybackQueue`/`MediaItem` implement neither
        // `PartialEq` nor `Debug` in this codebase (confirmed: `MediaItem`
        // derives only `Debug, Clone, Serialize, Deserialize`, and
        // `PlayerTab` derives only `Clone, Default`), so a whole-struct
        // `assert_eq!` against a captured "before" clone will not compile.
        // The established idiom elsewhere in this test module (e.g. the
        // rollback-path tests) is to assert on `.items` directly instead
        // -- here that's simplest as "still empty", since `make_app_stub`
        // starts with an empty queue and a rejected enqueue must leave it
        // that way.
        assert!(app
            .queue_for_scope(app.visible_queue_scope())
            .items
            .is_empty());
        assert!(app.status.contains("Can't mix libraries in a routed queue"));
    }

    #[test]
    fn enqueue_route_conflict_allows_matching_route() {
        let mut app = make_app_stub();
        app.active_route = Some("music".to_string());
        assert!(!app.enqueue_route_conflict(Some("music".to_string())));
    }

    #[test]
    fn enqueue_route_conflict_allows_local_queue_local_item() {
        let mut app = make_app_stub();
        assert!(!app.enqueue_route_conflict(None));
    }

    #[test]
    fn enqueue_route_conflict_rejects_mismatched_route() {
        let mut app = make_app_stub();
        app.active_route = Some("music".to_string());
        assert!(app.enqueue_route_conflict(Some("movies".to_string())));
        assert!(app.status.contains("Can't mix libraries in a routed queue"));
    }

    #[test]
    fn enqueue_route_conflict_allows_enqueue_while_attached_to_a_session() {
        // A Sessions-panel attached session (`connected_session_id`) has
        // its own, separate queue-scope rules -- the library-routing
        // invariant must not fire a "Can't mix libraries" toast for a
        // reason unrelated to library routing.
        let mut app = make_app_stub();
        app.connected_session_id = Some("sess-1".to_string());
        assert!(!app.enqueue_route_conflict(Some("music".to_string())));
    }

    #[test]
    fn enqueue_route_conflict_allows_enqueue_while_on_a_non_route_direct_remote() {
        let mut app = make_app_stub();
        let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
        app.player = mbv_core::player::PlayerProxy::remote(remote, false);
        app.player_rx = remote_rx;
        // active_route stays None: this is a Sessions-panel direct-remote
        // connection, not a library route.
        assert!(!app.enqueue_route_conflict(Some("music".to_string())));
    }

    #[test]
    fn play_item_swaps_to_library_route_before_replacing_queue() {
        // #256: library-route resolution is now a pure config read -- no
        // live session lookup, no SESSIONS_LOAD_OVERRIDE seam needed here.
        // DAEMON_ROUTE_CONNECT_OVERRIDE is still needed: apply_route_for_playback
        // still performs a real connect to the resolved endpoint.
        let _guard = crate::config::TestStateDirGuard::new();
        let _connect_guard = crate::app::DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
        fn route_connect_success(
            _endpoint: &mbv_core::remote_player::DaemonEndpoint,
            _auth_token: &str,
        ) -> Result<
            (
                mbv_core::remote_player::RemotePlayer,
                mpsc::Receiver<PlayerEvent>,
            ),
            String,
        > {
            Ok(mbv_core::remote_player::RemotePlayer::stub(
                make_items(1),
                0,
            ))
        }
        *crate::app::DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(route_connect_success);

        let mut app = make_app_stub();
        app.library_routes
            .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
        let mut lib_item = make_item("Music", "CollectionFolder");
        lib_item.id = "lib-music".to_string();
        app.libs.push(LibraryTab {
            library: lib_item,
            nav_stack: Vec::new(),
            search: None,
            feed_home_video: None,
            album_track_focus: None,
            artist_header_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: None,
        });
        app.tab_idx = app.lib_tab_offset();
        let mut item = make_item("Song", "Audio");
        item.id = "song-1".to_string();

        app.play_item(item);

        *crate::app::DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
        assert_eq!(app.active_route.as_deref(), Some("music"));
    }

    #[test]
    fn play_item_skips_library_routing_when_attached_to_a_session() {
        let mut app = make_app_stub();
        app.library_routes
            .insert("music".to_string(), "living-room-pc".to_string());
        app.connected_session_id = Some("sess-1".to_string());
        let mut lib_item = make_item("Music", "CollectionFolder");
        lib_item.id = "lib-music".to_string();
        app.libs.push(LibraryTab {
            library: lib_item,
            nav_stack: Vec::new(),
            search: None,
            feed_home_video: None,
            album_track_focus: None,
            artist_header_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: None,
        });
        app.tab_idx = app.lib_tab_offset();
        let mut item = make_item("Song", "Audio");
        item.id = "song-1".to_string();

        // No DAEMON_ROUTE_CONNECT_OVERRIDE set -- if library routing
        // engaged here it would attempt a real connection and this test
        // would hang/fail rather than reach the assertion below.
        app.play_item(item);

        assert!(app.active_route.is_none());
    }

    #[test]
    fn play_item_skips_library_routing_when_already_direct_remote_via_sessions_panel() {
        // Regression guard for the gap `connected_session_id.is_none()`
        // alone misses: a Sessions-panel "Direct Remote" ctrl-socket
        // upgrade leaves `connected_session_id` as `None` but
        // `self.player.is_remote()` `true` and `active_route` `None`.
        // Library routing must not engage here either -- it would swap
        // `self.player` out from under the active direct-remote
        // connection without ever clearing `direct_remote_label`.
        let mut app = make_app_stub();
        app.library_routes
            .insert("music".to_string(), "living-room-pc".to_string());
        let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
        let sess = crate::app::tests::make_session("other-mbv", "mbv");
        app.switch_to_direct_remote(&sess, remote, remote_rx);
        assert!(app.player.is_remote());
        assert!(app.active_route.is_none());

        let mut lib_item = make_item("Music", "CollectionFolder");
        lib_item.id = "lib-music".to_string();
        app.libs.push(LibraryTab {
            library: lib_item,
            nav_stack: Vec::new(),
            search: None,
            feed_home_video: None,
            album_track_focus: None,
            artist_header_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: None,
        });
        app.tab_idx = app.lib_tab_offset();
        let mut item = make_item("Song", "Audio");
        item.id = "song-1".to_string();

        // No DAEMON_ROUTE_CONNECT_OVERRIDE set -- if library routing
        // engaged here it would attempt a real connection and this test
        // would hang/fail rather than reach the assertion below.
        app.play_item(item);

        assert!(app.active_route.is_none());
    }

    fn lib_tab(collection_type: &str) -> LibraryTab {
        let mut library = make_item("Lib", "CollectionFolder");
        library.id = "lib-1".into();
        library.collection_type = collection_type.into();
        LibraryTab {
            library,
            nav_stack: Vec::new(),
            search: None,
            feed_home_video: None,
            album_track_focus: None,
            artist_header_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: None,
        }
    }

    #[test]
    fn active_lib_is_tvshows_true_only_on_a_tvshows_library_tab() {
        // `shuffle_folder` (issue: TV libraries should shuffle from a
        // video-only fetch, everything else from the broader playable-items
        // fetch) branches on this. Covers both view modes indirectly, since
        // `lib_tab_offset()` is view-mode-independent -- the tab_idx math is
        // identical whether the active library tab was reached through the
        // standard tab bar or Power View's left panel.
        let mut app = make_app_stub();
        app.libs.push(lib_tab("tvshows"));
        app.libs.push(lib_tab("music"));

        app.tab_idx = app.lib_tab_offset();
        assert!(
            app.active_lib_is_tvshows(),
            "tab_idx on the tvshows library tab"
        );

        app.tab_idx = app.lib_tab_offset() + 1;
        assert!(
            !app.active_lib_is_tvshows(),
            "tab_idx on the music library tab"
        );
    }

    #[test]
    fn active_lib_is_tvshows_false_outside_any_library_tab() {
        let mut app = make_app_stub();
        app.libs.push(lib_tab("tvshows"));

        app.tab_idx = 0; // home
        assert!(!app.active_lib_is_tvshows());

        app.tab_idx = 1; // queue
        assert!(!app.active_lib_is_tvshows());
    }

    /// Pushes a top-level, non-loading, non-searching `BrowseLevel` onto
    /// `lib`'s nav_stack -- the minimum state `should_show_letter_pills`
    /// needs to consider the library "at its top browse level".
    fn push_top_level(lib: &mut LibraryTab, item_count: usize) {
        lib.nav_stack.push(BrowseLevel {
            parent_id: lib.library.id.clone(),
            title: lib.library.name.clone(),
            items: make_items(item_count),
            total_count: item_count,
            cursor: 0,
            scroll: 0,
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
        });
    }

    #[test]
    fn should_show_letter_pills_requires_library_total_over_threshold() {
        let mut app = make_app_stub();
        app.libs.push(lib_tab("movies"));
        push_top_level(&mut app.libs[0], 10);

        // No captured library_total yet -> hidden even if the fetched-so-far
        // count is small.
        assert!(!app.should_show_letter_pills(0));

        app.libs[0].library_total = Some(300);
        assert!(
            !app.should_show_letter_pills(0),
            "300 is the threshold, not over it"
        );

        app.libs[0].library_total = Some(301);
        assert!(app.should_show_letter_pills(0));
    }

    #[test]
    fn should_show_letter_pills_excludes_music_search_and_drilldowns() {
        let mut app = make_app_stub();
        app.libs.push(lib_tab("music"));
        push_top_level(&mut app.libs[0], 10);
        app.libs[0].library_total = Some(1000);
        assert!(
            !app.should_show_letter_pills(0),
            "music libraries use group pills instead"
        );

        app.libs.push(lib_tab("movies"));
        push_top_level(&mut app.libs[1], 10);
        app.libs[1].library_total = Some(1000);
        assert!(app.should_show_letter_pills(1));

        app.libs[1].search = Some(crate::app::LibSearch {
            query: String::new(),
            items: Vec::new(),
            results: Vec::new(),
            cursor: 0,
            scroll: 0,
            loading: false,
        });
        assert!(!app.should_show_letter_pills(1), "hidden while searching");
        app.libs[1].search = None;

        // A second nav level (drilled into a folder) is no longer the "top"
        // browse level.
        push_top_level(&mut app.libs[1], 5);
        assert!(
            !app.should_show_letter_pills(1),
            "hidden below the top browse level"
        );
    }

    #[test]
    fn select_letter_pill_scopes_the_level_and_resets_cursor() {
        let mut app = make_app_stub();
        app.libs.push(lib_tab("movies"));
        push_top_level(&mut app.libs[0], 10);
        app.libs[0].library_total = Some(1000);
        app.libs[0].nav_stack[0].cursor = 4;
        app.libs[0].nav_stack[0].scroll = 2;

        app.select_letter_pill(0, 4); // "M–O"

        let lvl = app.libs[0].nav_stack.last().unwrap();
        let filter = lvl.letter_filter.as_ref().expect("pill should be set");
        assert_eq!(filter.index, 4);
        assert_eq!(filter.label, "M\u{2013}O");
        assert_eq!(filter.name_ge, Some("M"));
        assert_eq!(filter.name_lt, Some("P"));
        assert_eq!(lvl.cursor, 0);
        assert_eq!(lvl.scroll, 0);
        assert!(lvl.loading, "a scoped refresh should be in flight");
    }

    #[test]
    fn select_letter_pill_is_a_noop_outside_letter_pill_eligibility() {
        let mut app = make_app_stub();
        app.libs.push(lib_tab("movies"));
        push_top_level(&mut app.libs[0], 10);
        // library_total never captured -> should_show_letter_pills is false.

        app.select_letter_pill(0, 0);

        assert!(app.libs[0]
            .nav_stack
            .last()
            .unwrap()
            .letter_filter
            .is_none());
    }

    #[test]
    fn cycle_letter_pill_wraps_around() {
        let mut app = make_app_stub();
        app.libs.push(lib_tab("movies"));
        push_top_level(&mut app.libs[0], 10);
        app.libs[0].library_total = Some(1000);

        // Default (no pill selected yet) is treated as index 0; cycling back
        // wraps to the last bucket ("#").
        app.cycle_letter_pill(0, -1);
        let filter = app.libs[0]
            .nav_stack
            .last()
            .unwrap()
            .letter_filter
            .as_ref()
            .unwrap();
        assert_eq!(filter.label, "#");
    }

    // Regression coverage for the bug found in review of the letter-pills
    // PR: `spawn_all_items_prefetch`/`spawn_search_items_load` used to cap
    // their unfiltered fetch's `limit` at `lvl.total_count`, which is the
    // FILTERED range's count whenever a letter pill is active (e.g. ~40 for
    // an `M–O` pill out of a 3,000-movie library) -- so `all_items` (the set
    // `/`-search runs over) silently shrank to just the active range, and
    // whole-library search missed everything outside it.
    #[test]
    fn full_library_fetch_limit_uses_true_total_not_the_filtered_range_count() {
        let mut lib = lib_tab("movies");
        push_top_level(&mut lib, 40); // the "M–O" slice: 40 items
        lib.library_total = Some(3000); // the library's true size
        {
            let lvl = lib.nav_stack.last_mut().unwrap();
            lvl.total_count = 40; // what get_items_sorted_ranged reported for M–O
            lvl.letter_filter = crate::app::render::power::LetterFilter::for_index(4);
        }
        let lvl = lib.nav_stack.last().unwrap();

        assert_eq!(
            full_library_fetch_limit(&lib, lvl),
            3000,
            "must fetch the whole library, not just the active M–O range"
        );
    }

    #[test]
    fn full_library_fetch_limit_falls_back_to_total_count_before_library_total_is_known() {
        let mut lib = lib_tab("movies");
        push_top_level(&mut lib, 10);
        // library_total not yet captured (e.g. first-ever load in flight).
        let lvl = lib.nav_stack.last().unwrap();
        assert_eq!(full_library_fetch_limit(&lib, lvl), 10);
    }
}
#[test]
fn enqueue_action_context_names_action_item_and_thin_client_bypass() {
    assert_eq!(
            enqueue_action_context("item-42", "Track", "library-view", true),
            "user action=enqueue item_id=\"item-42\" item_name=\"Track\" source=library-view reason=non-library thin-client owns playback"
        );
}
