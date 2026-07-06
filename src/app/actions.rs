use super::ui_util::{is_playable, natural_sort_key, sort_audio_tracks, sort_episodes};
use super::{
    App, BrowseLevel, ContextAction, FeedHomeVideoGroup, FeedHomeVideoState, LibEvent,
    PendingQueueAction, PowerFocus, QueueScope, SessionEvent, PAGE_SIZE, PLAYLIST_VIEW_POWER,
    PREFETCH_AHEAD,
};
use crate::api::{EmbyClient, MediaItem, TICKS_PER_SECOND};
use crate::player::PlayerCommand;
use crate::ws::WsEvent;
use rand::seq::SliceRandom;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

type BrowseRefresh = (usize, String, Option<String>, bool, String, String, usize);

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
    if let Some(id) = last_played_item_id {
        let idx = items.iter().position(|i| i.id == id).unwrap_or(0);
        if last_played_completed {
            (idx + 1).min(items.len().saturating_sub(1))
        } else {
            idx
        }
    } else {
        saved_cursor.min(items.len().saturating_sub(1))
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
            "{context}: lib_idx={lib_idx} lib={} nav_len={} root_parent={} root_items={} root_loading={} root_cursor={} search={} detail={} feed_present={} feed_loading={} selected_group={} groups={} all_items={} video_cursor={} video_scroll={} group_view={}",
            lib.library.name,
            lib.nav_stack.len(),
            root.map(|lvl| lvl.parent_id.as_str()).unwrap_or(""),
            root.map(|lvl| lvl.items.len()).unwrap_or(0),
            root.map(|lvl| lvl.loading).unwrap_or(false),
            root.map(|lvl| lvl.cursor).unwrap_or(0),
            lib.search.is_some(),
            lib.power_detail_item.is_some(),
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
        if lib.nav_stack.len() != 1 || lib.search.is_some() || lib.power_detail_item.is_some() {
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
    /// fully paginated: power view is showing this library's tab, it's a
    /// feed-home-video or podcast library, and its root nav level has loaded
    /// every item. `extra_ok` carries the caller-specific condition (e.g.
    /// which event/level this check is reacting to).
    fn should_aggregate_feed(
        &self,
        lib_idx: usize,
        extra_ok: impl FnOnce(&BrowseLevel) -> bool,
    ) -> bool {
        self.playlist_view == PLAYLIST_VIEW_POWER
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
        // In power view the library list is rendered into the right panel, and the
        // normal-view per-row height map (`layout.library.lib_row_heights`) is never populated,
        // so it would fall back to 1. Use the panel height directly (rows are single-line;
        // subtract 1 for the count/search header line).
        if self.playlist_view == PLAYLIST_VIEW_POWER {
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

    pub(super) fn playlist_page_size(&self) -> usize {
        self.layout.playlist.inner.height.saturating_sub(2).max(1) as usize
    }

    pub(super) fn move_lib_cursor(&mut self, delta: i64) {
        let now = Instant::now();
        let idle = now.duration_since(self.last_nav_at) >= Duration::from_millis(150);
        self.last_nav_at = now;
        let lib_off = self.lib_tab_offset();
        let lib_idx = self.tab_idx - lib_off;

        if self.libs[lib_idx].search.is_none() && self.is_feed_home_video_group_view(lib_idx) {
            if let Some(state) = self.libs[lib_idx].feed_home_video.as_mut() {
                let n = state.selected_len();
                if n > 0 {
                    state.video_cursor =
                        (state.video_cursor as i64 + delta).clamp(0, n as i64 - 1) as usize;
                }
            }
            return;
        }

        // In power view with letter-grouped display, navigate in sorted display order so
        // the cursor follows what the user sees (articles stripped) rather than raw item order.
        if self.playlist_view == PLAYLIST_VIEW_POWER
            && !self.layout.power.left_sorted_indices.is_empty()
        {
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
            }
        }
        if idle {
            self.maybe_fetch_next_page(lib_idx);
        }
    }

    pub(super) fn jump_lib_cursor(&mut self, to_end: bool) {
        let lib_off = self.lib_tab_offset();
        let lib_idx = self.tab_idx - lib_off;

        if self.libs[lib_idx].search.is_none() && self.is_feed_home_video_group_view(lib_idx) {
            if let Some(state) = self.libs[lib_idx].feed_home_video.as_mut() {
                let n = state.selected_len();
                if n > 0 {
                    state.video_cursor = if to_end { n - 1 } else { 0 };
                }
            }
            return;
        }

        // In power view with letter-grouped display, Home/End jump to the first/last item
        // in sorted display order (article-stripped), not raw item order.
        if self.playlist_view == PLAYLIST_VIEW_POWER
            && !self.layout.power.left_sorted_indices.is_empty()
        {
            let needs_sorted = self.libs[lib_idx].search.is_none()
                && !self.layout.power.left_sorted_indices.is_empty();
            if needs_sorted {
                let n = self.layout.power.left_sorted_indices.len();
                let new_cursor =
                    self.layout.power.left_sorted_indices[if to_end { n - 1 } else { 0 }];
                if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut() {
                    lvl.cursor = new_cursor;
                }
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
        if let Some(ref hs) = self.home_search {
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
        let tx = self.search_tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(client.search_items(&query, 100));
        });
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
            if self.is_feed_home_video_group_view(self.tab_idx - self.lib_tab_offset()) {
                return self.selected_feed_home_video_item(self.tab_idx - self.lib_tab_offset());
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
    /// True when the power view should show the combined series view:
    /// either at episode level (with a Season level directly above), or at
    /// season level while episodes are still loading.
    pub(super) fn is_series_view(&self, lib_idx: usize) -> bool {
        let lib = &self.libs[lib_idx];
        if lib.search.is_some() {
            return false;
        }
        let lvl = match lib.nav_stack.last() {
            Some(l) => l,
            None => return false,
        };
        // Normal state: at episode level with a season list one level up.
        if lvl
            .items
            .first()
            .map(|i| i.item_type == "Episode")
            .unwrap_or(false)
        {
            let len = lib.nav_stack.len();
            return len >= 2
                && lib.nav_stack[len - 2]
                    .items
                    .first()
                    .map(|i| i.item_type == "Season")
                    .unwrap_or(false);
        }
        // Transitional state: switch_season pushed an empty loading BrowseLevel
        // above the season level. Neither branch above fires for empty items, so
        // detect this explicitly and keep treating it as a series view while the
        // new season's episodes load (prevents flashing the queue image).
        if lvl.loading && lvl.items.is_empty() {
            let len = lib.nav_stack.len();
            return len >= 2
                && lib.nav_stack[len - 2]
                    .items
                    .first()
                    .map(|i| i.item_type == "Season")
                    .unwrap_or(false);
        }
        // At-season level: browsing seasons before drilling into episodes.
        lvl.items
            .first()
            .map(|i| i.item_type == "Season")
            .unwrap_or(false)
    }

    /// True when the power view should show the combined music group view:
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
        if lib.power_detail_item.is_some() {
            return false;
        }
        lib.library.collection_type == "homevideos"
    }

    pub(super) fn is_feed_home_video_group_view(&self, lib_idx: usize) -> bool {
        let lib = &self.libs[lib_idx];
        if lib.power_detail_item.is_some() || lib.search.is_some() {
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
            ..FeedHomeVideoState::default()
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
            ..FeedHomeVideoState::default()
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

    /// Switch to the previous (`delta == -1`) or next (`delta == 1`) season
    /// while in the combined series view. Pops the current episode level,
    /// adjusts the season cursor, then kicks off a fetch for the new season.
    pub(super) fn switch_season(&mut self, lib_idx: usize, delta: i64) {
        let stack_len = self.libs[lib_idx].nav_stack.len();
        if stack_len < 2 {
            return;
        }
        let at_episodes = self.libs[lib_idx]
            .nav_stack
            .last()
            .map(|l| {
                l.items
                    .first()
                    .map(|i| i.item_type == "Episode")
                    .unwrap_or(false)
            })
            .unwrap_or(false);
        if !at_episodes {
            return;
        }

        // Check season count before popping so we never lose the episode level.
        let n = self.libs[lib_idx].nav_stack[stack_len - 2].items.len();
        if n == 0 {
            return;
        }

        // Pop the episode level.
        self.libs[lib_idx].nav_stack.pop();
        let cur = self.libs[lib_idx]
            .nav_stack
            .last()
            .map(|l| l.cursor)
            .unwrap_or(0);
        let new_cursor = (cur as i64 + delta).clamp(0, n as i64 - 1) as usize;
        if let Some(season_lvl) = self.libs[lib_idx].nav_stack.last_mut() {
            season_lvl.cursor = new_cursor;
        }

        // Reset per-episode scroll/detail state.
        self.libs[lib_idx].power_detail_scroll = 0;

        // Collect the new season's identity.
        let (season_id, season_name) = self.libs[lib_idx]
            .nav_stack
            .last()
            .and_then(|l| l.items.get(new_cursor))
            .map(|s| (s.id.clone(), s.name.clone()))
            .unwrap_or_default();
        if season_id.is_empty() {
            return;
        }

        // Push a loading placeholder so the Loaded handler can fill it in.
        self.libs[lib_idx].nav_stack.push(BrowseLevel {
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
        });
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
        if self.connected_session_id.is_some() {
            return self
                .connected_session_state
                .as_ref()
                .map(|s| s.media_info.audio_only)
                .unwrap_or(false);
        }
        let idx = self.player_tab.playlist_cursor;
        self.player_tab
            .items
            .get(idx)
            .map(|i| i.media_type == "Audio" || i.item_type == "Audio")
            .unwrap_or(false)
    }

    pub(super) fn toggle_mute(&mut self) {
        if self.connected_session_id.is_some() {
            // No session-level mute primitive exists (see #88's "out of
            // scope" decision, to avoid inventing new session-command
            // plumbing), so fall back to cycling the session's audio
            // stream via `cycle_audio()`'s existing session-aware branch.
            self.cycle_audio();
            return;
        }
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
        self.save_prefs();
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
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            let remote_indexes = self.remote_audio_indexes();
            let cur = self
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
        if tracks.is_empty() {
            return;
        }
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

    /// Clone the current subtitle prefs from the shared Arc and notify the player thread.
    pub(super) fn push_subtitle_prefs(&self) {
        let prefs = self.player.subtitle_prefs.lock().unwrap().clone();
        self.player
            .send_command(crate::player::PlayerCommand::SetSubtitlePrefs {
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
        crate::config::save_config_settings(&cfg);
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
        if self.connected_session_id.is_some() {
            let remote_indexes = self.remote_subtitle_indexes();
            if remote_indexes.is_empty() {
                self.toggle_sub();
                return;
            }
            let current = self
                .connected_session_state
                .as_ref()
                .map(|s| s.sub_index)
                .unwrap_or(-1);
            let mut entries = Vec::with_capacity(remote_indexes.len() + 1);
            entries.push(-1);
            entries.extend(remote_indexes);
            let next = Self::next_subtitle_entry(&entries, current);
            let id = self.connected_session_id.clone().unwrap();
            if let Some(ref mut state) = self.connected_session_state {
                state.sub_index = next;
            }
            self.do_session_command(move |c| c.session_set_subtitle_index(&id, next));
            return;
        }
        // When idle: cycle the default subtitle mode instead of a track --
        // there's no session equivalent to unify this fallback with (#86).
        let (active, tracks, current_id) = {
            let s = self.player.status.lock().unwrap();
            (s.active, s.sub_tracks.clone(), s.sub_id)
        };
        if !active {
            self.cycle_subtitle_mode();
            return;
        }
        if tracks.is_empty() {
            return;
        }
        let mut entries: Vec<i64> = vec![0];
        entries.extend(tracks.iter().map(|(id, _, _)| *id));
        let next_id = Self::next_subtitle_entry(&entries, current_id);
        self.player.send_command(PlayerCommand::SetSub(next_id));
        self.save_prefs();
    }

    pub(super) fn remove_from_playlist(&mut self, pos: usize) {
        let scope = self.displayed_queue_scope();
        let controls_playback_queue = self.queue_scope_is_playback(scope);
        let (active, current_idx) = {
            let s = self.player.status.lock().unwrap();
            (s.active, s.current_idx)
        };
        if scope == QueueScope::Remote && !active {
            self.flash_status_high("Remote queue can only be edited while active".into());
            return;
        }
        if controls_playback_queue && active && current_idx == pos {
            self.confirm_remove_idx = Some(pos);
            self.status = "Remove now-playing item and stop playback? (y/N)".into();
            self.status_expires = None;
            return;
        }
        let item = self.queue_for_scope_mut(scope).items.remove(pos);
        if self.queue_scope_has_local_metadata(scope) {
            self.queue_dirty = true;
        }
        self.undo_stack_for_scope_mut(scope).push((pos, item));
        self.persist_local_queue_state_if_needed(scope);
        if controls_playback_queue && active {
            self.player.send_command(PlayerCommand::PlaylistRemove(pos));
            // Player thread adjusts current_idx when it processes the command.
            // No eager adjustment here — doing so races with the player thread
            // and can cause index mismatches during rapid removals.
        }
        let queue = self.queue_for_scope_mut(scope);
        if !queue.items.is_empty() {
            queue.playlist_cursor = queue.playlist_cursor.min(queue.items.len() - 1);
        } else {
            queue.playlist_cursor = 0;
        }
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

    pub(super) fn notify_with_actions(&self, title: &str, body: &str, actions: &[(&str, &str)]) {
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
        let client = self.client.lock().unwrap().clone();
        let library_id = self.libs[lib_idx].library.id.clone();
        let name = self.libs[lib_idx].library.name.clone();
        std::thread::spawn(move || {
            let _ = client.post_library_refresh(&library_id);
        });
        self.flash_status(format!("Scanning '{name}'..."));
    }

    pub(super) fn flash_status(&mut self, msg: String) {
        self.notify_system(&msg);
        self.status = msg;
        self.status_expires = Some(Instant::now() + Duration::from_secs(2));
    }

    pub(super) fn flash_status_high(&mut self, msg: String) {
        self.notify_system(&msg);
        self.status = msg;
        self.status_expires = Some(Instant::now() + Duration::from_secs(5));
    }

    pub(super) fn effective_playback_state(&self) -> (bool, usize, i64, i64, bool) {
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
                (pos_s * crate::api::TICKS_PER_SECOND as f64) as i64
            };
            (
                remote.now_playing.is_some() && maybe_active_idx.is_some(),
                active_idx,
                pos_ticks,
                remote.runtime_s * crate::api::TICKS_PER_SECOND,
                remote.is_paused,
            )
        } else {
            let s = self.player.status.lock().unwrap();
            (
                s.active,
                s.current_idx,
                s.position_ticks,
                s.runtime_ticks,
                s.paused,
            )
        }
    }

    pub(super) fn play_items_routed(&mut self, items: Vec<MediaItem>, start_idx: usize) {
        self.on_queue_replace_silent();
        self.set_queue_scope(self.playback_queue_scope());
        // Keep library focus when playing from the power-view library panel.
        if !(self.playlist_view == PLAYLIST_VIEW_POWER
            && matches!(self.power_focus, PowerFocus::Left))
        {
            self.power_focus = PowerFocus::Queue;
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
        self.player
            .play_playlist(items, start_idx, c, self.ui_volume);
        self.player
            .send_command(PlayerCommand::SetMute(self.mute_on));
    }

    pub(super) fn play_item(&mut self, item: MediaItem) {
        self.on_queue_replace_silent();
        // Keep library focus when playing from the power-view library panel.
        if !(self.playlist_view == PLAYLIST_VIEW_POWER
            && matches!(self.power_focus, PowerFocus::Left))
        {
            self.power_focus = PowerFocus::Queue;
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
                self.player.play_playlist(episodes, 0, c, self.ui_volume);
                self.player
                    .send_command(PlayerCommand::SetMute(self.mute_on));
                if !self.has_direct_remote_queue() {
                    self.queue_source = crate::config::QueueSource::Series;
                    self.save_queue_state();
                }
                return;
            }
        }
        let c = Arc::new(self.client.lock().unwrap().clone());
        self.replace_playback_queue(vec![item.clone()], 0);
        self.player.play(&item, c, self.ui_volume);
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
            let name = item.display_name();
            let scope = self.displayed_queue_scope();
            {
                self.queue_for_scope_mut(scope).items.push(item);
            }
            if self.queue_scope_has_local_metadata(scope) {
                self.queue_dirty = true;
            }
            self.flash_status(format!("Added: {name}"));
            self.persist_local_queue_state_if_needed(scope);
            self.sync_direct_remote_queue_after_edit(scope);
        } else if self.tab_idx >= 2 && self.tab_idx != self.log_tab_idx() {
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
            let name = item.display_name();
            let scope = self.displayed_queue_scope();
            {
                self.queue_for_scope_mut(scope).items.push(item);
            }
            if self.queue_scope_has_local_metadata(scope) {
                self.queue_dirty = true;
            }
            self.flash_status(format!("Added: {name}"));
            self.persist_local_queue_state_if_needed(scope);
            self.sync_direct_remote_queue_after_edit(scope);
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
                if count == 0 {
                    self.flash_status_high("Nothing to enqueue".into());
                    return;
                }
                let scope = self.displayed_queue_scope();
                {
                    let queue = self.queue_for_scope_mut(scope);
                    queue.items.extend(items);
                }
                if self.queue_scope_has_local_metadata(scope) {
                    self.queue_dirty = true;
                }
                self.flash_status(format!(
                    "Enqueued {count} items from {}",
                    item.display_name()
                ));
                self.persist_local_queue_state_if_needed(scope);
                self.sync_direct_remote_queue_after_edit(scope);
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
            });
            if let Some(v) = self.layout.library.lib_scroll.get_mut(lib_idx) {
                *v = 0;
            }
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
            if self.libs[lib_idx].search.is_none() && self.is_album_level(lib_idx) {
                let level_items = self.libs[lib_idx]
                    .nav_stack
                    .last()
                    .map(|l| l.items.clone())
                    .unwrap_or_default();
                let mut tracks: Vec<MediaItem> =
                    level_items.into_iter().filter(is_playable).collect();
                sort_audio_tracks(&mut tracks);
                if let Some(start_idx) = tracks.iter().position(|i| i.id == fresh.id) {
                    self.replace_playback_queue(tracks.clone(), start_idx);
                    self.play_items_routed(tracks, start_idx);
                    if !self.has_direct_remote_queue() {
                        self.queue_source = crate::config::QueueSource::Album;
                        self.save_queue_state();
                    }
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
                                self.play_items_routed(siblings, start_idx);
                                if !self.has_direct_remote_queue() {
                                    self.queue_source = crate::config::QueueSource::Collection {
                                        collection_type: ct,
                                    };
                                    self.save_queue_state();
                                }
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
        if self.tab_idx > 1 && self.tab_idx != self.log_tab_idx() {
            let lib_off = self.lib_tab_offset();
            let lib_idx = self.tab_idx - lib_off;

            // Guard: don't pop when already at the root of a synthetic "group" view
            // (music groups: nav_stack[0]=groups, nav_stack[1]=albums; feed home
            // videos: nav_stack[0]=folders, nav_stack[1]=grouped videos) -- there is
            // no list above to go back to. Search-clearing still falls through
            // because this guard only fires when search is None.
            if self.playlist_view == PLAYLIST_VIEW_POWER
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

                // In the power view, skip past the auto-pushed Season level so
                // a single Escape takes the user back to the series list.
                if self.playlist_view == PLAYLIST_VIEW_POWER && self.power_left_tab == lib_idx + 1 {
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
        }
    }
    pub(super) fn execute_context_action(&mut self, action: Option<ContextAction>) {
        match action {
            Some(ContextAction::Play) => {
                if self.playlist_view == super::PLAYLIST_VIEW_POWER
                    && matches!(self.power_focus, PowerFocus::Left)
                    && self.power_left_tab == 0
                {
                    self.power_cw_play();
                } else if self.tab_idx == 0 {
                    self.select_home();
                } else if self.tab_idx == 1 {
                    let scope = self.displayed_queue_scope();
                    let queue = self.displayed_queue();
                    let t = queue.playlist_cursor;
                    if t < queue.items.len() {
                        if let Some(ref conn_id) = self.connected_session_id.clone() {
                            let item = queue.items[t].clone();
                            let id = conn_id.clone();
                            let item_ids: Vec<String> =
                                queue.items.iter().map(|i| i.id.clone()).collect();
                            let start_ticks = item.playback_position_ticks;
                            let label = item.playback_label();
                            self.flash_status(format!("Playing on remote: {label}"));
                            self.do_session_command(move |c| {
                                c.session_play_items(&id, &item_ids, t, start_ticks)
                            });
                        } else {
                            let st = self.player.status.lock().unwrap();
                            let active = st.active;
                            let current_idx = st.current_idx;
                            drop(st);
                            if active && self.queue_scope_is_playback(scope) {
                                if t != current_idx {
                                    self.player.send_command(PlayerCommand::JumpTo(t));
                                }
                            } else {
                                let items = queue.items.clone();
                                let c = Arc::new(self.client.lock().unwrap().clone());
                                self.replace_playback_queue(items.clone(), t);
                                self.player.play_playlist(items, t, c, self.ui_volume);
                            }
                        }
                    }
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
                self.play_folder(&id);
                self.queue_source = crate::config::QueueSource::Collection {
                    collection_type: ct,
                };
                self.save_queue_state();
            }
            Some(ContextAction::ShuffleFolder(id)) => {
                self.shuffle_folder(&id);
            }
            Some(ContextAction::Enqueue) => {
                if self.playlist_view == super::PLAYLIST_VIEW_POWER
                    && matches!(self.power_focus, PowerFocus::Left)
                    && self.power_left_tab == 0
                {
                    self.power_cw_enqueue();
                } else {
                    self.enqueue_selected();
                }
            }
            Some(ContextAction::EnqueueFolder(item)) => self.do_enqueue_folder((*item).clone()),
            Some(ContextAction::MarkPlayed(id)) => self.context_set_played(&id, true),
            Some(ContextAction::MarkItemsPlayed(ids)) => self.context_set_many_played(&ids),
            Some(ContextAction::MarkUnplayed(id)) => self.context_set_played(&id, false),
            Some(ContextAction::MarkItemsUnplayed(ids)) => self.context_set_many_unplayed(&ids),
            Some(ContextAction::RemoveFromContinueWatching) => self.remove_from_continue_watching(),
            Some(ContextAction::RemoveFromPlaylist(pos)) => self.remove_from_playlist(pos),
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
                    } else if self.playlist_view == super::PLAYLIST_VIEW_POWER
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
        let lib_idx = if self.tab_idx > 1 && self.tab_idx != self.log_tab_idx() {
            self.tab_idx - self.lib_tab_offset()
        } else if self.playlist_view == PLAYLIST_VIEW_POWER
            && matches!(self.power_focus, PowerFocus::Left)
            && self.power_left_tab > 0
        {
            self.power_left_tab - 1
        } else {
            return;
        };
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
            self.spawn_refresh(
                lib_idx,
                parent_id,
                item_types,
                unplayed_only,
                sort_by,
                sort_order,
                loaded_count,
            );
        }
    }

    fn refresh_queue(&mut self) {
        let scope = self.displayed_queue_scope();
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
            let mut map: HashMap<String, crate::api::MediaItem> =
                fetched.into_iter().map(|i| (i.id.clone(), i)).collect();
            drop(client);
            for item in &mut self.queue_for_scope_mut(scope).items {
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
                self.flash_status_high(format!("Refresh error: {e}"));
            }
        } else if self.tab_idx == 1 {
            self.refresh_queue();
        } else if self.tab_idx != self.log_tab_idx() {
            self.refresh_lib();
        }
    }

    pub(super) fn shuffle_play(&mut self) {
        if self.tab_idx <= 1 || self.tab_idx == self.log_tab_idx() {
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
        let client = self.client.lock().unwrap();
        match client.get_all_videos_recursive(&parent_id) {
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
                self.play_items_routed(items, 0);
                if !self.has_direct_remote_queue() {
                    self.queue_source = crate::config::QueueSource::Shuffle;
                    self.save_queue_state();
                }
            }
            Err(e) => {
                let msg = format!("Error: {e}");
                drop(client);
                self.flash_status_high(msg);
            }
        }
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

    pub(super) fn shuffle_folder(&mut self, folder_id: &str) {
        let client = self.client.lock().unwrap();
        match client.get_all_playable_recursive(folder_id) {
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
                self.play_items_routed(items, 0);
                if !self.has_direct_remote_queue() {
                    self.queue_source = crate::config::QueueSource::Shuffle;
                    self.save_queue_state();
                }
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
        } else {
            self.ensure_library_loaded();
        }
    }

    pub(super) fn ensure_library_loaded(&mut self) {
        if self.tab_idx <= 1 || self.tab_idx == self.log_tab_idx() {
            return;
        }
        let idx = self.tab_idx - self.lib_tab_offset();
        self.ensure_lib_loaded_for(idx);
    }

    pub(super) fn ensure_lib_loaded_for(&mut self, idx: usize) {
        if idx >= self.libs.len() {
            return;
        }
        if self.playlist_view == PLAYLIST_VIEW_POWER
            && self.power_left_tab == idx + 1
            && self.is_feed_home_video_library(idx)
        {
            self.ensure_feed_home_video_root_loaded(idx);
            return;
        }
        if self.playlist_view == PLAYLIST_VIEW_POWER
            && self.power_left_tab == idx + 1
            && self.is_podcast_library(idx)
        {
            self.ensure_podcast_root_loaded(idx);
            return;
        }
        if self.libs[idx].nav_stack.is_empty() {
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
                    )
                })
            })
            .collect();
        for (lib_idx, parent_id, item_types, unplayed_only, sort_by, sort_order, loaded_count) in
            fetches
        {
            self.spawn_refresh(
                lib_idx,
                parent_id,
                item_types,
                unplayed_only,
                sort_by,
                sort_order,
                loaded_count,
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
                    log::info!(target: "browse", "Loaded lib_idx={lib_idx} parent={parent_id} total={total_count} got={} first3={:?}",
                        items.len(),
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
                });
            }
            let _ = tx.send(LibEvent::NavigateTo {
                lib_idx,
                nav_stack,
                switch_tab: true,
            });
        });
    }

    fn spawn_browse_page(
        &self,
        lib_idx: usize,
        parent_id: String,
        start_index: usize,
        item_types: Option<String>,
        unplayed_only: bool,
        sort_by: String,
        sort_order: String,
    ) {
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            match client.get_items_sorted(
                &parent_id,
                item_types.as_deref(),
                unplayed_only,
                start_index,
                PAGE_SIZE,
                &sort_by,
                &sort_order,
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
        if lvl.is_fully_loaded() {
            return;
        }
        let parent_id = lvl.parent_id.clone();
        let total_count = lvl.total_count;
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
        let total_count = lvl.total_count;
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

    fn spawn_refresh(
        &self,
        lib_idx: usize,
        parent_id: String,
        item_types: Option<String>,
        unplayed_only: bool,
        sort_by: String,
        sort_order: String,
        loaded_count: usize,
    ) {
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        let limit = loaded_count.max(PAGE_SIZE);
        std::thread::spawn(move || {
            match client.get_items_sorted(
                &parent_id,
                item_types.as_deref(),
                unplayed_only,
                0,
                limit,
                &sort_by,
                &sort_order,
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

    fn maybe_fetch_next_page(&mut self, lib_idx: usize) {
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

    pub(super) fn handle_lib_event(&mut self, ev: LibEvent) {
        match ev {
            LibEvent::Loaded {
                lib_idx,
                parent_id,
                level,
            } => {
                let is_album = self.is_album_level(lib_idx);
                let is_grouped_albums = self.is_viewing_album_folders(lib_idx);
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    if let Some(last) = lib.nav_stack.last_mut() {
                        if last.parent_id == parent_id && last.loading {
                            *last = level;
                        }
                    }
                    if is_album {
                        let title = lib
                            .nav_stack
                            .last()
                            .map(|l| l.title.clone())
                            .unwrap_or_default();
                        log::debug!(target: "app", "album: entered «{title}»");
                        if let Some(last) = lib.nav_stack.last_mut() {
                            sort_audio_tracks(&mut last.items);
                        }
                    }
                    // The grouped-by-artist album views (music.rs/list.rs) display
                    // albums sorted by artist, not in the raw SortName-by-album-title
                    // order the API returns them in — so the freshly-loaded default
                    // cursor (index 0 in raw order) can land on an arbitrary album
                    // instead of the first one the user actually sees on screen. Snap
                    // it to the first album in (a synchronous best-effort guess at)
                    // display order. Mirrors `App::resolve_group_album_artist`'s
                    // fallback chain via `initial_group_artist_sort_key`.
                    if is_grouped_albums {
                        if let Some(last) = lib.nav_stack.last_mut() {
                            if !last.items.is_empty() {
                                let mut order: Vec<usize> = (0..last.items.len()).collect();
                                order.sort_by_key(|&i| {
                                    super::render::power::initial_group_artist_sort_key(
                                        &last.items[i],
                                    )
                                });
                                last.cursor = order[0];
                            }
                        }
                    }
                    if let Some(last) = lib.nav_stack.last_mut() {
                        if last
                            .items
                            .first()
                            .map(|i| i.item_type == "Episode")
                            .unwrap_or(false)
                        {
                            sort_episodes(&mut last.items);
                        }
                    }
                }
                // In the power view: when a season list arrives for a TV library,
                // automatically push a loading placeholder and fetch the first season's
                // episodes so the user lands directly in the combined series view.
                let should_auto_push = self.playlist_view == PLAYLIST_VIEW_POWER
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

                // In the power view: when the group list loads for a music library
                // with levels = ["group", …], automatically push the first group's
                // album level so the user lands directly in the combined group view.
                let should_auto_push_music = self.playlist_view == PLAYLIST_VIEW_POWER
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

                let should_aggregate_feed = self.should_aggregate_feed(lib_idx, |root| {
                    root.item_types.is_none() && !root.unplayed_only
                });
                if should_aggregate_feed {
                    self.log_feed_home_video_state(lib_idx, "loaded_before_aggregate");
                    self.spawn_feed_home_video_aggregate(lib_idx);
                    self.spawn_podcast_aggregate(lib_idx);
                }

                self.maybe_fetch_next_page(lib_idx);
                self.spawn_all_items_prefetch(lib_idx);
            }
            LibEvent::PageAppended {
                lib_idx,
                parent_id,
                items,
                total_count,
            } => {
                let is_album = self.is_album_level(lib_idx);
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    if let Some(last) = lib.nav_stack.last_mut() {
                        if last.parent_id == parent_id && last.loading {
                            last.items.extend(items);
                            last.total_count = total_count;
                            last.loading = false;
                        }
                    }
                    if is_album {
                        if let Some(last) = lib.nav_stack.last_mut() {
                            sort_audio_tracks(&mut last.items);
                        }
                    }
                    if let Some(last) = lib.nav_stack.last_mut() {
                        if last
                            .items
                            .first()
                            .map(|i| i.item_type == "Episode")
                            .unwrap_or(false)
                        {
                            sort_episodes(&mut last.items);
                        }
                    }
                }
                let should_aggregate_feed =
                    self.should_aggregate_feed(lib_idx, |root| root.parent_id == parent_id);
                if should_aggregate_feed {
                    self.log_feed_home_video_state(lib_idx, "page_appended_before_aggregate");
                    self.spawn_feed_home_video_aggregate(lib_idx);
                    self.spawn_podcast_aggregate(lib_idx);
                }
                self.maybe_fetch_next_page(lib_idx);
            }
            LibEvent::Refreshed {
                lib_idx,
                parent_id,
                item_types,
                unplayed_only,
                items,
                total_count,
            } => {
                let is_album = self.is_album_level(lib_idx);
                let is_feed_video_refresh = self.is_feed_home_video_library(lib_idx)
                    && item_types.as_deref() == Some("Video")
                    && unplayed_only;
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    if let Some(last) = lib.nav_stack.last_mut() {
                        if last.parent_id == parent_id && !is_feed_video_refresh {
                            last.items = items;
                            last.total_count = total_count;
                            last.loading = false;
                        }
                    }
                    if is_album {
                        if let Some(last) = lib.nav_stack.last_mut() {
                            sort_audio_tracks(&mut last.items);
                        }
                    }
                    if let Some(last) = lib.nav_stack.last_mut() {
                        if last
                            .items
                            .first()
                            .map(|i| i.item_type == "Episode")
                            .unwrap_or(false)
                        {
                            sort_episodes(&mut last.items);
                        }
                    }
                }
                let should_refresh_feed_groups = self
                    .libs
                    .get(lib_idx)
                    .map(|lib| {
                        self.playlist_view == PLAYLIST_VIEW_POWER
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
                    if let Some(state) = self
                        .libs
                        .get_mut(lib_idx)
                        .and_then(|lib| lib.feed_home_video.as_mut())
                    {
                        state.loading = true;
                    }
                    self.log_feed_home_video_state(lib_idx, "refreshed_before_aggregate");
                    self.spawn_feed_home_video_aggregate(lib_idx);
                    self.spawn_podcast_aggregate(lib_idx);
                }
                self.spawn_all_items_prefetch(lib_idx);
            }
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
                    self.home_search = None;
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
                // Merge by ID into whatever queue is *currently* live, rather than
                // overwriting wholesale: this fetch was kicked off by
                // restore_queue_state and may resolve long after the user has since
                // consumed items or replaced the queue entirely. An ID that's no
                // longer present (or belongs to an unrelated newer queue) is simply
                // skipped, so a late result can never resurrect or clobber anything.
                // The currently-playing item is also skipped, so a stale server-side
                // position can't stomp live in-session progress.
                let active_id = self
                    .player_tab
                    .items
                    .get(self.player_tab.playlist_cursor)
                    .map(|i| i.id.clone());
                let fresh_by_id: std::collections::HashMap<&str, &MediaItem> =
                    items.iter().map(|i| (i.id.as_str(), i)).collect();
                for item in self.player_tab.items.iter_mut() {
                    if active_id.as_deref() == Some(item.id.as_str()) {
                        continue;
                    }
                    if let Some(&fresh) = fresh_by_id.get(item.id.as_str()) {
                        *item = fresh.clone();
                    }
                }
            }
            LibEvent::Error(e) => {
                self.flash_status_high(format!("Error: {e}"));
            }
        }
    }

    pub(super) fn try_quit(&mut self) -> bool {
        if self.queue_dirty && self.queue_is_saved_playlist() {
            self.replace_queue_or_prompt(PendingQueueAction::Quit);
            false
        } else {
            self.save_prefs();
            if !self.player.is_remote() {
                self.player.stop();
            }
            true
        }
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
                if self.queue_scope_has_local_metadata(self.playback_queue_scope()) {
                    self.queue_source = source;
                }
                self.replace_playback_queue(items.clone(), start_idx);
                self.set_queue_scope(self.playback_queue_scope());
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
                    self.player
                        .play_playlist(items, start_idx, c, self.ui_volume);
                    self.player
                        .send_command(PlayerCommand::SetMute(self.mute_on));
                }
                if !direct_remote {
                    self.save_queue_state();
                }
            }
            PendingQueueAction::ClearQueue => {
                let scope = self.displayed_queue_scope();
                if self.queue_scope_has_local_metadata(scope) {
                    self.clear_local_queue_metadata();
                } else {
                    self.remote_playlist_undo_stack.clear();
                }
                if scope == QueueScope::Remote {
                    self.replace_direct_remote_queue(Vec::new(), 0);
                } else if self.queue_scope_is_playback(scope) {
                    self.player.stop();
                }
                if scope != QueueScope::Remote {
                    let queue = self.queue_for_scope_mut(scope);
                    queue.items.clear();
                    queue.playlist_cursor = 0;
                }
                self.persist_local_queue_state_if_needed(scope);
                self.flash_status("Queue cleared".into());
            }
            PendingQueueAction::Quit => {
                if !self.player.is_remote() {
                    self.player.stop();
                }
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
    /// Both checks are gated on `queue_scope_has_local_metadata`: `save_playlist_to_emby`
    /// always pushes `player_tab.items` (the *local* queue), so if the consume actually
    /// happened on a direct-remote/daemon queue, autosaving here would push an unrelated,
    /// unmodified local playlist to Emby instead of the queue that actually changed.
    pub(super) fn on_video_consumed(&mut self) {
        let scope = self.playback_queue_scope();
        log::info!(target: "consume", "on_video_consumed: scope={scope:?} has_local_metadata={}",
            self.queue_scope_has_local_metadata(scope));
        if !self.queue_scope_has_local_metadata(scope) {
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

    /// Number of selectable left-panel tabs in power view: Home/CW + all libraries.
    pub(super) fn power_left_tab_count(&self) -> usize {
        1 + self.libs.len()
    }

    /// Advance the left-panel tab (wrapping); load the library if needed.
    pub(super) fn power_left_tab_next(&mut self) {
        let n = self.power_left_tab_count();
        self.power_left_tab = (self.power_left_tab + 1) % n;
        self.last_card_height = 0; // reset stale image height for new view
        if self.power_left_tab > 0 {
            self.ensure_lib_loaded_for(self.power_left_tab - 1);
        }
        self.save_prefs();
    }

    /// Retreat the left-panel tab (wrapping); load the library if needed.
    pub(super) fn power_left_tab_prev(&mut self) {
        let n = self.power_left_tab_count();
        self.power_left_tab = (self.power_left_tab + n - 1) % n;
        self.last_card_height = 0;
        if self.power_left_tab > 0 {
            self.ensure_lib_loaded_for(self.power_left_tab - 1);
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

    /// Total number of items across all power-home groups (CW + all latest sections).
    fn power_home_total(&self) -> usize {
        self.home.continue_items.len()
            + self
                .home
                .latest
                .iter()
                .map(|(_, _, items, _)| items.len())
                .sum::<usize>()
    }

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

    /// Move the flat power-home cursor by `delta`, clamped to bounds.
    pub(super) fn power_home_move_cursor(&mut self, delta: i64) {
        let total = self.power_home_total();
        if total == 0 {
            return;
        }
        let cur = self.home.power_home_cursor.min(total - 1) as i64;
        self.home.power_home_cursor = (cur + delta).clamp(0, total as i64 - 1) as usize;
    }

    /// Section (index into `layout.power.home.layout`) currently holding the flat cursor.
    fn power_home_cur_section(&self) -> Option<usize> {
        let cursor = self.home.power_home_cursor;
        self.layout
            .power
            .home
            .layout
            .iter()
            .position(|m| m.len > 0 && cursor >= m.flat_start && cursor < m.flat_start + m.len)
    }

    /// Select the first item of the first non-empty section.
    fn power_home_select_first(&mut self) {
        if let Some(m) = self.layout.power.home.layout.iter().find(|x| x.len > 0) {
            self.home.power_home_cursor = m.flat_start;
        }
    }

    /// Grid-aware down: within the current card, else the top of the next non-empty
    /// card in the same column.
    pub(super) fn power_home_move_down(&mut self) {
        if self.layout.power.home.layout.is_empty() {
            self.power_home_move_cursor(1);
            return;
        }
        let Some(si) = self.power_home_cur_section() else {
            self.power_home_select_first();
            return;
        };
        let m = self.layout.power.home.layout[si].clone();
        let within = self.home.power_home_cursor - m.flat_start;
        if within + 1 < m.len {
            self.home.power_home_cursor += 1;
            return;
        }
        if let Some(next) = self
            .layout
            .power
            .home
            .layout
            .iter()
            .filter(|x| x.col == m.col && x.row > m.row && x.len > 0)
            .min_by_key(|x| x.row)
        {
            self.home.power_home_cursor = next.flat_start;
        }
    }

    /// Grid-aware up: within the current card, else the bottom of the previous
    /// non-empty card in the same column.
    pub(super) fn power_home_move_up(&mut self) {
        if self.layout.power.home.layout.is_empty() {
            self.power_home_move_cursor(-1);
            return;
        }
        let Some(si) = self.power_home_cur_section() else {
            self.power_home_select_first();
            return;
        };
        let m = self.layout.power.home.layout[si].clone();
        let within = self.home.power_home_cursor - m.flat_start;
        if within > 0 {
            self.home.power_home_cursor -= 1;
            return;
        }
        if let Some(prev) = self
            .layout
            .power
            .home
            .layout
            .iter()
            .filter(|x| x.col == m.col && x.row < m.row && x.len > 0)
            .max_by_key(|x| x.row)
        {
            self.home.power_home_cursor = prev.flat_start + prev.len - 1;
        }
    }

    /// Cycle the flat cursor to the first item of the previous/next non-empty
    /// section, wrapping at the ends. `dir` = -1 previous, +1 next.
    pub(super) fn power_home_move_section(&mut self, dir: i64) {
        let sections: Vec<usize> = self
            .layout
            .power
            .home
            .layout
            .iter()
            .enumerate()
            .filter(|(_, m)| m.len > 0)
            .map(|(i, _)| i)
            .collect();
        if sections.is_empty() {
            return;
        }
        let pos = self
            .power_home_cur_section()
            .and_then(|si| sections.iter().position(|&s| s == si));
        let next_pos = match pos {
            Some(p) => {
                let n = sections.len() as i64;
                (((p as i64 + dir) % n + n) % n) as usize
            }
            None => 0,
        };
        let si = sections[next_pos];
        self.home.power_home_cursor = self.layout.power.home.layout[si].flat_start;
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

    pub(super) fn save_queue_state(&self) {
        self.save_queue_state_impl(true);
    }

    /// Like `save_queue_state`, but never deletes the on-disk snapshot when the
    /// in-memory queue happens to be empty. Quit is not a genuine "user cleared
    /// the queue" signal — an empty `player_tab.items` at quit time can equally
    /// mean this session never touched the local queue at all, and unconditionally
    /// deleting in that case wipes a perfectly valid snapshot from an earlier
    /// session with no recovery path. Only an explicit `ClearQueue` action (which
    /// goes through `save_queue_state`) should ever delete the file.
    pub(super) fn save_queue_state_no_clear(&self) {
        self.save_queue_state_impl(false);
    }

    fn save_queue_state_impl(&self, allow_clear: bool) {
        let positions: std::collections::HashMap<String, i64> = self
            .player_tab
            .items
            .iter()
            .filter(|i| i.playback_position_ticks > 0 && !i.is_audio())
            .map(|i| (i.id.clone(), i.playback_position_ticks))
            .collect();
        let state = crate::config::QueueState {
            source: self.queue_source.clone(),
            items: self.player_tab.items.clone(),
            cursor: self.player_tab.playlist_cursor,
            last_played_item_id: self.last_played_item_id.clone(),
            last_played_completed: self.last_played_completed,
            positions,
        };
        if state.items.is_empty() {
            // Don't nuke the on-disk queue just because the local tab happens to be
            // empty while attached to a remote session — that reflects remote-control
            // UI state, not the user intentionally clearing their local queue.
            if allow_clear && self.connected_session_id.is_none() {
                crate::config::clear_queue_state();
            }
        } else {
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
        self.player_tab.playlist_cursor = cursor;
        self.player_tab.items = state.items;
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
    fn spawn_enrich_queue_state(&self, positions: std::collections::HashMap<String, i64>) {
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
                            saved_pos / crate::api::TICKS_PER_SECOND,
                            item.playback_position_ticks / crate::api::TICKS_PER_SECOND,
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
        // Drain existing libs, preserving nav stacks, the pinned detail item, and scroll pos
        // so that a UserDataChanged websocket refresh (fired when playback starts) doesn't
        // silently dismiss the movie detail panel.
        struct SavedLibState {
            nav_stack: Vec<BrowseLevel>,
            feed_home_video: Option<FeedHomeVideoState>,
            detail_item: Option<crate::api::MediaItem>,
            detail_scroll: usize,
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
                        detail_item: l.power_detail_item,
                        detail_scroll: l.power_detail_scroll,
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
                        })
                        .collect()
                })
                .unwrap_or_default();
            let feed_home_video = saved.and_then(|s| s.feed_home_video.clone());
            let detail_item = saved.and_then(|s| s.detail_item.clone());
            let detail_scroll = saved.map(|s| s.detail_scroll).unwrap_or(0);
            self.libs.push(super::LibraryTab {
                library: view.clone(),
                nav_stack: stack,
                search: None,
                feed_home_video,
                power_detail_item: detail_item,
                power_detail_scroll: detail_scroll,
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
            let title = format!("New {}", v.name);
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
                if items.len() == 1 {
                    let mut item = items[0].clone();
                    if start_position_ticks > 0 {
                        item.playback_position_ticks = start_position_ticks;
                    }
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
                    log::info!(target: "ws", "Play multi: count={count}, start_idx={start_idx}");
                    // Always hand the whole list to play_playlist (not just the clicked
                    // item) so the remote-controlled queue continues past start_idx.
                    // play_playlist already handles the "something is already playing"
                    // case in place via ReplacePlaylist.
                    let mut items_with_pos = items.clone();
                    if start_position_ticks > 0 {
                        items_with_pos[start_idx].playback_position_ticks = start_position_ticks;
                    }
                    self.player
                        .play_playlist(items_with_pos, start_idx, c, self.ui_volume);
                }
                self.queue_source = crate::config::QueueSource::Remote;
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
        let visible = self.terminal_height.saturating_sub(4) as usize;
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
                    matcher
                        .fuzzy_match(&item.display_name(), &query)
                        .map(|s| (s, i))
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

#[cfg(test)]
mod tests {
    use super::*;

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

    // `XDG_CONFIG_HOME`/`XDG_STATE_HOME` are process-global env vars, so
    // tests that touch them must not run concurrently with each other.
    // Mirrors action.rs's `XDG_STATE_HOME_LOCK` / config.rs's `SYS_ENV_LOCK`
    // convention (each module guards only its own tests; see those for the
    // same caveat).
    static XDG_HOME_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// RAII guard that points both `XDG_CONFIG_HOME` (subtitle-mode saves)
    /// and `XDG_STATE_HOME` (prefs saves) at a fresh tempdir, restoring and
    /// cleaning up on drop -- including on panic.
    struct XdgHomeGuard {
        dir: std::path::PathBuf,
    }

    impl XdgHomeGuard {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("mbv-test-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            std::env::set_var("XDG_CONFIG_HOME", &dir);
            std::env::set_var("XDG_STATE_HOME", &dir);
            std::env::remove_var("MBV_SYSTEM");
            Self { dir }
        }
    }

    impl Drop for XdgHomeGuard {
        fn drop(&mut self) {
            std::env::remove_var("XDG_CONFIG_HOME");
            std::env::remove_var("XDG_STATE_HOME");
            let _ = std::fs::remove_dir_all(&self.dir);
        }
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
        assert_eq!(app.player_tab.playlist_cursor, 1);
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

    fn make_remote_session(audio_only: bool) -> crate::api::SessionInfo {
        crate::api::SessionInfo {
            media_info: crate::api::SessionMediaInfo {
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
        app.player_tab.playlist_cursor = 0;

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
}
