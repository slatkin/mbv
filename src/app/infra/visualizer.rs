use super::super::App;
use mbv_visualizer::PipeWireWorker;

impl App {
    pub(in crate::app) fn sync_visualizer(&mut self) {
        if let Some(worker) = self.visualizer.as_ref() {
            match worker.take_latest_window() {
                Ok(Some(window)) => self.visualizer_window = window,
                Ok(None) => {}
                Err(error) => {
                    log::warn!(target: "visualizer", "PipeWire worker stopped; visualizer disabled for this playback: {error}");
                    self.visualizer_failed = true;
                    self.stop_visualizer_capture();
                }
            }
        }

        let should_run = self.visualizer_should_run();
        if !should_run {
            self.stop_visualizer_capture();
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
            && !self.visual_slot_hidden
    }

    pub(in crate::app) fn stop_visualizer_capture(&mut self) {
        if let Some(mut worker) = self.visualizer.take() {
            worker.stop();
        }
        self.visualizer_window = mbv_visualizer::StereoSampleWindow::default();
    }

    pub(in crate::app) fn toggle_visualizer(&mut self) {
        if self.visual_slot_hidden {
            return;
        }
        self.visualizer_enabled = !self.visualizer_enabled;
        self.visualizer_failed = false;
        if self.visualizer_enabled {
            self.sync_visualizer();
        } else {
            self.stop_visualizer_capture();
        }
        self.save_prefs();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn idle_playback_does_not_start_pipewire() {
        let mut app = crate::app::tests::make_app_stub();
        app.visualizer_enabled = true;

        app.sync_visualizer();

        assert!(!app.visualizer_should_run());
        assert!(app.visualizer.is_none());
    }

    #[test]
    fn hidden_slot_blocks_capture_when_visualizer_is_selected_and_playing() {
        let mut app = crate::app::tests::make_app_stub();
        app.visualizer_enabled = true;
        app.visual_slot_hidden = true;
        app.player.status.lock().unwrap().active = true;

        app.sync_visualizer();

        assert!(!app.visualizer_should_run());
        assert!(
            app.visualizer.is_none(),
            "hidden slot starts no capture worker"
        );
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
    fn selecting_artwork_stops_capture() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut app = crate::app::tests::make_app_stub();
        app.visualizer_enabled = true;
        app.visualizer_window.samples = vec![mbv_visualizer::StereoSample {
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
}
