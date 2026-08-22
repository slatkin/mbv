// Routes transport-key actions to the attached cast receiver (5.5), the
// `PlaybackTarget` arm alongside `playback_target_local.rs`/
// `playback_target_remote.rs`. Audio-track cycling and subtitles have no
// cast-protocol primitive in v1 (design.md Risks) and surface the standard
// "command not supported" flash instead of a client call.

use super::notify_actions::ToastSeverity;
use super::{App, CastPlaybackTarget};
use crate::app::render::indicators::IndicatorData;
use mbv_core::cast_client::CastPlaybackState;

impl CastPlaybackTarget {
    pub(super) fn toggle_play_pause(&self, app: &mut App) {
        let paused = app
            .cast_attachment
            .as_ref()
            .and_then(|a| a.status.as_ref())
            .is_some_and(|s| s.state == CastPlaybackState::Paused);
        if paused {
            app.send_cast_command(|t| t.play());
        } else {
            app.send_cast_command(|t| t.pause());
        }
    }

    pub(super) fn stop(&self, app: &mut App) {
        app.send_cast_command(|t| t.stop());
    }

    pub(super) fn seek_relative(&self, app: &mut App, delta: f64) {
        let position = app.cast_extrapolated_position_seconds().unwrap_or(0.0);
        let target = (position + delta as f32).max(0.0);
        app.send_cast_command(move |t| t.seek(target));
    }

    pub(super) fn jump_track(&self, app: &mut App, step: i64) {
        if step >= 0 {
            app.send_cast_command(|t| t.skip_next());
        } else {
            app.send_cast_command(|t| t.skip_previous());
        }
    }

    pub(super) fn toggle_command_mute(&self, app: &mut App) {
        let muted = !app.cast_attachment.as_ref().is_some_and(|a| a.muted);
        if let Some(attachment) = app.cast_attachment.as_mut() {
            attachment.muted = muted;
        }
        app.send_cast_command(move |t| t.set_muted(muted));
    }

    /// No local audio-track cycling exists for a cast target (`cycle_audio`
    /// below flashes "not supported"), so treating every item as audio
    /// routes the `a` key to mute-toggle, which cast does support.
    pub(super) fn is_audio_item(&self, _app: &App) -> bool {
        true
    }

    pub(super) fn toggle_soft_mute(&self, app: &mut App) {
        self.toggle_command_mute(app);
    }

    pub(super) fn cycle_audio(&self, app: &mut App) {
        app.flash(
            "Audio-track cycling is not supported for cast targets".to_string(),
            ToastSeverity::Warning,
        );
    }

    pub(super) fn adjust_volume(&self, app: &mut App, delta: i64) {
        let current = app
            .cast_attachment
            .as_ref()
            .map(|a| i64::from(a.volume))
            .unwrap_or(50);
        let new_volume = (current + delta).clamp(0, 100);
        if let Some(attachment) = app.cast_attachment.as_mut() {
            attachment.volume = new_volume as u8;
        }
        app.send_cast_command(move |t| t.set_volume(new_volume as f32 / 100.0));
    }

    pub(super) fn cycle_sub(&self, app: &mut App) {
        app.flash(
            "Subtitles are not supported for cast targets".to_string(),
            ToastSeverity::Warning,
        );
    }

    pub(super) fn displayed_volume(&self, app: &App) -> i64 {
        app.cast_attachment
            .as_ref()
            .map(|a| i64::from(a.volume))
            .unwrap_or(0)
    }

    pub(super) fn displayed_mute(&self, app: &App) -> bool {
        app.cast_attachment.as_ref().is_some_and(|a| a.muted)
    }

    pub(super) fn indicator_data(&self, _app: &App) -> Option<IndicatorData> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::make_app_stub;
    use crate::app::types_cast::{spawn_fake_cast_worker, FakeCastTransport};
    use std::sync::{Arc, Mutex};

