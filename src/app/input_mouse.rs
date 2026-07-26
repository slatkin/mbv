#![allow(unused_imports)]

use crate::app::action::Command;
use crate::app::layout::LibraryRowTarget;
use crate::app::{
    App, PanelFocus, PendingQueueAction, QueueScope, HELP_PANEL_W, PLAYLISTS_PANEL_W,
    SESSIONS_PANEL_W, SETTINGS_PANEL_W,
};
use mbv_core::api::{MediaItem, TICKS_PER_SECOND};
use mbv_core::player::PlayerCommand;
use ratatui::layout::Rect;
use std::time::{Duration, Instant};
impl App {
    /// Map a column click to a left-panel tab index (0 = Home, 1+ = library),
    /// scroll-aware: returns `usize::MAX - 1` for a click on the `«` arrow
    /// and `usize::MAX` for a click on the `»` arrow (see `handle_mouse`).
    pub(super) fn power_tab_idx_at(&self, col: u16) -> Option<usize> {
        let area = self.layout.tabs_area;
        if col < area.x || col >= area.x + area.width {
            return None;
        }
        let rel = col - area.x;
        let (vis_start, vis_end) = self.visible_tab_range(area.width);
        let has_left = vis_start > 0;
        let has_right = vis_end < self.tab_count();
        let left_w: u16 = if has_left { 2 } else { 0 };
        let right_w: u16 = if has_right { 2 } else { 0 };
        if has_left && rel < left_w {
            return Some(usize::MAX - 1);
        }
        if has_right && rel >= area.width - right_w {
            return Some(usize::MAX);
        }
        let rel = rel - left_w;
        let widths = self.tab_title_widths();
        let pad = 1u16;
        let mut x = 0u16;
        for (i, &w) in widths
            .iter()
            .enumerate()
            .skip(vis_start)
            .take(vis_end - vis_start)
        {
            let end = x + pad + w + pad;
            if rel < end {
                return Some(i);
            }
            x = end;
        }
        None
    }

