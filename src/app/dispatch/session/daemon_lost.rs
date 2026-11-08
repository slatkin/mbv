//! The daemon-lost recovery dialog's answers, shared by both Clients
//! (`daemon-disconnect-handling`: "Recovery options behave distinctly").
//!
//! Each Client's own handler owns only where the dialog lives — the TUI's
//! mounted component, the GUI's held overlay — and, for a failed restart,
//! where the diagnostic line goes. The answer's own effect is here, once, so
//! the GUI runs no recovery path of its own.

use crate::app::App;
use mbv_ui_msg::DaemonLostIntent;

/// What one answer did, so each Client's handler can update its own dialog
/// state: a failed restart carries the diagnostic for the dialog's
/// `Restart failed:` line, and the quit answer carries the Client's quit
/// decision back to its run loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::app) enum DaemonLostAnswer {
    /// A local daemon is up again and this Client is attached to it. The
    /// restart dismissed the dialog itself.
    Restarted,
    /// The restart failed: the dialog stays up with this diagnostic, so the
    /// user can try again or quit.
    RestartFailed(String),
    /// The user chose to quit; the Client ends its run on `true`.
    Quit(bool),
}

impl App {
    /// Apply one answer of the daemon-lost recovery dialog: either restart
    /// ensures a local daemon exists and attaches to it
    /// (`restart_local_daemon`, which dismisses the dialog on success), and
    /// Quit runs this Client's quit.
    pub(in crate::app) fn apply_daemon_lost_intent(
        &mut self,
        intent: DaemonLostIntent,
    ) -> DaemonLostAnswer {
        match intent {
            DaemonLostIntent::RestartWithTray | DaemonLostIntent::RestartWithoutTray => {
                match self.restart_local_daemon() {
                    Ok(()) => DaemonLostAnswer::Restarted,
                    Err(error) => DaemonLostAnswer::RestartFailed(error.to_string()),
                }
            }
            DaemonLostIntent::Quit => DaemonLostAnswer::Quit(self.try_quit()),
        }
    }
}