    fn attached_app_with_fake_transport() -> (App, Arc<Mutex<Vec<String>>>) {
        let mut app = make_app_stub();
        app.attach_cast("device-1".to_string());
        let (job_tx, calls) = spawn_fake_cast_worker(FakeCastTransport::default());
        app.set_cast_client("device-1", job_tx);
        (app, calls)
    }

    #[test]
    fn toggle_play_pause_sends_pause_while_playing() {
        let (mut app, calls) = attached_app_with_fake_transport();
        CastPlaybackTarget.toggle_play_pause(&mut app);
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(calls.lock().unwrap().contains(&"pause".to_string()));
    }

    #[test]
    fn toggle_play_pause_sends_play_while_paused() {
        use mbv_core::cast_client::CastStatus;
        let (mut app, calls) = attached_app_with_fake_transport();
        app.cast_attachment.as_mut().unwrap().status = Some(CastStatus {
            position_seconds: Some(0.0),
            duration_seconds: None,
            playback_rate: 1.0,
            state: CastPlaybackState::Paused,
            playing_content_id: None,
        });
        CastPlaybackTarget.toggle_play_pause(&mut app);
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(calls.lock().unwrap().contains(&"play".to_string()));
    }

    #[test]
    fn stop_sends_stop() {
        let (mut app, calls) = attached_app_with_fake_transport();
        CastPlaybackTarget.stop(&mut app);
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(calls.lock().unwrap().contains(&"stop".to_string()));
    }

    #[test]
    fn seek_relative_sends_seek_from_the_extrapolated_position() {
        let (mut app, calls) = attached_app_with_fake_transport();
        CastPlaybackTarget.seek_relative(&mut app, 5.0);
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(calls.lock().unwrap().iter().any(|c| c.starts_with("seek(")));
    }

    #[test]
    fn detach_issues_no_stop() {
        let (mut app, calls) = attached_app_with_fake_transport();
        app.detach_cast();
        assert!(app.cast_attachment.is_none());
        assert!(!calls.lock().unwrap().contains(&"stop".to_string()));
    }

    #[test]
    fn jump_track_routes_to_skip_next_and_skip_previous() {
        let (mut app, calls) = attached_app_with_fake_transport();
        CastPlaybackTarget.jump_track(&mut app, 1);
        std::thread::sleep(std::time::Duration::from_millis(50));
        CastPlaybackTarget.jump_track(&mut app, -1);
        std::thread::sleep(std::time::Duration::from_millis(50));
        let calls = calls.lock().unwrap().clone();
        assert!(calls.contains(&"skip_next".to_string()));
        assert!(calls.contains(&"skip_previous".to_string()));
    }

    #[test]
    fn toggle_command_mute_sends_set_muted_and_updates_displayed_mute() {
        let (mut app, calls) = attached_app_with_fake_transport();
        assert!(!CastPlaybackTarget.displayed_mute(&app));
        CastPlaybackTarget.toggle_command_mute(&mut app);
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(CastPlaybackTarget.displayed_mute(&app));
        assert!(calls
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.starts_with("set_muted(true)")));
    }

    #[test]
    fn adjust_volume_sends_set_volume_and_updates_displayed_volume() {
        let (mut app, calls) = attached_app_with_fake_transport();
        app.cast_attachment.as_mut().unwrap().volume = 50;
        CastPlaybackTarget.adjust_volume(&mut app, 10);
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert_eq!(CastPlaybackTarget.displayed_volume(&app), 60);
        assert!(calls
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.starts_with("set_volume(")));
    }

    #[test]
    fn cycle_audio_and_cycle_sub_flash_unsupported_without_a_client_call() {
        let (mut app, calls) = attached_app_with_fake_transport();
        CastPlaybackTarget.cycle_audio(&mut app);
        assert!(app.status.contains("not supported"));
        CastPlaybackTarget.cycle_sub(&mut app);
        assert!(app.status.contains("not supported"));
        assert!(calls.lock().unwrap().is_empty());
    }
}
