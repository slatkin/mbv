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
    fn power_tab_idx_at(&self, col: u16) -> Option<usize> {
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

    fn seek_to_col(&mut self, col: u16) {
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

    /// Handle a mouse event when a panel overlay (help/settings/sessions/playlists) is open.
    /// Returns true if the event was consumed.
    fn handle_mouse_panels(&mut self, mouse: crossterm::event::MouseEvent) -> bool {
        use crossterm::event::{MouseButton, MouseEventKind};
        let col = mouse.column;
        let row = mouse.row;
        let panel_w: u16 = if self.show_help {
            HELP_PANEL_W
        } else if self.show_settings {
            SETTINGS_PANEL_W
        } else if self.show_sessions {
            SESSIONS_PANEL_W
        } else if self.show_playlists {
            PLAYLISTS_PANEL_W
        } else {
            return false;
        };
        let power_panel = self.layout.main.panel_area.width > 0;
        let panel_area = if power_panel {
            self.layout.main.panel_area
        } else {
            Rect {
                x: 0,
                y: 0,
                width: panel_w.min(self.terminal_width),
                height: self.terminal_height,
            }
        };
        let content_area = self.layout.main.panel_content_area;
        let inside_panel = panel_area.contains((col, row).into());
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) && !inside_panel {
            if self.show_settings {
                self.close_settings();
            } else {
                self.show_help = false;
                self.show_sessions = false;
                self.show_playlists = false;
            }
            return true;
        }
        if self.show_help {
            match mouse.kind {
                MouseEventKind::ScrollDown => {
                    self.help_scroll += 3;
                }
                MouseEventKind::ScrollUp => {
                    self.help_scroll = self.help_scroll.saturating_sub(3);
                }
                _ => {}
            }
            return true;
        }
        if self.show_settings && self.multiselect_popup.is_none() {
            let settings_content_area = self.layout.settings_content_area;
            let content_top = settings_content_area.y;
            let content_bottom = settings_content_area
                .y
                .saturating_add(settings_content_area.height);
            match mouse.kind {
                MouseEventKind::ScrollDown => {
                    self.settings_scroll += 3;
                }
                MouseEventKind::ScrollUp => {
                    self.settings_scroll = self.settings_scroll.saturating_sub(3);
                }
                MouseEventKind::Down(MouseButton::Left)
                    if row >= content_top && row < content_bottom =>
                {
                    let lines_idx = (row - content_top) as usize + self.settings_scroll;
                    if let Some(cur) = self
                        .layout
                        .settings_line_of_cursor
                        .iter()
                        .position(|&l| l == lines_idx)
                    {
                        self.settings_cursor = cur;
                        self.settings_scroll_follow();
                        self.handle_settings_activate();
                    }
                }
                _ => {}
            }
            return true;
        }
        if self.show_sessions {
            const ENTRY_H: u16 = 4;
            let content_top = if power_panel { content_area.y } else { 1 };
            match mouse.kind {
                MouseEventKind::ScrollDown => {
                    if !self.sessions.is_empty() {
                        self.sessions_cursor =
                            (self.sessions_cursor + 1).min(self.sessions.len() - 1);
                    }
                }
                MouseEventKind::ScrollUp => {
                    self.sessions_cursor = self.sessions_cursor.saturating_sub(1);
                }
                MouseEventKind::Down(MouseButton::Left) if row >= content_top => {
                    let idx = ((row - content_top) / ENTRY_H) as usize;
                    if idx < self.sessions.len() {
                        if self.sessions_cursor == idx {
                            if let Some(sess) = self.sessions.get(idx) {
                                let sess = sess.clone();
                                self.connect_to_session(&sess);
                            }
                        } else {
                            self.sessions_cursor = idx;
                        }
                    }
                }
                _ => {}
            }
            return true;
        }
        if self.show_playlists {
            let content_top = if power_panel { content_area.y } else { 1 };
            if self.playlists_open.is_some() {
                match mouse.kind {
                    MouseEventKind::ScrollDown => {
                        if !self.playlists_open_items.is_empty() {
                            self.playlists_open_cursor = (self.playlists_open_cursor + 1)
                                .min(self.playlists_open_items.len() - 1);
                        }
                    }
                    MouseEventKind::ScrollUp => {
                        self.playlists_open_cursor = self.playlists_open_cursor.saturating_sub(1);
                    }
                    MouseEventKind::Down(MouseButton::Left) if row >= content_top => {
                        let click_line = (row - content_top) as usize;
                        let mut y = 0usize;
                        let mut idx = self.playlists_open_scroll;
                        for i in self.playlists_open_items[self.playlists_open_scroll..].iter() {
                            let text_width = if power_panel {
                                content_area.width as usize
                            } else {
                                PLAYLISTS_PANEL_W.min(self.terminal_width) as usize
                            };
                            let h = if i.display_name().len() <= text_width.saturating_sub(6) {
                                1
                            } else {
                                2
                            };
                            if click_line < y + h {
                                break;
                            }
                            y += h;
                            idx += 1;
                        }
                        if idx < self.playlists_open_items.len() {
                            if self.playlists_open_cursor == idx {
                                let selected_id =
                                    self.playlists_open_items.get(idx).map(|i| i.id.clone());
                                let pl_source = crate::config::QueueSource::Playlist {
                                    id: self.playlists_open.as_ref().map(|p| p.id.clone()),
                                    name: self
                                        .playlists_open
                                        .as_ref()
                                        .map(|p| p.name.clone())
                                        .unwrap_or_default(),
                                };
                                let items: Vec<MediaItem> = self
                                    .playlists_open_items
                                    .iter()
                                    .filter(|i| !i.is_folder)
                                    .cloned()
                                    .collect();
                                if !items.is_empty() {
                                    let start = selected_id
                                        .as_deref()
                                        .and_then(|id| items.iter().position(|i| i.id == id))
                                        .unwrap_or(0);
                                    let action = PendingQueueAction::PlayItems {
                                        items,
                                        start_idx: start,
                                        source: pl_source,
                                    };
                                    self.replace_queue_or_prompt(action);
                                    if self.confirm_modal.is_none() {
                                        self.show_playlists = false;
                                        self.set_panel_focus(PanelFocus::Queue);
                                    }
                                }
                            } else {
                                self.playlists_open_cursor = idx;
                            }
                        }
                    }
                    MouseEventKind::Down(MouseButton::Right) if row >= content_top => {
                        self.playlists_open = None;
                        self.playlists_open_items = Vec::new();
                    }
                    _ => {}
                }
            } else {
                match mouse.kind {
                    MouseEventKind::ScrollDown => {
                        if !self.playlists.is_empty() {
                            self.playlists_cursor =
                                (self.playlists_cursor + 1).min(self.playlists.len() - 1);
                        }
                    }
                    MouseEventKind::ScrollUp => {
                        self.playlists_cursor = self.playlists_cursor.saturating_sub(1);
                    }
                    MouseEventKind::Down(MouseButton::Left) if row >= content_top => {
                        let idx = (row - content_top) as usize + self.playlists_scroll;
                        if idx < self.playlists.len() {
                            if self.playlists_cursor == idx {
                                let id = self.playlists[idx].id.clone();
                                self.load_and_play_playlist(id);
                            } else {
                                self.playlists_cursor = idx;
                            }
                        }
                    }
                    _ => {}
                }
            }
            return true;
        }
        false
    }

    pub(super) fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent) {
        use crossterm::event::{MouseButton, MouseEventKind};
        let col = mouse.column;
        let row = mouse.row;
        // Always track mouse position so hover rendering is up to date.
        self.mouse_col = col;
        self.mouse_row = row;

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
            if let Some(idx) = self.power_tab_idx_at(col) {
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
                    let n = self.displayed_queue().items.len();
                    if n > 0 {
                        let delta = delta * 3;
                        let queue = self.displayed_queue_mut();
                        queue.queue_cursor =
                            (queue.queue_cursor as i64 + delta).clamp(0, n as i64 - 1) as usize;
                    }
                } else if left_area.contains((col, row).into()) {
                    if self.library_tab == 0 {
                        self.power_cw_move_cursor(delta);
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
                            if self.library_tab == 0 {
                                self.power_home_select_section(target);
                            } else {
                                let lib_idx = self.library_tab - 1;
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
                        if t < queue.items.len()
                            && self.layout.main.queue_area.contains((col, row).into())
                        {
                            self.dispatch(Command::QueuePlayCursor);
                        }
                    } else if self.library_tab == 0 {
                        self.power_home_play();
                    } else if self
                        .current_lib_item()
                        .map(|i| !i.is_folder)
                        .unwrap_or(false)
                    {
                        self.select();
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
                // Header breadcrumb clicks.
                if self.library_tab > 0 {
                    let crumbs = self.layout.main.breadcrumbs.clone();
                    let lib_idx = self.library_tab - 1;
                    for (x_start, x_end, crumb_row, target_depth) in crumbs {
                        if row == crumb_row && col >= x_start && col < x_end {
                            self.libs[lib_idx].nav_stack.truncate(target_depth);
                            self.libs[lib_idx].search = None;
                            self.save_default_library_position(lib_idx);
                            return;
                        }
                    }
                }

                // Capture the row that was already selected *before* this
                // click moves the cursor. Clicking a folder row normally
                // emulates Enter (drills in), but if the click landed on
                // the row that was already selected (e.g. re-clicking the
                // current row), drilling in again produces a jarring,
                // unrequested navigation. Treat that case as a no-op
                // instead.
                let prev_id = self.current_lib_item().map(|i| i.id);
                let hit = self.click_set_cursor(col, row);
                if hit && self.library_tab > 0 {
                    let lib_idx = self.library_tab - 1;
                    if self.activate_recursive_album(lib_idx) {
                        // active-search jump; unchanged
                    } else if self.is_viewing_album_folders(lib_idx) {
                        // Track-selection mode only opens via Enter; mouse
                        // click never opens it. If it's already open, a click
                        // still plays the focused track (mirrors Enter's
                        // "already focused" branch inside
                        // `activate_album_folder_row`), but cannot open it.
                        if self.libs[lib_idx].album_track_focus.is_some() {
                            self.activate_album_folder_row(lib_idx);
                        }
                    } else {
                        let cur_item = self.current_lib_item();
                        let already_selected = cur_item
                            .as_ref()
                            .is_some_and(|i| prev_id.as_deref() == Some(i.id.as_str()));
                        // TV `Series` rows are always `is_folder` (they have
                        // seasons underneath), but Enter on a Series row
                        // doesn't drill into a generic folder browse -- it
                        // opens the inline series-selection detail (season
                        // pills + episode list; see
                        // `enter_series_selection`). A mouse click has no
                        // equivalent of that inline mode, so calling
                        // `select()` here would instead push a raw
                        // folder-browse nav level, a screen the click never
                        // asked for. Mouse clicks on Series rows -- and on
                        // any row that was already selected before this
                        // click -- should only ever move the
                        // cursor/highlight; leave opening the detail view to
                        // Enter/double-click.
                        let is_series = cur_item.as_ref().is_some_and(|i| i.item_type == "Series");
                        if !already_selected
                            && !is_series
                            && cur_item.map(|i| i.is_folder).unwrap_or(false)
                        {
                            self.select();
                        }
                    }
                }
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
