//! The confirm overlay's App-side answers (add-tauri-gui-client row 6.5):
//! the button press the canvas reports and the outside-press dismissal,
//! both resolved through the shared confirm dispatcher.

use mbv_ui_msg::{Key, KeyEvent, KeyModifiers};

use super::overlay::GuiOverlay;
use crate::app::App;

impl App {
    /// Execute the confirm overlay's button at `index` (the press on the
    /// region the front end reported): the same execution the button's key
    /// hint applies (`execute_gui_overlay_entry`'s confirm arm). Gated on
    /// the open overlay being the confirm — a stale press while another
    /// overlay or none holds the slot executes nothing.
    pub fn execute_gui_confirm_button(&mut self, index: usize) {
        if matches!(self.gui_overlay, Some(GuiOverlay::Confirm(_))) {
            self.execute_gui_overlay_entry(index);
        }
    }

    /// Dismiss the open overlay without executing an entry: the outside
    /// press the canvas and the TUI's blocking context menu both take
    /// (add-tauri-gui-client row 6.5). A confirm's outside press answers
    /// Esc — the same cancel the modal's own key applies — so a staged
    /// transfer the prompt guarded still unwinds.
    pub fn dismiss_open_gui_overlay(&mut self) {
        if let Some(GuiOverlay::Confirm(modal)) = self.gui_overlay.take() {
            self.apply_confirm_action(
                modal.on_confirm,
                KeyEvent::new(Key::Esc, KeyModifiers::NONE),
            );
        }
    }
}

/// The key chord a confirm button's `keys` hint names: `"Enter"`, `"Esc"`,
/// or a single character — the same mapping the TUI's Confirm component
/// resolves a click's pill hint through.
pub(super) fn confirm_button_key(hint: &str) -> Option<Key> {
    match hint {
        "Enter" => Some(Key::Enter),
        "Esc" => Some(Key::Esc),
        other => {
            let mut chars = other.chars();
            let ch = chars.next()?;
            chars.next().is_none().then_some(Key::Char(ch))
        }
    }
}
