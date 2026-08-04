use super::{
    App, PanelFocus, QueueScope, SavePlaylistDialog, SavePlaylistStage, POWER_LEFT_WIDTH_DEFAULT,
    POWER_LEFT_WIDTH_STEP,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::Instant;

impl App {
    pub(super) fn handle_key_queue_column_width(&mut self, key: KeyEvent) -> Option<bool> {
        if self.handle_queue_column_width_key(key) {
            Some(false)
        } else {
            None
        }
    }

    fn is_queue_column_width_resize_key(key: KeyEvent) -> bool {
        matches!(key.code, KeyCode::Left | KeyCode::Right) && key.modifiers == KeyModifiers::SHIFT
    }

    fn handle_queue_column_width_key(&mut self, key: KeyEvent) -> bool {
        if self.context_menu_open()
            || self.queue_column_collapsed
            || !Self::is_queue_column_width_resize_key(key)
        {
            return false;
        }

        let max_width = Self::queue_column_width_max_for_terminal(self.terminal_width);
        let next_width = if key.code == KeyCode::Left {
            self.queue_column_width
                .saturating_sub(POWER_LEFT_WIDTH_STEP)
        } else {
            self.queue_column_width
                .saturating_add(POWER_LEFT_WIDTH_STEP)
        };
        let normalized = Self::normalize_queue_column_width(next_width, self.terminal_width);
        if normalized == self.queue_column_width {
            let limit = if key.code == KeyCode::Left {
                format!("Queue column width already at minimum ({POWER_LEFT_WIDTH_DEFAULT} cols)")
            } else {
                format!("Queue column width already at maximum ({max_width} cols)")
            };
            self.flash_status(limit);
            return true;
        }

        self.queue_column_width = normalized;
        self.save_prefs();
        self.flash_status(format!(
            "Queue column width: {} cols",
            self.queue_column_width
        ));
        true
    }

    pub(super) fn handle_queue_key(&mut self, key: KeyEvent) -> bool {
        // Bare Left/Right switch focus between the two panels. Queue is on
        // the left; library is on the right.
        if key.modifiers.is_empty() {
            if key.code == KeyCode::Right && matches!(self.panel_focus, PanelFocus::Queue) {
                self.set_panel_focus(PanelFocus::Library);
                self.last_card_height = 0; // reset stale image height for new view
                return false;
            }
            if key.code == KeyCode::Left
                && matches!(self.panel_focus, PanelFocus::Library)
                && !self.queue_column_collapsed
            {
                self.set_panel_focus(PanelFocus::Queue);
                self.last_card_height = 0;
                return false;
            }
        }

        // Bracket keys are panel-scoped; the queue panel owns Local/Remote
        // scope switching, while the left panel keeps its section/season/
        // group bracket actions.
        if matches!(self.panel_focus, PanelFocus::Queue) {
            match key.code {
                KeyCode::Char('[')
                    if self.has_direct_remote_queue()
                        && !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !key.modifiers.contains(KeyModifiers::ALT) =>
                {
                    self.set_queue_scope(QueueScope::Local);
                    return false;
                }
                KeyCode::Char(']')
                    if self.has_direct_remote_queue()
                        && !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !key.modifiers.contains(KeyModifiers::ALT) =>
                {
                    self.set_queue_scope(QueueScope::Remote);
                    return false;
                }
                _ => {}
            }
        }

        // Route nav keys to the focused library panel.
        if matches!(self.panel_focus, PanelFocus::Library) {
            if self.library_tab == 0 && self.handle_power_cw_key(key) {
                return false;
            }
            if self.library_tab > 0 {
                let lib_idx = self.library_tab - 1;

                // Season switching: [ = previous season, ] = next season.
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT)
                {
                    if key.code == KeyCode::Char('[') && self.is_music_group_view(lib_idx) {
                        self.switch_music_group(lib_idx, -1);
                        return false;
                    }
                    if key.code == KeyCode::Char(']') && self.is_music_group_view(lib_idx) {
                        self.switch_music_group(lib_idx, 1);
                        return false;
                    }
                    if key.code == KeyCode::Char('[') && self.is_feed_home_video_group_view(lib_idx)
                    {
                        self.switch_feed_folder_group(lib_idx, -1);
                        return false;
                    }
                    if key.code == KeyCode::Char(']') && self.is_feed_home_video_group_view(lib_idx)
                    {
                        self.switch_feed_folder_group(lib_idx, 1);
                        return false;
                    }
                    // Letter-range pill cycling for large non-music libraries
                    // (`[`/`]` are otherwise free at the top browse level).
                    if key.code == KeyCode::Char('[') && self.should_show_letter_pills(lib_idx) {
                        self.cycle_letter_pill(lib_idx, -1);
                        return false;
                    }
                    if key.code == KeyCode::Char(']') && self.should_show_letter_pills(lib_idx) {
                        self.cycle_letter_pill(lib_idx, 1);
                        return false;
                    }
                }

                let is_power_nav = matches!(
                    key.code,
                    KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down
                ) && key.modifiers.contains(KeyModifiers::ALT);
                // Route non-power-nav keys to the library handler for this
                // panel. `handle_lib_key`'s own `Some`/`None` is now the
                // single source of truth for "did the library view claim
                // this key" — no more hand-maintained mirror of its key set.
                //
                // The `library_tab` derivation stays: many action methods
                // `handle_lib_key` calls into (`current_lib_item`, `select`,
                // `move_lib_cursor`, `refresh_lib`, `shuffle_play`,
                // `play_folder`, `go_back`, ...) derive their own lib index
                // from `self.library_tab` rather than taking it as a parameter.
                // Impact analysis on `current_lib_item` alone showed 6
                // affected symbols across `execute_context_action`,
                // `enqueue_selected`, `select`, and `toggle_watched`
                // (HIGH risk) — parameterizing all of them is a separate,
                // larger follow-up, not in scope for #132.
                // Track-selection mode (#145 task 3): while the power-left
                // panel is sitting on the album-folder-listing nav level
                // (the level `render_power_library` shows inline album
                // detail for, per task 2), Enter/Escape/Up/Down are
                // reinterpreted for moving a track focus within the
                // currently-displayed album instead of drilling into
                // `nav_stack` (`select`) or moving the album cursor
                // (`move_lib_cursor`). Scoped strictly to `!is_power_nav`
                // (so Alt+arrow pane-switching is untouched) and to
                // `is_viewing_album_folders` (so movies/series/home-video
                // panels and non-power tabs are completely unaffected; the
                // legacy `is_album_level` drilldown this used to also
                // exclude was removed entirely -- Enter now always routes
                // here via `activate_album_folder_row`).
                if !is_power_nav && self.is_viewing_album_folders(lib_idx) {
                    match key.code {
                        KeyCode::Enter => {
                            self.activate_album_folder_row(lib_idx);
                            return false;
                        }
                        KeyCode::Esc | KeyCode::Backspace => {
                            if self.libs[lib_idx].album_track_focus.is_some() {
                                self.libs[lib_idx].album_track_focus = None;
                                return false;
                            }
                        }
                        KeyCode::Up | KeyCode::Down => {
                            if let Some(idx) = self.libs[lib_idx].album_track_focus {
                                let track_count = self
                                    .selected_album_item(lib_idx)
                                    .and_then(|item| self.album_tracks_cache.get(&item.id))
                                    .map(|tracks| tracks.len())
                                    .unwrap_or(0);
                                if track_count > 0 {
                                    let delta: i64 = if key.code == KeyCode::Up { -1 } else { 1 };
                                    let new_idx = (idx as i64 + delta)
                                        .clamp(0, track_count as i64 - 1)
                                        as usize;
                                    self.libs[lib_idx].album_track_focus = Some(new_idx);
                                }
                                return false;
                            }
                        }
                        _ => {}
                    }
                }

                // Series-selection mode: while the power-left panel has a
                // Series item selected and selection mode is active, Enter/
                // Escape/Up/Down/[/] are intercepted for navigating within
                // the inline series detail (season pills + episode list)
                // instead of drilling into `nav_stack` (`select`) or
                // moving the list cursor (`move_lib_cursor`).
                if !is_power_nav && self.libs[lib_idx].series_selection.is_some() {
                    match key.code {
                        KeyCode::Enter => {
                            // Play the focused episode in selection mode.
                            if let Some(episodes) = self.series_selection_episodes(lib_idx) {
                                let ep_idx = self.libs[lib_idx].series_selection.unwrap_or(0);
                                if let Some(ep) = episodes.get(ep_idx) {
                                    let ep = ep.clone();
                                    self.libs[lib_idx].series_selection = None;
                                    self.play_item(ep);
                                }
                            }
                            return false;
                        }
                        KeyCode::Esc | KeyCode::Backspace => {
                            self.libs[lib_idx].series_selection = None;
                            return false;
                        }
                        KeyCode::Up | KeyCode::Down => {
                            let delta: i64 = if key.code == KeyCode::Up { -1 } else { 1 };
                            if let Some(episodes) = self.series_selection_episodes(lib_idx) {
                                let count = episodes.len();
                                if count > 0 {
                                    let cur = self.libs[lib_idx].series_selection.unwrap_or(0);
                                    let new_idx =
                                        (cur as i64 + delta).clamp(0, count as i64 - 1) as usize;
                                    self.libs[lib_idx].series_selection = Some(new_idx);
                                }
                            }
                            return false;
                        }
                        KeyCode::Char('[') => {
                            self.switch_series_selection_season(lib_idx, -1);
                            return false;
                        }
                        KeyCode::Char(']') => {
                            self.switch_series_selection_season(lib_idx, 1);
                            return false;
                        }
                        _ => {}
                    }
                }
                // Activate series-selection mode on Enter when the cursor is
                // on a Series item (instead of drilling down via `select`).
                if !is_power_nav
                    && key.code == KeyCode::Enter
                    && self.libs[lib_idx].series_selection.is_none()
                {
                    if let Some(item) = self.power_selected_series_item(lib_idx) {
                        self.enter_series_selection(lib_idx, &item);
                        return false;
                    }
                }

                // Let the shared Tab/BackTab cycling path run after this block.
                if !is_power_nav && !matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
                    if let Some(quit) = self.handle_lib_key(lib_idx, key) {
                        return quit;
                    }
                }
            }
        }

        // Queue focus: PageUp/PageDown use the actual queue panel height.
        if matches!(self.panel_focus, PanelFocus::Queue) {
            let page = self.layout.main.queue_area.height.saturating_sub(1).max(1) as usize;
            match key.code {
                KeyCode::PageUp => {
                    self.last_nav_at = Instant::now();
                    let queue = self.displayed_queue_mut();
                    queue.queue_cursor = queue.queue_cursor.saturating_sub(page);
                    return false;
                }
                KeyCode::PageDown => {
                    self.last_nav_at = Instant::now();
                    let queue = self.displayed_queue_mut();
                    let n = queue.items.len();
                    queue.queue_cursor = (queue.queue_cursor + page).min(n.saturating_sub(1));
                    return false;
                }
                _ => {}
            }
        }

        if let Some(quit) = self.handle_global_view_key(key) {
            return quit;
        }

        match key.code {
            KeyCode::Char('t')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(self.panel_focus, PanelFocus::Queue)
                    && self.remote_tracker.is_some() =>
            {
                self.stop_remote_tracking();
            }
            KeyCode::Char('r')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(self.panel_focus, PanelFocus::Queue)
                    && self.remote_tracker.is_some() =>
            {
                self.reanchor_remote_tracking();
            }
            KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.move_queue_item_up();
            }
            KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.move_queue_item_down();
            }
            KeyCode::Up if self.displayed_queue().queue_cursor > 0 => {
                self.last_nav_at = Instant::now();
                self.displayed_queue_mut().queue_cursor -= 1;
            }
            KeyCode::Down
                if self.displayed_queue().queue_cursor + 1 < self.displayed_queue().items.len() =>
            {
                self.last_nav_at = Instant::now();
                self.displayed_queue_mut().queue_cursor += 1;
            }
            KeyCode::PageUp => {
                let p = self.queue_page_size();
                let queue = self.displayed_queue_mut();
                queue.queue_cursor = queue.queue_cursor.saturating_sub(p);
            }
            KeyCode::PageDown => {
                let p = self.queue_page_size();
                let queue = self.displayed_queue_mut();
                let n = queue.items.len();
                queue.queue_cursor = (queue.queue_cursor + p).min(n.saturating_sub(1));
            }
            KeyCode::Home => {
                self.displayed_queue_mut().queue_cursor = 0;
            }
            KeyCode::End => {
                let n = self.displayed_queue().items.len();
                if n > 0 {
                    self.displayed_queue_mut().queue_cursor = n - 1;
                }
            }
            KeyCode::Enter => {
                self.dispatch(super::action::Command::QueuePlayCursor);
            }
            KeyCode::Delete => {
                let queue = self.displayed_queue();
                let t = queue.queue_cursor;
                if t < queue.items.len() {
                    self.remove_from_queue(t);
                }
            }
            KeyCode::Char('z') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let scope = self.visible_queue_scope();
                if scope == QueueScope::Remote {
                    self.flash_status_high("Undo is not supported for remote queue edits".into());
                    return false;
                }
                self.undo_last_queue_edit(scope);
            }
            KeyCode::Char('i') => {
                let queue = self.displayed_queue();
                let cursor = queue.queue_cursor;
                if let Some(item) = queue.items.get(cursor) {
                    let item_id = item.id.clone();
                    let item_type = item.item_type.clone();
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
            }
            KeyCode::Char('/') => {
                self.search.open(true);
                return false;
            }
            KeyCode::Char('p') => {
                let (active, current_idx) = {
                    let s = self.player.status.lock().unwrap();
                    (s.active, s.current_idx)
                };
                if active {
                    self.playback_queue_mut().queue_cursor = current_idx;
                    if self.player.is_remote() {
                        self.set_queue_scope(QueueScope::Remote);
                    }
                } else {
                    self.flash_status_high("Nothing is playing".into());
                }
            }
            KeyCode::Char('s')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                if !self.player_tab.items.is_empty() {
                    self.save_playlist_dialog = Some(SavePlaylistDialog {
                        input: self.queue_playlist_name().to_string(),
                        stage: SavePlaylistStage::EnterName,
                    });
                }
            }
            KeyCode::Left | KeyCode::Up if key.modifiers.contains(KeyModifiers::ALT) => {
                self.library_tab_prev();
            }
            KeyCode::Right | KeyCode::Down if key.modifiers.contains(KeyModifiers::ALT) => {
                self.library_tab_next();
            }
            _ => {}
        }
        false
    }
}
