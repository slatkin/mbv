use super::App;
use mbv_core::visualizer::CavaWorker;

impl App {
    pub(super) fn sync_visualizer(&mut self) {
        if let Some(worker) = self.visualizer.as_ref() {
            match worker.take_latest_frame() {
                Ok(Some(frame)) => self.visualizer_frame = frame,
                Ok(None) => {}
                Err(()) => {
                    log::warn!(target: "visualizer", "CAVA worker stopped; visualizer disabled for this playback");
                    self.visualizer_failed = true;
                    self.stop_visualizer_worker();
                }
            }
        }

        let is_local = !self.player.is_remote() && self.connected_session_id.is_none();
        let audio_pipe_enabled = self.client.lock().unwrap().config.audio_pipe_enabled;
        let active = self.player.status.lock().unwrap().active;

        let should_run = self.visualizer_enabled && is_local && active && !audio_pipe_enabled;
        if !should_run {
            self.stop_visualizer_worker();
            return;
        }
        if self.visualizer.is_none() && !self.visualizer_failed {
            match CavaWorker::start() {
                Ok(worker) => {
                    log::info!(target: "visualizer", "started CAVA system-audio worker");
                    self.visualizer = Some(worker);
                }
                Err(error) => {
                    log::warn!(target: "visualizer", "system-audio visualizer unavailable: {error}");
                    self.visualizer_failed = true;
                }
            }
        }
    }

    pub(super) fn stop_visualizer_worker(&mut self) {
        if let Some(mut worker) = self.visualizer.take() {
            worker.stop();
        }
        self.visualizer_frame.clear();
    }

    pub(super) fn toggle_visualizer(&mut self) {
        self.visualizer_enabled = !self.visualizer_enabled;
        self.visualizer_failed = false;
        if !self.visualizer_enabled {
            self.stop_visualizer_worker();
        } else {
            self.sync_visualizer();
        }
        self.save_prefs();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn audio_pipe_playback_does_not_start_cava() {
        let mut app = crate::app::tests::make_app_stub();
        app.visualizer_enabled = true;
        app.player.status.lock().unwrap().active = true;
        app.client.lock().unwrap().config.audio_pipe_enabled = true;
        app.sync_visualizer();
        assert!(app.visualizer.is_none());
    }

    #[test]
    fn remote_playback_does_not_start_local_cava() {
        let mut app = crate::app::tests::make_remote_app_stub(Vec::new(), Vec::new());
        app.visualizer_enabled = true;
        app.sync_visualizer();
        assert!(app.visualizer.is_none());
    }

    #[test]
    fn attached_remote_session_visualizer_key_is_noop() {
        let (mut app, cmd_rx) =
            crate::app::tests::make_remote_app_stub_with_cmd_rx(Vec::new(), Vec::new());
        app.connected_session_id = Some("session-1".to_string());
        app.visualizer_enabled = false;

        let handled = app.handle_key_visualizer(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('v'),
            crossterm::event::KeyModifiers::NONE,
        ));

        assert_eq!(handled, Some(false));
        assert!(!app.visualizer_enabled);
        assert!(cmd_rx.try_recv().is_err());
    }

    #[test]
    fn local_visualizer_key_still_toggles() {
        let mut app = crate::app::tests::make_app_stub();
        app.visualizer_enabled = false;

        let handled = app.handle_key_visualizer(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('v'),
            crossterm::event::KeyModifiers::NONE,
        ));

        assert_eq!(handled, Some(false));
        assert!(app.visualizer_enabled);
    }

    #[test]
    fn new_playback_clears_visualizer_failure() {
        let mut app = crate::app::tests::make_app_stub();
        app.visualizer_failed = true;

        app.handle_player_event(mbv_core::player::PlayerEvent::TrackChanged(0));

        assert!(!app.visualizer_failed);
    }
}
