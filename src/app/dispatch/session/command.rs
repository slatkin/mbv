use crate::app::{App, SessionEvent};
impl App {
    /// Advances whenever queue identity or state is replaced, so in-flight
    /// playlist mutations can detect stale completions.
    pub(in crate::app) fn advance_queue_epoch(&mut self) {
        self.queue_epoch.advance();
    }

    /// The GUI Client's minimal Emby fill (add-tauri-gui-client design D9's
    /// ruled exception) and the discovery it feeds. A cached client that
    /// arrived with the attach (`owner_link`'s `cached_emby_client`) needs
    /// no worker: the sessions and cast loads start directly. Otherwise the
    /// armed startup request becomes an authentication-only worker — no
    /// library bootstrap `spawn_startup_services` runs — whose completion
    /// installs the client and starts the same two loads. A no-op while a
    /// fill is in flight or no request is armed.
    pub fn start_emby_client_fill(&mut self) {
        if self.emby_runtime.client.is_some() {
            self.spawn_sessions_load();
            self.spawn_cast_discovery();
            return;
        }
        if self.setup.emby_client_fill_rx.is_some() {
            return;
        }
        let Some(request) = self.setup.emby_startup_request.take() else {
            return;
        };
        self.setup.emby_client_fill_rx = Some(
            crate::app::dispatch::session::service_startup::start_client_fill(
                request.config,
                request.generation,
            ),
        );
    }

    /// Whether the GUI Client's minimal Emby fill is armed and unanswered:
    /// the fact a Client's test reads after construction, before any
    /// completion drains.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn emby_client_fill_pending_for_test(&self) -> bool {
        self.setup.emby_client_fill_rx.is_some()
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
