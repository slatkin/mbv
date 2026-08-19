use super::notify_actions::ToastSeverity;
use super::{
    App, PanelFocus, PanelMode, QueueScope, SavePlaylistDialog, SavePlaylistStage,
    LEFT_WIDTH_DEFAULT, LEFT_WIDTH_STEP,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::{Duration, Instant};

/// How long a queue navigation gesture (Up/Down/click) holds the cursor
/// against background events that would snap it to the now-playing item.
const QUEUE_NAV_CURSOR_HOLD: Duration = Duration::from_millis(500);

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
            || self.effective_panel_mode() != PanelMode::Both
            || !Self::is_queue_column_width_resize_key(key)
        {
            return false;
        }

        let max_width = Self::queue_column_width_max_for_terminal(self.terminal_width);
        let next_width = if key.code == KeyCode::Left {
            self.queue_column_width.saturating_sub(LEFT_WIDTH_STEP)
        } else {
            self.queue_column_width.saturating_add(LEFT_WIDTH_STEP)
        };
        let normalized = Self::normalize_queue_column_width(next_width, self.terminal_width);
        if normalized == self.queue_column_width {
            let limit = if key.code == KeyCode::Left {
                format!("Queue column width already at minimum ({LEFT_WIDTH_DEFAULT} cols)")
            } else {
                format!("Queue column width already at maximum ({max_width} cols)")
            };
            self.flash(limit, ToastSeverity::Neutral);
            return true;
        }

        self.queue_column_width = normalized;
        self.save_prefs();
        self.flash(
            format!("Queue column width: {} cols", self.queue_column_width),
            ToastSeverity::Neutral,
        );
        true
    }

    pub(super) fn handle_queue_key(&mut self, key: KeyEvent) -> bool {
        // Queue-focused keys. `handle_key_view_dispatch` routes here only
        // when `panel_focus == Queue`; global, Alt, and browse-destination
        // keys are handled above it. Bracket keys own the queue panel's
        // Local/Remote scope switching, and PageUp/PageDown use the actual
        // queue panel height.
        if matches!(self.effective_panel_focus(), PanelFocus::Queue) {
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

        // Queue focus: PageUp/PageDown use the actual queue panel height.
        if matches!(self.effective_panel_focus(), PanelFocus::Queue) {
            let page = self.layout.main.queue_area.height.saturating_sub(1).max(1) as usize;
            match key.code {
                KeyCode::PageUp => {
                    self.mark_queue_cursor_user_active();
                    let queue = self.displayed_queue_mut();
                    queue.queue_cursor = queue.queue_cursor.saturating_sub(page);
                    return false;
                }
                KeyCode::PageDown => {
                    self.mark_queue_cursor_user_active();
                    let queue = self.displayed_queue_mut();
                    let n = queue.total_queue_len();
                    queue.queue_cursor = (queue.queue_cursor + page).min(n.saturating_sub(1));
                    return false;
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Char('t')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(self.effective_panel_focus(), PanelFocus::Queue)
                    && self.remote_tracker.is_some() =>
            {
                self.stop_remote_tracking();
            }
            KeyCode::Char('r')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(self.effective_panel_focus(), PanelFocus::Queue)
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
                self.mark_queue_cursor_user_active();
                self.displayed_queue_mut().queue_cursor -= 1;
            }
            KeyCode::Down
                if self.displayed_queue().queue_cursor + 1
                    < self.displayed_queue().total_queue_len() =>
            {
                self.mark_queue_cursor_user_active();
                self.displayed_queue_mut().queue_cursor += 1;
            }
            KeyCode::PageUp => {
                self.mark_queue_cursor_user_active();
                let p = self.queue_page_size();
                let queue = self.displayed_queue_mut();
                queue.queue_cursor = queue.queue_cursor.saturating_sub(p);
            }
            KeyCode::PageDown => {
                self.mark_queue_cursor_user_active();
                let p = self.queue_page_size();
                let queue = self.displayed_queue_mut();
                let n = queue.total_queue_len();
                queue.queue_cursor = (queue.queue_cursor + p).min(n.saturating_sub(1));
            }
            KeyCode::Home => {
                self.mark_queue_cursor_user_active();
                self.displayed_queue_mut().queue_cursor = 0;
            }
            KeyCode::End => {
                self.mark_queue_cursor_user_active();
                let n = self.displayed_queue().total_queue_len();
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
                if t < queue.total_queue_len() {
                    self.remove_from_queue(t);
                }
            }
            KeyCode::Char('z') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let scope = self.visible_queue_scope();
                if scope == QueueScope::Remote {
                    self.flash(
                        "Undo is not supported for remote queue edits".into(),
                        ToastSeverity::Error,
                    );
                    return false;
                }
                self.undo_last_queue_edit(scope);
            }
            KeyCode::Char('i') => {
                let queue = self.displayed_queue();
                let cursor = queue.queue_cursor;
                if let Some(item) = queue.emby_item_at(cursor) {
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
                    self.flash("Nothing is playing".into(), ToastSeverity::Error);
                }
            }
            KeyCode::Char('s')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                    && self.player_tab.total_queue_len() > 0 =>
            {
                self.save_playlist_dialog = Some(SavePlaylistDialog {
                    input: self.queue_playlist_name().to_string(),
                    stage: SavePlaylistStage::EnterName,
                });
            }
            _ => {}
        }
        false
    }

    /// Record that the user just navigated the queue, arming a short
    /// hold window during which background events must not snap the
    /// cursor to the now-playing item.
    pub(super) fn mark_queue_cursor_user_active(&mut self) {
        self.last_nav_at = Instant::now();
    }

    /// Whether a recent user navigation gesture should prevent a
    /// background event from overwriting `queue_cursor`.
    pub(super) fn queue_cursor_held_by_user(&self) -> bool {
        self.last_nav_at.elapsed() < QUEUE_NAV_CURSOR_HOLD
    }
}