    pub(super) fn seek_to_col(&mut self, col: u16) {
        let bar = self.layout.playback.seekbar_area;
        if bar.width == 0 {
            return;
        }
        let fraction = (col.saturating_sub(bar.x)) as f64 / bar.width as f64;
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            let runtime_s = self
                .connected_session_state
                .as_ref()
                .map(|s| s.runtime_s)
                .unwrap_or(0);
            if runtime_s == 0 {
                return;
            }
            let ticks = (fraction * (runtime_s * mbv_core::api::TICKS_PER_SECOND) as f64) as i64;
            let id = conn_id.clone();
            self.remote_pos_s = (fraction * runtime_s as f64) as i64;
            self.remote_pos_at = Instant::now();
            self.remote_seek_pending_until = Instant::now() + Duration::from_secs(4);
            self.do_session_command(move |c| c.session_seek(&id, ticks));
            return;
        }
        let runtime_ticks = self.player.status.lock().unwrap().runtime_ticks;
        if runtime_ticks == 0 {
            return;
        }
        let target_secs = (fraction * runtime_ticks as f64) / TICKS_PER_SECOND as f64;
        self.player
            .send_command(PlayerCommand::SeekAbsolute(target_secs));
    }

    pub(super) fn click_set_cursor(&mut self, col: u16, row: u16) -> bool {
        {
            if self.has_direct_remote_queue() {
                if self
                    .layout
                    .main
                    .queue_scope_local_area
                    .contains((col, row).into())
                {
                    self.set_queue_scope(QueueScope::Local);
                    return true;
                }
                if self
                    .layout
                    .main
                    .queue_scope_remote_area
                    .contains((col, row).into())
                {
                    self.set_queue_scope(QueueScope::Remote);
                    return true;
                }
            }
            // Click in queue area: focus queue and move cursor.
            let qa = self.layout.main.queue_area;
            if qa.contains((col, row).into()) {
                if !matches!(self.panel_focus, PanelFocus::Queue) {
                    self.last_card_height = 0;
                }
                self.set_panel_focus(PanelFocus::Queue);
                let content_y = (row - qa.y) as usize;
                if let Some(&Some(item_idx)) = self.layout.main.queue_row_map.get(content_y) {
                    self.displayed_queue_mut().queue_cursor = item_idx;
                }
                return true;
            }
            // Click in the left panel: focus it and set its cursor.
            let la = self.layout.main.left_area;
            if la.contains((col, row).into()) {
                if !matches!(self.panel_focus, PanelFocus::Library) {
                    self.last_card_height = 0;
                }
                self.set_panel_focus(PanelFocus::Library);
                if self.library_tab == 0 {
                    // Home tab: rectangle hit-test the two-column card grid.
                    let pos = (col, row).into();
                    if let Some((_, flat_idx)) = self
                        .layout
                        .main
                        .home
                        .hitmap
                        .iter()
                        .find(|(rect, _)| rect.contains(pos))
                    {
                        self.home.home_cursor = *flat_idx;
                    }
                } else {
                    let lib_idx = self.library_tab - 1;
                    if self.is_music_group_view(lib_idx)
                        || self.is_feed_home_video_group_view(lib_idx)
                        || self.should_show_letter_pills(lib_idx)
                    {
                        for (rect, target) in self.layout.main.selector_tabs.clone() {
                            if rect.contains((col, row).into()) {
                                if self.is_music_group_view(lib_idx) {
                                    self.select_music_group(lib_idx, target);
                                } else if self.is_feed_home_video_group_view(lib_idx) {
                                    self.select_feed_folder_group(lib_idx, target);
                                } else {
                                    self.select_letter_pill(lib_idx, target);
                                }
                                return true;
                            }
                        }
                    }
                    let click_y = (row - la.y) as usize;
                    // Read the row map before taking a mutable borrow on libs (borrow checker).
                    let use_row_map = !self.layout.main.left_row_map.is_empty();
                    let row_map_item = if use_row_map {
                        self.layout.main.left_row_map.get(click_y).copied()
                    } else {
                        None
                    };
                    let row_target = self
                        .layout
                        .main
                        .left_row_targets
                        .get(click_y)
                        .cloned()
                        .flatten();
                    if self.is_music_group_view(lib_idx) {
                        match row_target {
                            Some(LibraryRowTarget::ArtistHeader(selection)) => {
                                self.libs[lib_idx].album_track_focus = None;
                                self.libs[lib_idx].artist_header_focus = Some(selection);
                                self.save_default_library_position(lib_idx);
                                return true;
                            }
                            Some(LibraryRowTarget::Album(item_idx)) => {
                                let lib = &mut self.libs[lib_idx];
                                if let Some(lvl) = lib.nav_stack.last_mut() {
                                    if item_idx < lvl.items.len() {
                                        if lvl.cursor != item_idx {
                                            lib.album_track_focus = None;
                                        }
                                        lib.artist_header_focus = None;
                                        lvl.cursor = item_idx;
                                        self.save_default_library_position(lib_idx);
                                        return true;
                                    }
                                }
                            }
                            None => {}
                        }
                    }
                    let is_feed_group = self.is_feed_home_video_group_view(lib_idx);
                    let lib = &mut self.libs[lib_idx];
                    if let Some(s) = &mut lib.search {
                        if use_row_map {
                            // Letter-grouped or banner-adjacent mode: row map gives the
                            // result index directly (None = header/banner-filler row).
                            if let Some(Some(item_idx)) = row_map_item {
                                if item_idx < s.results.len() {
                                    s.cursor = item_idx;
                                }
                            }
                        } else {
                            let visible = la.height as usize;
                            let offset = if s.cursor >= visible {
                                s.cursor - visible + 1
                            } else {
                                0
                            };
                            let clicked = offset + click_y;
                            if clicked < s.results.len() {
                                s.cursor = clicked;
                            }
                        }
                    } else if is_feed_group {
                        let visible = la.height as usize;
                        if let Some(state) = lib.feed_home_video.as_mut() {
                            let items_len = state.selected_len();
                            if use_row_map {
                                if let Some(Some(item_idx)) = row_map_item {
                                    if item_idx < items_len {
                                        state.video_cursor = item_idx;
                                    }
                                }
                            } else {
                                let offset = if state.video_cursor >= visible {
                                    state.video_cursor - visible + 1
                                } else {
                                    0
                                };
                                let clicked = offset + click_y;
                                if clicked < items_len {
                                    state.video_cursor = clicked;
                                }
                            }
                        }
                        self.save_default_library_position(lib_idx);
                    } else if let Some(lvl) = lib.nav_stack.last_mut() {
                        if use_row_map {
                            // Letter-grouped mode: row map gives item index (None = header row).
                            if let Some(Some(item_idx)) = row_map_item {
                                if item_idx < lvl.items.len() {
                                    if lvl.cursor != item_idx {
                                        lib.album_track_focus = None;
                                    }
                                    lib.artist_header_focus = None;
                                    lvl.cursor = item_idx;
                                }
                            }
                        } else {
                            let visible = la.height as usize;
                            let offset = if lvl.cursor >= visible {
                                lvl.cursor - visible + 1
                            } else {
                                0
                            };
                            let clicked = offset + click_y;
                            if clicked < lvl.items.len() {
                                if lvl.cursor != clicked {
                                    lib.album_track_focus = None;
                                }
                                lib.artist_header_focus = None;
                                lvl.cursor = clicked;
                            }
                        }
                        self.save_default_library_position(lib_idx);
                    }
                }
                return true;
            }
        }
        false
    }
}
