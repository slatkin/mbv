use super::action::{album_track_command_for_key, Command};
use super::input_resolver::KeyChord;
use super::{App, ConfirmAction, ConfirmModal, PanelFocus};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::{Duration, Instant};

impl App {
    pub(super) fn active_album_track_lib_idx(&self) -> Option<usize> {
        let lib_idx = self.tab.emby_library_index()?;
        let lib = self.libs.get(lib_idx)?;
        if lib.album_track_focus.is_some() && self.is_viewing_album_folders(lib_idx) {
            Some(lib_idx)
        } else {
            None
        }
    }

    pub(super) fn handle_key_album_track_mode(&mut self, key: KeyEvent) -> Option<bool> {
        if matches!(self.effective_panel_focus(), PanelFocus::Queue)
            && matches!(key.code, KeyCode::Up | KeyCode::Down)
        {
            return None;
        }
        let lib_idx = self.active_album_track_lib_idx()?;
        let command = album_track_command_for_key(KeyChord::from_key(key), lib_idx)?;
        Some(self.dispatch(command))
    }

    pub(super) fn handle_key_panel_mode_cycle(&mut self, key: KeyEvent) -> Option<bool> {
        if key.code != KeyCode::Char('x') || !key.modifiers.is_empty() {
            return None;
        }
        Some(self.dispatch(Command::CyclePanelMode))
    }

    /// Global view keys shared by the left-column handlers (`handle_lib_key`,
    /// `handle_queue_key`, and Home nav via `handle_cw_key`): quit, tab
    /// cycling, digit tab-jump, and the context-menu key. Each handler calls
    /// this at the point in its own precedence order where these keys used
    /// to be independently matched; genuinely per-view behavior (`/` search,
    /// `Ctrl+a` enqueue) stays local. See
    /// docs/adr/0002-centralized-input-handling.md, phase 3 (#132).
    pub(super) fn handle_global_view_key(&mut self, key: KeyEvent) -> Option<bool> {
        match key.code {
            KeyCode::Char('q') if key.modifiers.is_empty() => Some(self.try_quit()),
            KeyCode::Tab => {
                self.library_tab_next();
                Some(false)
            }
            KeyCode::BackTab => {
                self.library_tab_prev();
                Some(false)
            }
            KeyCode::Char(c @ '1'..='9') => {
                let idx = (c as usize) - ('1' as usize);
                if idx < self.tab_count() {
                    self.set_library_tab(idx);
                }
                Some(false)
            }
            KeyCode::Char('.') => {
                self.open_context_menu();
                Some(false)
            }
            _ => None,
        }
    }

