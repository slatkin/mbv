use super::visualizer_worker::PipeWireWorker;
use super::App;

impl App {
    pub(super) fn sync_visualizer(&mut self) {
        if let Some(worker) = self.visualizer.as_ref() {
            match worker.take_latest_window() {
                Ok(Some(window)) => self.visualizer_window = window,
                Ok(None) => {}
                Err(error) => {
                    log::warn!(target: "visualizer", "PipeWire worker stopped; visualizer disabled for this playback: {error}");
                    self.visualizer_failed = true;
                    self.stop_visualizer_worker();
                }
            }
        }

        let should_run = self.visualizer_should_run();
        if !should_run {
            self.stop_visualizer_worker();
            return;
        }
        if self.visualizer.is_none() && !self.visualizer_failed {
            match PipeWireWorker::start() {
                Ok(worker) => {
                    log::info!(target: "visualizer", "started PipeWire system-audio worker");
                    self.visualizer = Some(worker);
                }
                Err(error) => {
                    log::warn!(target: "visualizer", "system-audio visualizer unavailable: {error}");
                    self.visualizer_failed = true;
                }
            }
        }
    }

    fn visualizer_should_run(&self) -> bool {
        let audio_pipe_enabled = self.config.lock().unwrap().audio_pipe_enabled;
        let active = self.player.status.lock().unwrap().active;
        self.visualizer_enabled
            && self.connected_session_id.is_none()
            && !self.is_cast_attached()
            && active
            && !audio_pipe_enabled
    }

    pub(super) fn stop_visualizer_worker(&mut self) {
        if let Some(mut worker) = self.visualizer.take() {
            worker.stop();
        }
        self.visualizer_window = Default::default();
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
    fn audio_pipe_playback_does_not_start_pipewire() {
        let mut app = crate::app::tests::make_app_stub();
        app.visualizer_enabled = true;
        app.player.status.lock().unwrap().active = true;
        app.config.lock().unwrap().audio_pipe_enabled = true;
        app.sync_visualizer();
        assert!(app.visualizer.is_none());
    }

    #[test]
    fn direct_remote_playback_allows_local_pipewire() {
        let mut app = crate::app::tests::make_remote_app_stub(Vec::new(), Vec::new());
        app.visualizer_enabled = true;
        app.player.status.lock().unwrap().active = true;

        assert!(app.visualizer_should_run());
    }

    #[test]
    fn attached_cast_target_blocks_the_visualizer_gate() {
        let mut app = crate::app::tests::make_app_stub();
        app.visualizer_enabled = true;
        app.player.status.lock().unwrap().active = true;
        assert!(app.visualizer_should_run());

        app.attach_cast("device-1".to_string());

        assert!(!app.visualizer_should_run());
    }

    #[test]
    fn detaching_a_cast_target_restores_the_gate() {
        let mut app = crate::app::tests::make_app_stub();
        app.visualizer_enabled = true;
        app.player.status.lock().unwrap().active = true;
        app.attach_cast("device-1".to_string());
        assert!(!app.visualizer_should_run());

        app.detach_cast();

        assert!(app.visualizer_should_run());
    }

    #[test]
    fn attached_session_v_key_toggles_selection_without_local_capture() {
        let _guard = crate::config::TestStateDirGuard::new();
        let (mut app, cmd_rx) =
            crate::app::tests::make_remote_app_stub_with_cmd_rx(Vec::new(), Vec::new());
        app.connected_session_id = Some("session-1".to_string());
        app.visualizer_enabled = false;
        app.player.status.lock().unwrap().active = true;

        let handled = app.handle_key_visualizer(
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('v'),
                crossterm::event::KeyModifiers::NONE,
            ),
            false,
        );

        assert_eq!(handled, Some(false));
        assert!(
            app.visualizer_enabled,
            "v flips artwork/visualizer selection even in an attached session"
        );
        assert!(cmd_rx.try_recv().is_err());
        assert!(!app.visualizer_should_run());
        app.sync_visualizer();
        assert!(
            app.visualizer.is_none(),
            "attached-session playback must not start a local PipeWire capture"
        );
    }

    #[test]
    fn local_visualizer_key_still_toggles() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut app = crate::app::tests::make_app_stub();
        app.visualizer_enabled = false;

        let handled = app.handle_key_visualizer(
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('v'),
                crossterm::event::KeyModifiers::NONE,
            ),
            false,
        );

        assert_eq!(handled, Some(false));
        assert!(app.visualizer_enabled);
    }

    #[test]
    fn selecting_artwork_stops_capture() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut app = crate::app::tests::make_app_stub();
        app.visualizer_enabled = true;
        app.visualizer_window.samples = vec![crate::app::visualizer_worker::StereoSample {
            left: 1.0,
            right: 1.0,
        }];

        app.toggle_visualizer();

        assert!(!app.visualizer_enabled);
        assert!(
            app.visualizer_window.samples.is_empty(),
            "selecting artwork must tear down the capture sample window"
        );
    }

    #[test]
    fn toggle_visualizer_does_not_persist_selection() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut app = crate::app::tests::make_app_stub();
        app.visualizer_enabled = false;

        app.toggle_visualizer();

        let prefs: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(crate::config::prefs_path()).expect("prefs written"),
        )
        .expect("prefs json");
        assert!(
            prefs.get("visualizer_enabled").is_none(),
            "visualizer selection must stay session-local"
        );
    }

    #[test]
    fn build_starts_on_artwork_even_with_saved_visualizer_pref() {
        let _guard = crate::config::TestStateDirGuard::new();
        std::fs::write(
            crate::config::prefs_path(),
            serde_json::json!({ "visualizer_enabled": true }).to_string(),
        )
        .expect("write prefs");

        let app = crate::app::tests::make_built_app();

        assert!(
            !app.visualizer_enabled,
            "every launch must default to artwork, ignoring the stale key"
        );
    }

    #[test]
    fn new_playback_clears_visualizer_failure() {
        let mut app = crate::app::tests::make_app_stub();
        app.visualizer_failed = true;

        app.handle_player_event(mbv_core::player::PlayerEvent::TrackChanged(0));

        assert!(!app.visualizer_failed);
    }
}
