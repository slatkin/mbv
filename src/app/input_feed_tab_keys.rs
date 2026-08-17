use super::{App, PanelFocus};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl App {
    /// The Feeds destination keyboard handler called from the exhaustive
    /// browse dispatch (`handle_key_browse_dispatch`). Delegates to
    /// `handle_feed_tab_key` for the Feeds key set (refresh, watched filter,
    /// cursor navigation, group cycling, play, enqueue) and consumes every
    /// other key so Emby-only actions and queue-item handling are
    /// unreachable while Feeds is focused.
    pub(super) fn handle_key_feeds(&mut self, key: KeyEvent) -> Option<bool> {
        if let Some(quit) = self.handle_feed_tab_key(key) {
            return Some(quit);
        }
        Some(false)
    }

    /// Handle key events while the Feeds tab is active and the library
    /// panel has focus. Returns `Some(quit)` if the key was consumed, or
    /// `None` to fall through to the global view keys.
    pub(super) fn handle_feed_tab_key(&mut self, key: KeyEvent) -> Option<bool> {
        // Allow Tab/BackTab to cycle tabs via `handle_global_view_key`.
        if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
            return None;
        }
        if !matches!(self.effective_panel_focus(), PanelFocus::Library) {
            return None;
        }
        if !self.tab.is_feeds() {
            return None;
        }

        match key.code {
            // Manual refresh.
            KeyCode::Char('r') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.refresh_feeds();
                Some(false)
            }
            // Watched filter cycle (unmodified w only).
            KeyCode::Char('w') if key.modifiers.is_empty() => {
                self.feed_tab.cycle_watched_filter();
                Some(false)
            }
            // Cursor navigation. Up/Down (and j/k) move by display row,
            // resolved through the packed layout so a two-column list moves
            // visually down/up. Left/Right (and h/l) move across cells in
            // 2-col mode, mirroring the Emby library list's arrow-key
            // column navigation.
            KeyCode::Up => {
                self.feed_tab_move_cursor_rows(-1);
                Some(false)
            }
            KeyCode::Down => {
                self.feed_tab_move_cursor_rows(1);
                Some(false)
            }
            KeyCode::Char('k') => {
                self.feed_tab_move_cursor_rows(-1);
                Some(false)
            }
            KeyCode::Char('j') => {
                self.feed_tab_move_cursor_rows(1);
                Some(false)
            }
            KeyCode::Left
                if crate::app::library_column_width::library_column_count(
                    self.layout.main.left_area.width,
                ) > 1 =>
            {
                self.feed_tab_move_cursor(-1);
                Some(false)
            }
            KeyCode::Right
                if crate::app::library_column_width::library_column_count(
                    self.layout.main.left_area.width,
                ) > 1 =>
            {
                self.feed_tab_move_cursor(1);
                Some(false)
            }
            KeyCode::Char('h')
                if crate::app::library_column_width::library_column_count(
                    self.layout.main.left_area.width,
                ) > 1 =>
            {
                self.feed_tab_move_cursor(-1);
                Some(false)
            }
            KeyCode::Char('l')
                if crate::app::library_column_width::library_column_count(
                    self.layout.main.left_area.width,
                ) > 1 =>
            {
                self.feed_tab_move_cursor(1);
                Some(false)
            }
            KeyCode::PageUp => {
                let page = self.layout.main.left_area.height.saturating_sub(1).max(1) as usize;
                self.feed_tab_page_cursor(page, false);
                Some(false)
            }
            KeyCode::PageDown => {
                let page = self.layout.main.left_area.height.saturating_sub(1).max(1) as usize;
                self.feed_tab_page_cursor(page, true);
                Some(false)
            }
            KeyCode::Home => {
                self.feed_tab_jump_cursor(false);
                Some(false)
            }
            KeyCode::End => {
                self.feed_tab_jump_cursor(true);
                Some(false)
            }
            // Group cycling.
            KeyCode::Char('[') => {
                self.feed_tab_cycle_group(-1);
                Some(false)
            }
            KeyCode::Char(']') => {
                self.feed_tab_cycle_group(1);
                Some(false)
            }
            // Enter — play the selected entry (§5.5/§5.6). Consumed
            // unconditionally so it never falls through to the global/
            // library-view Enter handling of Emby queue actions.
            KeyCode::Enter => {
                use super::notify_actions::ToastSeverity;
                if self.feed_tab.visible_entries().is_empty() {
                    self.flash("No entries to play".into(), ToastSeverity::Neutral);
                } else {
                    self.feed_tab_play_selected();
                }
                Some(false)
            }
            // e — enqueue the selected entry into the canonical queue
            // without starting playback, using the same append path as
            // library enqueue.
            KeyCode::Char('e') => {
                use super::notify_actions::ToastSeverity;
                if self.feed_tab.visible_entries().is_empty() {
                    self.flash("No entries to enqueue".into(), ToastSeverity::Neutral);
                } else {
                    self.feed_tab_enqueue_selected();
                }
                Some(false)
            }
            _ => None,
        }
    }
}
