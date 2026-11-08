use crate::app::{App, SessionEvent};
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
}
