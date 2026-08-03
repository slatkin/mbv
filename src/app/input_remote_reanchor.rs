use super::App;
use crossterm::event::{KeyCode, KeyEvent};

impl App {
    pub(super) fn handle_key_remote_reanchor(&mut self, key: KeyEvent) -> Option<bool> {
        self.remote_reanchor_popup.as_ref()?;
        match key.code {
            KeyCode::Esc => {
                self.remote_reanchor_popup = None;
            }
            KeyCode::Up => {
                if let Some(popup) = &mut self.remote_reanchor_popup {
                    popup.cursor = popup.cursor.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if let Some(popup) = &mut self.remote_reanchor_popup {
                    if popup.cursor + 1 < popup.targets.len() {
                        popup.cursor += 1;
                    }
                }
            }
            KeyCode::Enter => self.select_remote_reanchor_target(),
            _ => {}
        }
        Some(false)
    }
}
