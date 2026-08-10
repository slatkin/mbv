#![allow(unused_imports)]

use crate::app::action::Command;
use crate::app::layout::LibraryRowTarget;
use crate::app::{
    App, PanelFocus, PendingQueueAction, QueueScope, HELP_PANEL_W, PLAYLISTS_PANEL_W,
    SESSIONS_PANEL_W, SETTINGS_PANEL_W,
};
use mbv_core::api::{EmbyItem, TICKS_PER_SECOND};
use mbv_core::player::PlayerCommand;
use ratatui::layout::Rect;
use std::time::{Duration, Instant};
impl App {
    pub(super) fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent) {
        use crossterm::event::{MouseButton, MouseEventKind};
        let col = mouse.column;
        let row = mouse.row;
        // Always track mouse position so hover rendering is up to date.
        self.mouse_col = col;
        self.mouse_row = row;

        if self.remote_reanchor_popup.is_some() {
            return;
        }

        // Swallow the single click that merely refocused the window (e.g.
        // alt-tab back in by clicking): if a FocusGained landed within the
        // last 150ms, this Down(Left)/Down(Right) is that click, not a UI
        // action. `.take()` makes this strictly one-shot -- it clears
        // `refocus_at` whether or not the click was in the window, so a
        // second click right after a suppressed one dispatches normally.
        if matches!(
            mouse.kind,
            MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Down(MouseButton::Right)
        ) {
            if let Some(t) = self.refocus_at.take() {
                if t.elapsed() < Duration::from_millis(150) {
                    log::debug!(target: "input", "suppressed refocus click at ({col}, {row})");
                    return;
                }
            }
        }

        if matches!(
            mouse.kind,
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
        ) {
            let now = Instant::now();
            if now.duration_since(self.last_scroll_at) < Duration::from_millis(30) {
                return;
            }
            self.last_scroll_at = now;
        }

        if self.handle_mouse_panels(mouse) {
            return;
        }

        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
            && self.layout.tabs_area.contains((col, row).into())
        {
            // Tab clicks change the left-panel selection.
            if let Some(idx) = self.tab_idx_at(col) {
                if idx == usize::MAX - 1 {
                    self.tab_scroll = self.tab_scroll.saturating_sub(1);
                } else if idx == usize::MAX {
                    let max_scroll = self.tab_count().saturating_sub(1);
                    self.tab_scroll = (self.tab_scroll + 1).min(max_scroll);
                } else {
                    self.set_library_tab(idx);
                }
            }
            return;
        }

        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
            && self.layout.settings_area.contains((col, row).into())
        {
            self.show_settings = !self.show_settings;
            return;
        }

        match mouse.kind {
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                let delta: i64 = if matches!(mouse.kind, MouseEventKind::ScrollDown) {
                    1
                } else {
                    -1
                };
                if self.layout.tabbar_vol_area.contains((col, row).into()) {
                    // Same `Command` the `-`/`+` keys dispatch (issue #134);
                    // only the hit-test and the wheel-to-delta mapping are
                    // mouse-specific.
                    self.dispatch(Command::AdjustVolume(-delta * 5));
                    return;
                }
                // Scroll in whichever panel the mouse is over.
                let queue_area = self.layout.main.queue_area;
                let left_area = self.layout.main.left_area;
                if queue_area.contains((col, row).into()) {
                    let n = self.displayed_queue().total_queue_len();
                    if n > 0 {
                        let delta = delta * 3;
                        let queue = self.displayed_queue_mut();
                        queue.queue_cursor =
                            (queue.queue_cursor as i64 + delta).clamp(0, n as i64 - 1) as usize;
                    }
                } else if left_area.contains((col, row).into()) {
                    if self.tab.is_home() {
                        self.cw_move_cursor(delta);
                    } else if self.tab.is_feeds() {
                        self.feed_tab_move_cursor(delta);
                    } else {
                        self.move_lib_cursor(delta);
                    }
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if self.context_menu.is_some() {
                    if let Some(rect) = self.layout.context_menu_rect {
                        if rect.contains((col, row).into()) {
                            let inner_y = rect.y + 1;
                            if row >= inner_y
                                && (row - inner_y)
                                    < self.context_menu.as_ref().unwrap().entries.len() as u16
                            {
                                let idx = (row - inner_y) as usize;
                                let action = self
                                    .context_menu
                                    .as_ref()
                                    .unwrap()
                                    .entries
                                    .get(idx)
                                    .and_then(|entry| entry.action.clone());
                                if action.is_some() {
                                    self.context_menu = None;
                                    self.layout.context_menu_rect = None;
                                    self.force_clear = true;
                                    self.execute_context_action(action);
                                }
                            } else {
                                self.context_menu = None;
                                self.force_clear = true;
                            }
                            return;
                        }
                    }
                    self.context_menu = None;
                    self.force_clear = true;
                    return;
                }

                let now = Instant::now();

                let is_double = now.duration_since(self.last_click_time)
                    < Duration::from_millis(400)
                    && self.last_click_pos == (col, row);
                self.last_click_time = now;
                self.last_click_pos = (col, row);

                {
                    for (rect, target) in self.layout.main.selector_tabs.clone() {
                        if rect.contains((col, row).into()) {
                            if self.tab.is_home() {
                                self.home_select_section(target);
                            } else if self.tab.is_feeds() {
                                self.feed_tab_select_group(target);
                            } else {
                                let lib_idx = self.tab.library_index().unwrap();
                                if self.is_music_group_view(lib_idx) {
                                    self.select_music_group(lib_idx, target);
                                } else if self.is_feed_home_video_group_view(lib_idx) {
                                    self.select_feed_folder_group(lib_idx, target);
                                } else if self.should_show_letter_pills(lib_idx) {
                                    self.select_letter_pill(lib_idx, target);
                                }
                            }
                            return;
                        }
                    }
                }

                if is_double {
                    if self
                        .layout
                        .playback
                        .seekbar_area
                        .contains((col, row).into())
                    {
                        self.seek_to_col(col);
                        return;
                    }
                    if matches!(self.panel_focus, PanelFocus::Queue) {
                        let queue = self.displayed_queue();
                        let t = queue.queue_cursor;
                        // Spatial hit-test stays local (issue #134); the
                        // activation itself is the same `Command` the queue
                        // tab's `Enter` key dispatches, so double-click and
                        // `Enter` can't drift again the way they did before
                        // a70ad7a.
                        if t < queue.total_queue_len()
                            && self.layout.main.queue_area.contains((col, row).into())
                        {
                            self.dispatch(Command::QueuePlayCursor);
                        }
                    } else if self.tab.is_home() {
                        self.home_play();
                    } else if self.tab.is_feeds() {
                        // Double-click on Feeds: no-op (playback wiring pending).
                    } else if self.layout.main.left_area.contains((col, row).into())
                        || self.layout.main.hero_area.contains((col, row).into())
                    {
                        // Double-click activates the row under the cursor
                        // (the first click of the pair already focused it).
                        // Mirrors the Enter key's activation for the same
                        // row so the two gestures can't drift: recursive
                        // album search jump, album-folder track mode,
                        // series selection, then `select()` (plays media
                        // items and drills into folders). The inline hero is
                        // just another surface over the selected item, so a
                        // double-click there activates it the same way --
                        // including entering a Series' season/episode
                        // selection, which a single click never did.
                        let lib_idx = self.tab.library_index().unwrap();
                        // Wide Music: double-click on a track plays it (Task 5.2).
                        let is_wide_music = !self.layout.main.wide_music_track_hitmap.is_empty();
                        if is_wide_music {
                            let pos = (col, row).into();
                            for (rect, track_idx) in &self.layout.main.wide_music_track_hitmap {
                                if rect.contains(pos) {
                                    self.libs[lib_idx].album_track_focus = Some(*track_idx);
                                    // Play the focused track.
                                    self.select();
                                    return;
                                }
                            }
                            // Double-click on artwork or blank space: no-op.
                            return;
                        }
                        if self.activate_recursive_album(lib_idx) {
                            // active-search jump; unchanged
                        } else if self.is_viewing_album_folders(lib_idx) {
                            self.activate_album_folder_row(lib_idx);
                        } else if self.libs[lib_idx].series_selection.is_some() {
                            // Play the focused episode in selection mode.
                            if let Some(episodes) = self.series_selection_episodes(lib_idx) {
                                let ep_idx = self.libs[lib_idx].series_selection.unwrap_or(0);
                                if let Some(ep) = episodes.get(ep_idx) {
                                    let ep = ep.clone();
                                    self.libs[lib_idx].series_selection = None;
                                    self.play_item(ep);
                                }
                            }
                        } else if let Some(item) = self.selected_series_item(lib_idx) {
                            self.enter_series_selection(lib_idx, &item);
                        } else {
                            self.select();
                        }
                    }
                    // Wide Music: double-click on right pane album enters
                    // track selection (same as Enter).
                    if !self.layout.main.wide_music_track_hitmap.is_empty()
                        && self
                            .layout
                            .main
                            .wide_music_right_area
                            .contains((col, row).into())
                    {
                        let lib_idx = self.tab.library_index().unwrap();
                        self.activate_album_folder_row(lib_idx);
                    }
                    return;
                }

                if self.layout.playback.ind_rc.contains((col, row).into()) {
                    self.show_sessions = !self.show_sessions;
                    if self.show_sessions {
                        self.spawn_sessions_load();
                    }
                    return;
                }
                if self.layout.playback.ind_mu.contains((col, row).into()) {
                    // The "m" pill renders `self.mute_on` (see
                    // render_control_pill) and the `m` key flips it via
                    // `Command::ToggleMute` -- dispatch the same action here
                    // rather than calling `toggle_mute()` (the *other*,
                    // ui_volume-based mechanism used by the `a` key; see
                    // `Command::ToggleMute`'s doc comment in action.rs).
                    // Calling the wrong one here predates #88, but #88 makes
                    // it worse: `toggle_mute()` now falls back to
                    // `cycle_audio()` for a connected remote session, so
                    // clicking this pill while attached to a session used to
                    // be a harmless no-op and would otherwise start silently
                    // cycling that session's audio track instead of muting
                    // anything.
                    self.dispatch(Command::ToggleMute);
                    return;
                }
                if self
                    .layout
                    .playback
                    .play_pause_area
                    .contains((col, row).into())
                {
                    self.dispatch(Command::TogglePlayPause);
                    return;
                }
                if self.layout.playback.stop_area.contains((col, row).into()) {
                    let stop_avail = self.connected_session_id.is_some()
                        || self.player.status.lock().unwrap().active;
                    if stop_avail {
                        self.dispatch(Command::Stop);
                    }
                    return;
                }
                if self.layout.playback.next_area.contains((col, row).into()) {
                    if self.transport_prev_next_available().1 {
                        self.dispatch(Command::NextTrack);
                    }
                    return;
                }
                if self
                    .layout
                    .playback
                    .idle_feed_link_area
                    .contains((col, row).into())
                {
                    self.dispatch(Command::OpenIdleFeedLink);
                    return;
                }
                // Header breadcrumb clicks.
                if self.tab.library_index().is_some() {
                    let crumbs = self.layout.main.breadcrumbs.clone();
                    let lib_idx = self.tab.library_index().unwrap();
                    for (x_start, x_end, crumb_row, target_depth) in crumbs {
                        if row == crumb_row && col >= x_start && col < x_end {
                            self.libs[lib_idx].nav_stack.truncate(target_depth);
                            self.save_default_library_position(lib_idx);
                            return;
                        }
                    }
                }

                // Single click only focuses the clicked row. Activation --
                // playing a media item, drilling into a folder, opening
                // track/series selection -- is a double-click (or Enter)
                // gesture and never happens here.
                self.click_set_cursor(col, row);
            }
            MouseEventKind::Down(MouseButton::Right) => {
                if self.click_set_cursor(col, row) {
                    self.open_context_menu_at(col, row);
                }
            }
            MouseEventKind::Drag(MouseButton::Left)
                if self
                    .layout
                    .playback
                    .seekbar_area
                    .contains((col, row).into())
                    && self.last_drag_seek.elapsed() >= Duration::from_millis(150) =>
            {
                self.last_drag_seek = Instant::now();
                self.seek_to_col(col);
            }
            MouseEventKind::Moved | MouseEventKind::Drag(MouseButton::Right) => {
                if let (Some(ref mut menu), Some(rect)) =
                    (&mut self.context_menu, self.layout.context_menu_rect)
                {
                    let inner_y = rect.y + 1;
                    if rect.contains((col, row).into()) && row >= inner_y {
                        let idx = (row - inner_y) as usize;
                        if idx < menu.entries.len() && menu.entries[idx].action.is_some() {
                            menu.cursor = idx;
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
