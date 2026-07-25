use super::{App, PanelFocus, PendingQueueAction, QueueScope};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::player::PlayerCommand;

impl App {
    pub(super) fn handle_key_confirm_clear_queue(&mut self, key: KeyEvent) -> Option<bool> {
        if !self.confirm_clear_queue {
            return None;
        }
        self.confirm_clear_queue = false;
        if matches!(
            key.code,
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
        ) {
            self.replace_queue_or_prompt(PendingQueueAction::ClearQueue);
        } else {
            self.status.clear();
        }
        Some(false)
    }

    pub(super) fn handle_key_confirm_rescan(&mut self, key: KeyEvent) -> Option<bool> {
        if !self.confirm_rescan {
            return None;
        }
        self.confirm_rescan = false;
        let pending_lib_idx = self.pending_rescan_lib_idx.take();
        self.status.clear();
        if matches!(
            key.code,
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
        ) {
            let lib_idx = pending_lib_idx.unwrap_or_else(|| {
                if matches!(self.panel_focus, PanelFocus::Library) && self.library_tab > 0 {
                    self.library_tab - 1
                } else {
                    0
                }
            });
            self.trigger_lib_rescan(lib_idx);
        }
        Some(false)
    }

    pub(super) fn handle_key_confirm_skip_intro(&mut self, key: KeyEvent) -> Option<bool> {
        self.skip_intro_end_ticks?;
        if matches!(
            key.code,
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
        ) {
            if let Some(end_ticks) = self.skip_intro_end_ticks.take() {
                let secs = end_ticks as f64 / mbv_core::api::TICKS_PER_SECOND as f64;
                self.player.send_command(PlayerCommand::SeekAbsolute(secs));
                self.player.send_command(PlayerCommand::SkipIntroDismiss);
                self.status.clear();
            }
        } else {
            self.skip_intro_end_ticks = None;
            self.player.send_command(PlayerCommand::SkipIntroDismiss);
            self.status.clear();
        }
        Some(false)
    }

    pub(super) fn handle_key_confirm_next_up(&mut self, key: KeyEvent) -> Option<bool> {
        self.next_up_item.as_ref()?;
        if matches!(
            key.code,
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
        ) {
            if let Some(item) = self.next_up_item.take() {
                if let Some(idx) = self
                    .playback_queue()
                    .items
                    .iter()
                    .position(|i| i.id == item.id)
                {
                    let label = item.playback_label();
                    self.player.send_command(PlayerCommand::JumpTo(idx));
                    self.playback_queue_mut().queue_cursor = idx;
                    self.flash_status(label);
                }
            }
        } else {
            self.next_up_item = None;
            self.player.send_command(PlayerCommand::NextUpDismiss);
            self.status.clear();
        }
        Some(false)
    }

    pub(super) fn handle_key_clear_queue_prompt(&mut self, key: KeyEvent) -> Option<bool> {
        // Behavior change (phase 6, #135): gate on an open context menu. Before
        // this fix, `clear_queue_prompt_c` sat above `context_menu` in
        // CONTEXT_STACK with no guard, so pressing 'c' while a context menu was
        // open silently opened the clear-queue confirmation instead of being
        // swallowed by the menu (which has no 'c' binding of its own). See
        // docs/adr/0002-centralized-input-handling.md phase 6 and phase-2's
        // `home_search`, which already guards the same way.
        if key.code != KeyCode::Char('c')
            || key.modifiers.contains(KeyModifiers::ALT)
            || self.context_menu_open()
        {
            return None;
        }
        let in_lib_search = self.library_tab > 0
            && self
                .libs
                .get(self.library_tab - 1)
                .is_some_and(|l| l.search.is_some());
        if in_lib_search {
            return None;
        }
        if matches!(self.panel_focus, PanelFocus::Queue)
            && self.visible_queue_scope() == QueueScope::Remote
        {
            self.flash_status_high("Remote queue is controlled by the daemon".into());
            return Some(false);
        }
        if self.player_tab.items.is_empty() {
            return Some(false);
        }
        self.notify_with_actions(
            "mbv",
            "Clear queue?",
            &[("clear:yes", "Clear"), ("clear:no", "Cancel")],
        );
        self.status = "Clear queue? (Y/n)".into();
        self.confirm_clear_queue = true;
        Some(false)
    }
}
