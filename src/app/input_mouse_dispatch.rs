#![allow(unused_imports)]

use crate::app::action::Command;
use crate::app::layout::LibraryRowTarget;
use crate::app::{
    App, PanelFocus, PendingQueueAction, QueueScope, TabSelection, HELP_PANEL_W, PLAYLISTS_PANEL_W,
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

        if self.context_menu.is_some() {
            if !matches!(
                mouse.kind,
                crossterm::event::MouseEventKind::Down(MouseButton::Left)
            ) {
                return;
            }
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
                            self.execute_context_action(action);
                        } else {
                            self.context_menu = None;
                            self.layout.context_menu_rect = None;
                            self.force_clear = true;
                        }
                    } else {
                        self.context_menu = None;
                        self.layout.context_menu_rect = None;
                        self.force_clear = true;
                    }
                } else {
                    self.context_menu = None;
                    self.layout.context_menu_rect = None;
                    self.force_clear = true;
                }
            } else {
                self.context_menu = None;
                self.force_clear = true;
            }
            return;
        }

        // Suppress all mouse actions while the terminal does not have
        // focus.  `refocus_at` is `None` when unfocused (or never yet
        // focused); `Some` when focused.  When transitioning from
        // unfocused to focused the first click within `REFOCUS_WINDOW`
        // is the click that merely brought the window into focus and is
        // swallowed.  After the grace window expires, `refocus_at`
        // remains `Some` so subsequent clicks dispatch normally until
        // the next `FocusLost`.
        const REFOCUS_WINDOW: Duration = Duration::from_millis(150);
        match self.refocus_at {
            None => {
                // Terminal is not focused -- suppress everything.
                return;
            }
            Some(ref t) if t.elapsed() < REFOCUS_WINDOW => {
                // Within the grace window after FocusGained: swallow
                // button-down events (the refocusing click).  Track
                // mouse position so hover rendering stays current, but
                // take no action.
                if matches!(
                    mouse.kind,
                    MouseEventKind::Down(MouseButton::Left)
                        | MouseEventKind::Down(MouseButton::Right)
                ) {
                    log::debug!(target: "input", "suppressed refocus click at ({col}, {row})");
                    return;
                }
            }
            _ => {}
        }

        // A selection modal owns the topmost browse targets. Consume every
        // mouse event while it is open so the underlying library cannot see
        // a click intended for a modal row or pill.
        if self.selection_modal.is_some() {
            if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
                return;
            }
            let pos = (col, row).into();
            if !self.layout.main.selection_modal_area.contains(pos) {
                self.close_selection_modal();
                return;
            }
            if let Some(target) = self
                .layout
                .main
                .selector_tabs
                .iter()
                .find(|(rect, _)| rect.contains(pos))
                .map(|(_, target)| *target)
            {
                let source = self
                    .selection_modal
                    .as_ref()
                    .map(|modal| modal.source.clone());
                match source {
                    Some(crate::app::types_selection_modal::SelectionModalSource::Series {
                        ..
                    }) => self.select_series_selection_modal_season(target),
                    Some(crate::app::types_selection_modal::SelectionModalSource::Podcast {
                        ..
                    }) => self.select_podcast_selection_modal_filter(target),
                    _ => {}
                }
                return;
            }
            if let Some(row_index) = self
                .layout
                .main
                .selection_modal_rows
                .iter()
                .find(|(rect, _)| rect.contains(pos))
                .map(|(_, row_index)| *row_index)
            {
                if let Some(modal) = self.selection_modal.as_mut() {
                    modal.cursor = row_index;
                }
                self.activate_selection_modal_item();
            }
            return;
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
                if self.layout.playback.ind_vol.contains((col, row).into()) {
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
                            super::ui_util::move_cursor(queue.queue_cursor, delta, n);
                    }
                } else if let TabSelection::AudiobookshelfLibrary(index) = self.tab {
                    let is_book = matches!(
                        self.audiobookshelf_kind_at(index),
                        Some(
                            crate::app::types_audiobookshelf_browse::AudiobookshelfBrowseKind::Book
                        )
                    );
                    let right_area = self.layout.main.audiobookshelf_book_right_area;
                    if is_book && right_area.contains((col, row).into()) {
                        self.move_audiobookshelf_book_cursor(delta * 3);
                    } else if left_area.contains((col, row).into()) {
                        self.handle_mouse_scroll_browse(delta);
                    }
                } else if left_area.contains((col, row).into()) {
                    self.handle_mouse_scroll_browse(delta);
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let now = Instant::now();

                let is_double = now.duration_since(self.last_click_time)
                    < Duration::from_millis(400)
                    && self.last_click_pos == (col, row);
                self.last_click_time = now;
                self.last_click_pos = (col, row);

                {
                    for (rect, target) in self.layout.main.selector_tabs.clone() {
                        if rect.contains((col, row).into()) {
                            // Selector tabs are shared geometry, but which
                            // Service's group/pill/filter action they invoke
                            // is destination-specific: dispatch exhaustively
                            // and no-op on a stale completed-frame layout.
                            if !self.browse_mouse_ready() {
                                return;
                            }
                            match self.tab {
                                TabSelection::Home => self.home_select_section(target),
                                TabSelection::Feeds => self.feed_tab_select_group(target),
                                TabSelection::AudiobookshelfLibrary(index) => {
                                    match self.audiobookshelf_kind_at(index) {
                                        Some(crate::app::types_audiobookshelf_browse::AudiobookshelfBrowseKind::Book) => {
                                            self.select_audiobookshelf_book_bucket(target);
                                        }
                                        _ if self.podcast_filter_target_active(index) => {
                                            self.select_audiobookshelf_filter(target);
                                        }
                                        _ => self.select_audiobookshelf_podcast_bucket(target),
                                    }
                                }
                                TabSelection::EmbyLibrary(lib_idx) => {
                                    if self.is_music_group_view(lib_idx) {
                                        self.select_music_group(lib_idx, target);
                                    } else if self.is_feed_home_video_group_view(lib_idx) {
                                        self.select_feed_folder_group(lib_idx, target);
                                    } else if self.should_show_letter_pills(lib_idx) {
                                        self.select_letter_pill(lib_idx, target);
                                    }
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
                    if matches!(self.effective_panel_focus(), PanelFocus::Queue) {
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
                    } else if self.browse_mouse_ready() {
                        // Browse-located double-click: dispatch by the
                        // selected destination before interpreting any
                        // Service-local geometry (design §4). Each Service
                        // reads only its own hit targets / index space; there
                        // is no default-to-Emby branch.
                        if let TabSelection::EmbyLibrary(lib_idx) = self.tab {
                            if self.layout.main.is_wide_tv_active() {
                                let pos = (col, row).into();
                                if self
                                    .layout
                                    .main
                                    .tv_wide_episode_rows
                                    .iter()
                                    .any(|(rect, _)| rect.contains(pos))
                                {
                                    self.activate_series_selection_episode(lib_idx);
                                } else if self.layout.main.tv_wide_right_area.contains(pos) {
                                    self.activate_selected_series(lib_idx);
                                }
                                return;
                            }
                        }
                        let pos = (col, row).into();
                        let in_left = self.layout.main.left_area.contains(pos)
                            || self.layout.main.inline_hero_area.contains(pos);
                        if let TabSelection::EmbyLibrary(lib_idx) = self.tab {
                            if self.layout.main.is_wide_tv_active()
                                && self
                                    .layout
                                    .main
                                    .tv_wide_episode_rows
                                    .iter()
                                    .any(|(rect, _)| rect.contains(pos))
                            {
                                self.activate_series_selection_episode(lib_idx);
                                return;
                            }
                            if self.is_music_group_view(lib_idx) {
                                let track_idx = self
                                    .layout
                                    .main
                                    .is_wide_music_active()
                                    .then(|| self.layout.main.wide_music_track_at(pos))
                                    .flatten();
                                if let Some(track_idx) = track_idx {
                                    self.libs[lib_idx].album_track_focus = Some(track_idx);
                                    self.select(lib_idx);
                                    return;
                                }
                            }
                        }
                        match self.tab {
                            TabSelection::Home if in_left => self.home_play(),
                            TabSelection::Home => {}
                            TabSelection::Feeds => {
                                // Double-click on Feeds: no-op (playback wiring pending).
                            }
                            TabSelection::AudiobookshelfLibrary(index) => {
                                let Some(kind) = self.audiobookshelf_kind_at(index) else {
                                    return;
                                };
                                match kind {
                                    crate::app::types_audiobookshelf_browse::AudiobookshelfBrowseKind::Podcast => {
                                        if in_left {
                                            let in_episodes = self
                                                .audiobookshelf_browse
                                                .get(index)
                                                .is_some_and(|state| state.episode_selection.is_some());
                                            if !in_episodes {
                                                if self.layout.main.is_wide_podcast_active() {
                                                    self.enter_audiobookshelf_episode_selection();
                                                } else {
                                                    self.open_podcast_selection_modal();
                                                }
                                            } else {
                                                // Episode activation: inert seam for
                                                // #518 (double-click on a selected
                                                // episode).
                                                self.activate_audiobookshelf_episode(index);
                                            }
                                        }
                                    }
                                    crate::app::types_audiobookshelf_browse::AudiobookshelfBrowseKind::Book => {
                                        if !self.layout.main.is_wide_book_active() && in_left {
                                            self.activate_audiobookshelf_book_parent();
                                        } else {
                                            let in_chapters = self
                                                .audiobookshelf_book_browse
                                                .get(index)
                                                .is_some_and(|state| state.chapter_selection.is_some());
                                            if in_chapters
                                                && self
                                                    .layout
                                                    .main
                                                    .audiobookshelf_book_chapter_rows
                                                    .iter()
                                                    .any(|(rect, _)| rect.contains(pos))
                                            {
                                                self.activate_audiobookshelf_book_row();
                                            }
                                        }
                                    }
                                }
                            }
                            TabSelection::EmbyLibrary(lib_idx) => {
                                if in_left {
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
                                    // Wide Music: double-click on a track plays it.
                                    if self.layout.main.is_wide_music_active() {
                                        let pos = (col, row).into();
                                        if let Some(track_idx) =
                                            self.layout.main.wide_music_track_at(pos)
                                        {
                                            self.libs[lib_idx].album_track_focus = Some(track_idx);
                                            self.select(lib_idx);
                                        }
                                        // Double-click on artwork or blank space: no-op.
                                        return;
                                    }
                                    if self.activate_recursive_album(lib_idx) {
                                        // active-search jump; unchanged
                                    } else if self.is_viewing_album_folders(lib_idx) {
                                        self.activate_album_folder_row(lib_idx);
                                    } else if self.libs[lib_idx].series_selection.is_some() {
                                        self.activate_series_selection_episode(lib_idx);
                                    } else if !self.activate_selected_series(lib_idx) {
                                        self.select(lib_idx);
                                    }
                                }
                            }
                        }
                    }
                    // Wide Music: double-click on right pane album enters
                    // track selection (same as Enter). Wide Music is an Emby
                    // surface, so the right pane is only interpreted for an
                    // explicitly selected Emby library.
                    if self.layout.main.is_wide_music_active()
                        && self
                            .layout
                            .main
                            .wide_music_right_area
                            .contains((col, row).into())
                    {
                        if let TabSelection::EmbyLibrary(lib_idx) = self.tab {
                            self.activate_album_folder_row(lib_idx);
                        }
                    }
                    return;
                }

                if self.layout.playback.ind_rc.contains((col, row).into()) {
                    self.show_sessions = !self.show_sessions;
                    if self.show_sessions {
                        self.spawn_sessions_load();
                        self.spawn_cast_discovery();
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
                // Header breadcrumb clicks (an Emby-only surface: the
                // crumb trail is published by the Emby renderer, so it is
                // only interpreted for an explicitly selected Emby library).
                if let TabSelection::EmbyLibrary(lib_idx) = self.tab {
                    if !self.browse_mouse_ready() {
                        return;
                    }
                    let target_depth = self
                        .layout
                        .main
                        .breadcrumbs
                        .iter()
                        .copied()
                        .find(|&(x_start, x_end, crumb_row, _)| {
                            row == crumb_row && col >= x_start && col < x_end
                        })
                        .map(|(_, _, _, target_depth)| target_depth);
                    if let Some(target_depth) = target_depth {
                        self.libs[lib_idx].nav_stack.truncate(target_depth);
                        self.save_default_library_position(lib_idx);
                        return;
                    }
                }

                // Single click only focuses the clicked row. Activation --
                // playing a media item, drilling into a folder, opening
                // track/series selection -- is a double-click (or Enter)
                // gesture and never happens here.
                self.click_set_cursor(col, row);
            }
            MouseEventKind::Down(MouseButton::Right) => {
                // Right-click dispatches by destination: Home and Emby open
                // their existing menu after focusing the clicked row;
                // Audiobookshelf and Feeds right-clicks focus the row but
                // never open an Emby menu (menu construction refinement is
                // Section 6.2). A stale completed-frame layout no-ops the
                // whole gesture.
                if !self.browse_mouse_ready() {
                    return;
                }
                match self.tab {
                    TabSelection::Home | TabSelection::EmbyLibrary(_) => {
                        if self.click_set_cursor(col, row) {
                            self.open_context_menu_at(col, row);
                        }
                    }
                    TabSelection::AudiobookshelfLibrary(_) | TabSelection::Feeds => {
                        self.click_set_cursor(col, row);
                    }
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

    /// Wheel-scroll in the library panel dispatches by destination (design
    /// §4). Each Service reads only its own cursor state / episode mode; no
    /// default-to-Emby branch. A stale completed-frame layout no-ops.
    fn handle_mouse_scroll_browse(&mut self, delta: i64) {
        if !self.browse_mouse_ready() {
            return;
        }
        match self.tab {
            TabSelection::Home => self.cw_move_cursor(delta),
            TabSelection::EmbyLibrary(lib_idx) => self.move_lib_cursor(lib_idx, delta),
            TabSelection::AudiobookshelfLibrary(_) => {
                let in_episodes = self
                    .tab
                    .audiobookshelf_index()
                    .and_then(|index| self.audiobookshelf_browse.get(index))
                    .is_some_and(|state| state.episode_selection.is_some());
                if in_episodes {
                    self.move_audiobookshelf_episode_cursor(delta * 3);
                } else {
                    self.move_audiobookshelf_show_rows(delta * 3);
                }
            }
            TabSelection::Feeds => self.feed_tab_move_cursor(delta),
        }
    }
}