    /// `Ctrl+a`: enqueue the current selection. Shared by `handle_lib_key`
    /// (Home nav has its own enqueue binding) -- the queue view has no
    /// "enqueue selected" concept, so `handle_queue_key` does not call this.
    ///
    /// Issue #209: `a` (no modifier) is the playback-context audio-track
    /// binding (`playback_command_for_key`'s `ToggleMuteOrCycleAudio`,
    /// guarded `!ctrl` there so Ctrl+a never reaches it). Previously Ctrl+a
    /// fell through this front door unbound and hit that playback binding
    /// unguarded, muting/cycling audio instead of enqueuing. This arm is
    /// the actual fix: Ctrl+a now means "enqueue" here, before the playback
    /// context even sees the key. Replaces the old `Ctrl+q`/`Alt+q`
    /// bindings, which no longer enqueue.
    fn handle_enqueue_selected_key(&mut self, lib_idx: usize, key: KeyEvent) -> Option<bool> {
        match key.code {
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.enqueue_selected(Some(lib_idx));
                Some(false)
            }
            _ => None,
        }
    }

    /// Edit the active library search query. Only query-editing keys are
    /// recognised here: `Esc` closes the search, `Backspace` deletes one
    /// character, and printable characters extend the query. Navigation
    /// keys (arrows, page, Home/End, Enter) and the new vim-nav letters
    /// (h/j/k/l) are deliberately NOT handled in this context -- typing
    /// into the query and moving the result cursor must not interleave.
    /// To navigate results, close the search (Esc) and use the flat-list
    /// bindings, which include h/j/k/l in 2-col mode.
    pub(super) fn handle_lib_key(&mut self, lib_idx: usize, key: KeyEvent) -> Option<bool> {
        if let Some(quit) = self.handle_enqueue_selected_key(lib_idx, key) {
            return Some(quit);
        }
        if let Some(quit) = self.handle_global_view_key(key) {
            return Some(quit);
        }

        match key.code {
            KeyCode::Esc | KeyCode::Backspace => self.go_back(lib_idx),
            KeyCode::Up => {
                if self.is_viewing_season_grid(lib_idx) {
                    self.move_lib_cursor(lib_idx, -4);
                } else {
                    self.move_lib_cursor_rows(lib_idx, -1);
                }
            }
            KeyCode::Down => {
                if self.is_viewing_season_grid(lib_idx) {
                    self.move_lib_cursor(lib_idx, 4);
                } else {
                    self.move_lib_cursor_rows(lib_idx, 1);
                }
            }
            KeyCode::Left if self.is_viewing_season_grid(lib_idx) => {
                self.move_lib_cursor(lib_idx, -1)
            }
            KeyCode::Right if self.is_viewing_season_grid(lib_idx) => {
                self.move_lib_cursor(lib_idx, 1)
            }
            // Arrow-key column navigation: in 2-col lists (flat,
            // letter-grouped, and grouped-album views) Left/Right mirror h/l
            // (Up/Down already mirror j/k). Season-grid Left/Right are
            // covered above.
            KeyCode::Left if self.current_library_columns(lib_idx) > 1 => {
                self.move_lib_cursor(lib_idx, -1)
            }
            KeyCode::Right if self.current_library_columns(lib_idx) > 1 => {
                self.move_lib_cursor(lib_idx, 1)
            }
            // Vim-style navigation. Complements the arrow keys (Left/Right
            // and Up/Down above carry the same movements):
            //   j/k mirror Up/Down (any column count) -- `j` is down, `k` is up.
            //   h/l mirror Left/Right across cells in 2-col mode; in 1-col
            //   mode h/l are unbound (left as a free input character -- h is
            //   not a global key now that the panel-mode cycle moved to `x`).
            KeyCode::Char('j') => {
                if self.is_viewing_season_grid(lib_idx) {
                    self.move_lib_cursor(lib_idx, 4);
                } else {
                    self.move_lib_cursor_rows(lib_idx, 1);
                }
            }
            KeyCode::Char('k') => {
                if self.is_viewing_season_grid(lib_idx) {
                    self.move_lib_cursor(lib_idx, -4);
                } else {
                    self.move_lib_cursor_rows(lib_idx, -1);
                }
            }
            KeyCode::Char('l') if self.current_library_columns(lib_idx) > 1 => {
                self.move_lib_cursor(lib_idx, 1)
            }
            KeyCode::Char('h') if self.current_library_columns(lib_idx) > 1 => {
                self.move_lib_cursor(lib_idx, -1)
            }
            KeyCode::PageUp => {
                if !self.page_grouped_album_cursor(lib_idx, false) {
                    let p = self.lib_page_size();
                    self.move_lib_cursor_rows(lib_idx, -(p as i64));
                }
            }
            KeyCode::PageDown => {
                if !self.page_grouped_album_cursor(lib_idx, true) {
                    let p = self.lib_page_size();
                    self.move_lib_cursor_rows(lib_idx, p as i64);
                }
            }
            KeyCode::Home => self.jump_lib_cursor(lib_idx, false),
            KeyCode::End => self.jump_lib_cursor(lib_idx, true),
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let item = self.current_lib_item(lib_idx);
                if let Some(item) = item {
                    if item.is_folder {
                        let ct = self.libs[lib_idx].library.collection_type.clone();
                        self.queue_source = crate::config::QueueSource::Collection {
                            collection_type: ct,
                        };
                        self.play_folder(&item.id.clone());
                        self.save_queue_state();
                    } else {
                        self.select(lib_idx);
                    }
                }
            }
            KeyCode::Enter => self.select(lib_idx),
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.toggle_watched(lib_idx)
            }
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.shuffle_play(lib_idx)
            }
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let name = self.libs[lib_idx].library.name.clone();
                self.ask_confirm(ConfirmModal {
                    title: " Rescan Library ".into(),
                    message: format!("Rescan '{name}'?"),
                    hint: "[y] Confirm    [Esc] Cancel".into(),
                    on_confirm: ConfirmAction::RescanLibrary(lib_idx),
                });
            }
            KeyCode::Char('r') => self.refresh_lib(lib_idx),
            // Any other Ctrl/Alt-modified character is claimed here as a
            // no-op. This mirrors the pre-phase-3 `is_lib_key` mirror's
            // broad catch-all in `handle_queue_key`'s left-panel
            // routing: unmapped Ctrl/Alt combos are swallowed while a
            // library sub-panel is focused, rather than leaking through to
            // an unrelated queue-view shortcut with the same bare key
            // (e.g. `Ctrl+z` must not trigger queue-undo while the library
            // panel has focus). Harmless at the other call site
            // (`handle_key_view_dispatch`), which already swallows any
            // unmatched key as the last entry in `CONTEXT_STACK`.
            KeyCode::Char(_)
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    || key.modifiers.contains(KeyModifiers::ALT) => {}
            _ => {
                return None;
            }
        }
        Some(false)
    }

    pub(super) fn adjust_volume(&mut self, delta: i64) {
        self.playback_target().adjust_volume(self, delta);
    }

    pub(super) fn handle_playback_key(&mut self, key: KeyEvent) -> Option<bool> {
        let snapshot = self.input_snapshot();
        if let Some(command) = super::action::idle_feed_command_for_key(
            super::input_resolver::KeyChord::from_key(key),
            snapshot.player_active,
            self.connected_session_id.is_some(),
            self.idle_feed_link_available(),
        ) {
            return Some(self.dispatch(command));
        }
        match super::input_resolver::resolve_key(
            super::input_resolver::InputContext::Playback,
            &snapshot,
            super::input_resolver::KeyChord::from_key(key),
        ) {
            super::input_resolver::KeyResolution::Command(cmd @ Command::TogglePlayPause) => {
                let now = Instant::now();
                let double_tap = self
                    .last_space_press
                    .is_some_and(|t| t.elapsed() < Duration::from_millis(300));
                self.last_space_press = Some(now);
                if double_tap {
                    self.last_space_press = None;
                    Some(self.dispatch(cmd))
                } else {
                    None
                }
            }
            super::input_resolver::KeyResolution::Command(cmd @ Command::Stop) => {
                let now = Instant::now();
                let double_tap = self
                    .last_esc_press
                    .is_some_and(|t| t.elapsed() < Duration::from_millis(300));
                self.last_esc_press = Some(now);
                if double_tap {
                    self.last_esc_press = None;
                    Some(self.dispatch(cmd))
                } else {
                    None
                }
            }
            super::input_resolver::KeyResolution::Command(cmd) => Some(self.dispatch(cmd)),
            // Swallow is unreachable for Playback today; both non-command outcomes
            // mean "not a playback key" → let it fall through (`None`).
            super::input_resolver::KeyResolution::FallThrough
            | super::input_resolver::KeyResolution::Swallow => None,
        }
    }

    /// Handle a key for the focused home list (all groups: CW + library latest).
    /// Returns true if the key was consumed (others fall through to focus nav).
    pub(super) fn handle_cw_key(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Up => {
                self.home_move_up();
                true
            }
            KeyCode::Down => {
                self.home_move_down();
                true
            }
            KeyCode::Char('[') if !ctrl => {
                self.home_move_section(-1);
                true
            }
            KeyCode::Char(']') if !ctrl => {
                self.home_move_section(1);
                true
            }
            KeyCode::PageUp => {
                self.home_move_cursor(-(self.cw_page() as i64));
                true
            }
            KeyCode::PageDown => {
                self.home_move_cursor(self.cw_page() as i64);
                true
            }
            KeyCode::Home => {
                self.home_select_start();
                true
            }
            KeyCode::End => {
                self.home_select_end();
                true
            }
            KeyCode::Enter if ctrl => {
                self.home_enqueue();
                true
            }
            KeyCode::Enter => {
                self.home_play();
                true
            }
            // Ctrl+a: enqueue (issue #209). Replaces the old Ctrl+q/Alt+q
            // bindings, which no longer enqueue — see
            // `handle_enqueue_selected_key`'s doc comment for why Ctrl+a
            // specifically had to become the enqueue key here.
            KeyCode::Char('a') if ctrl => {
                self.home_enqueue();
                true
            }
            KeyCode::Char('w') if ctrl => {
                self.cw_toggle_watched();
                true
            }
            KeyCode::Char('.') => {
                self.open_context_menu();
                true
            }
            KeyCode::Delete => {
                let cursor = self.home.home_cursor;
                let cw_len = self.home.continue_items.len();
                if cursor < cw_len {
                    let saved = self.home.continue_cursor;
                    self.home.continue_cursor = cursor;
                    self.remove_from_continue_watching();
                    self.home.continue_cursor = saved;
                }
                true
            }
            _ => false,
        }
    }

    fn cw_page(&self) -> usize {
        (self.layout.main.left_area.height as usize).max(1)
    }
}
