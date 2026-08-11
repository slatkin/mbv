use super::types_feeds_manage::{FeedForm, FeedFormField, FeedsManageStage};
use super::App;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl App {
    /// Key routing for the feeds management overlay (§6.1), nested inside
    /// `handle_key_settings` the same way `multiselect_popup` and
    /// `library_routes_popup` are.
    pub(super) fn handle_key_feeds_manage(&mut self, key: KeyEvent) -> Option<bool> {
        let stage = self.feeds_manage_popup.as_ref()?.stage.clone();
        match stage {
            FeedsManageStage::List => self.handle_feeds_manage_list_key(key),
            FeedsManageStage::Form(form) => self.handle_feeds_manage_form_key(key, form),
        }
        Some(false)
    }

    fn handle_feeds_manage_list_key(&mut self, key: KeyEvent) {
        let count = self.config.lock().unwrap().feeds.len();
        match key.code {
            KeyCode::Esc => self.feeds_manage_popup = None,
            KeyCode::Up => {
                if let Some(popup) = &mut self.feeds_manage_popup {
                    popup.cursor = popup.cursor.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if count > 0 {
                    if let Some(popup) = &mut self.feeds_manage_popup {
                        popup.cursor = (popup.cursor + 1).min(count - 1);
                    }
                }
            }
            KeyCode::Char('a') => self.start_add_feed(),
            KeyCode::Enter | KeyCode::Char('e') if count > 0 => self.start_edit_feed(),
            KeyCode::Char('d') if count > 0 => self.confirm_remove_feed(),
            _ => {}
        }
    }

    fn handle_feeds_manage_form_key(&mut self, key: KeyEvent, form: FeedForm) {
        let submitting = self
            .feeds_manage_popup
            .as_ref()
            .map(|p| p.pending_add.is_some())
            .unwrap_or(false);
        match key.code {
            KeyCode::Esc => self.cancel_feed_form(),
            KeyCode::Tab if !submitting => self.feed_form_next_field(),
            KeyCode::BackTab if !submitting => self.feed_form_prev_field(),
            KeyCode::Enter if !submitting => self.submit_feed_form(),
            KeyCode::Left | KeyCode::Right if !submitting && form.focus == FeedFormField::Kind => {
                self.toggle_feed_form_kind();
            }
            KeyCode::Backspace if !submitting => self.feed_form_backspace(),
            KeyCode::Char(c)
                if !submitting
                    && (key.modifiers == KeyModifiers::NONE
                        || key.modifiers == KeyModifiers::SHIFT) =>
            {
                self.feed_form_push_char(c);
            }
            _ => {}
        }
    }
}
