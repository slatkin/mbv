use super::App;
use crossterm::event::KeyEvent;

impl App {
    /// The Feeds destination keyboard handler remains a catch-all while the
    /// component owns Feeds-local key handling.
    pub(super) fn handle_key_feeds(&mut self, _key: KeyEvent) -> Option<bool> {
        Some(false)
    }
}
