use super::App;
use mbv_core::ctrl::CtrlCmd;
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

        if self.visualizer_enabled
            && self.player.is_remote()
            && active
            && self.player.supports_spectrum()
        {
            if !self.spectrum_started {
                let _ = self.player.send_ctrl_cmd(CtrlCmd::StartSpectrum);
                self.spectrum_started = true;
            }
            return;
        }

        let should_run = self.visualizer_enabled && is_local && active && !audio_pipe_enabled;
        if !should_run {
            self.stop_visualizer_worker();
            self.visualizer_failed = false;
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
        if self.player.is_remote() {
            let _ = self.player.send_ctrl_cmd(CtrlCmd::StopSpectrum);
        }
        self.spectrum_started = false;
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
    fn daemon_visualizer_toggle_sends_start_spectrum() {
        let (mut app, cmd_rx) =
            crate::app::tests::make_remote_app_stub_with_cmd_rx(Vec::new(), Vec::new());
        app.visualizer_enabled = false;
        app.player.status.lock().unwrap().active = true;
        app.toggle_visualizer();
        assert!(app.visualizer_enabled);
        let cmd = cmd_rx.try_recv().unwrap();
        assert!(matches!(cmd, mbv_core::ctrl::CtrlCmd::StartSpectrum));
    }

    #[test]
    fn daemon_visualizer_toggle_off_sends_stop_spectrum() {
        let (mut app, cmd_rx) =
            crate::app::tests::make_remote_app_stub_with_cmd_rx(Vec::new(), Vec::new());
        app.visualizer_enabled = true;
        app.toggle_visualizer();
        assert!(!app.visualizer_enabled);
        let cmd = cmd_rx.try_recv().unwrap();
        assert!(matches!(cmd, mbv_core::ctrl::CtrlCmd::StopSpectrum));
    }

    #[test]
    fn stop_visualizer_worker_sends_stop_spectrum_for_remote() {
        let (mut app, cmd_rx) =
            crate::app::tests::make_remote_app_stub_with_cmd_rx(Vec::new(), Vec::new());
        app.stop_visualizer_worker();
        let cmd = cmd_rx.try_recv().unwrap();
        assert!(matches!(cmd, mbv_core::ctrl::CtrlCmd::StopSpectrum));
    }

    #[test]
    fn stop_visualizer_worker_does_not_send_stop_spectrum_for_local() {
        let mut app = crate::app::tests::make_app_stub();
        app.stop_visualizer_worker();
        // No ctrl channel, so send_ctrl_cmd returns false — no panic
    }

    #[test]
    fn daemon_without_spectrum_support_cannot_toggle_visualizer() {
        let (mut app, _cmd_rx) =
            crate::app::tests::make_v2_remote_app_stub_with_cmd_rx(Vec::new(), Vec::new());
        app.visualizer_enabled = false;
        app.player.status.lock().unwrap().active = true;
        app.handle_key_visualizer(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('v'),
            crossterm::event::KeyModifiers::NONE,
        ));
        assert!(!app.visualizer_enabled);
    }

    #[test]
    fn spectrum_event_writes_frame() {
        let mut app = crate::app::tests::make_app_stub();
        let bars = vec![0.5; 64];
        let ev = mbv_core::player::PlayerEvent::Spectrum(bars.clone());
        app.handle_player_event(ev);
        assert_eq!(app.visualizer_frame, bars);
    }

    #[test]
    fn spectrum_failed_sets_visualizer_failed() {
        let mut app = crate::app::tests::make_app_stub();
        let ev = mbv_core::player::PlayerEvent::SpectrumFailed("cava not found".to_string());
        app.handle_player_event(ev);
        assert!(app.visualizer_failed);
    }
}
