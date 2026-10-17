use crate::app::{App, SessionEvent};
use mbv_emby::EmbyClient;
use mbv_emby_model::TICKS_PER_SECOND;
impl App {
    /// Advances whenever queue identity or state is replaced, so in-flight
    /// playlist mutations can detect stale completions.
    pub(in crate::app) fn advance_queue_epoch(&mut self) {
        self.queue_epoch.advance();
    }

    pub(in crate::app) fn spawn_sessions_load(&mut self) {
        self.sessions_loading = true;
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.channels.sessions_tx.clone();
        std::thread::spawn(move || match client.get_sessions() {
            Ok(sessions) => {
                let _ = tx.send(SessionEvent::Loaded { sessions });
            }
            Err(e) => {
                let _ = tx.send(SessionEvent::Error(e.to_string()));
            }
        });
    }

    /// Compute the absolute tick position for a remote-session seek, given
    /// the current position in seconds and a relative delta in seconds.
    ///
    /// This reconstructs the asymmetric math the old inline remote-session
    /// `<`/`>` handlers in `input.rs` had: rewinding (`delta < 0`) clamps at
    /// zero, fast-forwarding does not (matching the prior
    /// `(pos_s - 5).max(0)` vs. `(pos_s + 5)`). Used by `action::dispatch`'s
    /// `Action::SeekRelative` arm; kept here rather than in `action.rs`,
    /// since it's pure session-position math with no dependency on the
    /// `Action` seam itself.
    pub(in crate::app) fn remote_seek_ticks(pos_s: i64, delta: f64) -> i64 {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "fractional seconds in the seek delta truncate to whole seconds; sub-second seek precision is not representable here (approved, issue #804)"
        )]
        let moved = pos_s + delta as i64;
        let target = if delta < 0.0 { moved.max(0) } else { moved };
        target * TICKS_PER_SECOND
    }

    pub(in crate::app) fn do_session_command(
        &mut self,
        f: impl FnOnce(&EmbyClient) -> Result<(), mbv_emby::EmbyError> + Send + 'static,
    ) {
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.channels.sessions_tx.clone();
        std::thread::spawn(move || {
            if let Err(e) = f(&client) {
                let _ = tx.send(SessionEvent::CommandError {
                    error: e.to_string(),
                });
                return;
            }
            // Refresh the directly observed Session state after a successful command.
            match client.get_sessions() {
                Ok(sessions) => {
                    let _ = tx.send(SessionEvent::Loaded { sessions });
                }
                Err(e) => {
                    let _ = tx.send(SessionEvent::Error(e.to_string()));
                }
            }
        });
    }
}
