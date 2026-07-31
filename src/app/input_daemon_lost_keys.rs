use super::App;
use crossterm::event::{KeyCode, KeyEvent};

impl App {
    /// Dispatcher for the blocking daemon-lost modal (`daemon_lost_modal`,
    /// tasks 7.1-7.3): `[R]` restarts the local daemon and resumes the
    /// saved queue, `[S]` restarts without resuming (escapes a crash loop
    /// where resuming the offending item re-triggers the crash), `[Q]` quits
    /// like any other quit. A failed restart attempt stays on the modal with
    /// an updated diagnostic line rather than silently doing nothing.
    pub(super) fn handle_key_daemon_lost_modal(&mut self, key: KeyEvent) -> Option<bool> {
        self.daemon_lost_modal.as_ref()?;
        match key.code {
            KeyCode::Char('r') | KeyCode::Char('R') => {
                if let Err(e) = self.restart_local_daemon(true) {
                    self.set_daemon_lost_restart_error(e);
                }
                Some(false)
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                if let Err(e) = self.restart_local_daemon(false) {
                    self.set_daemon_lost_restart_error(e);
                }
                Some(false)
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.daemon_lost_modal = None;
                Some(self.try_quit())
            }
            _ => Some(false),
        }
    }

    fn set_daemon_lost_restart_error(&mut self, message: String) {
        if let Some(modal) = self.daemon_lost_modal.as_mut() {
            modal.restart_error = Some(message);
        }
    }
}
