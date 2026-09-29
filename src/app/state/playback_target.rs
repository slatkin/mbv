mod cast;
mod local;
mod remote;

use crate::app::App;
use mbv_render::indicators::IndicatorData;
use mbv_ui_model::playback_target::NowPlayingStatus;

#[derive(Clone, Copy)]
pub(in crate::app) struct LocalPlaybackTarget;

#[derive(Clone)]
pub(in crate::app) struct RemotePlaybackTarget {
    pub session_id: String,
}

#[derive(Clone, Copy)]
pub(in crate::app) struct CastPlaybackTarget;

#[derive(Clone)]
pub(in crate::app) enum PlaybackTarget {
    Local(LocalPlaybackTarget),
    Remote(RemotePlaybackTarget),
    Cast(CastPlaybackTarget),
}

impl PlaybackTarget {
    pub(in crate::app) fn toggle_play_pause(&self, app: &mut App) {
        match self {
            Self::Local(_) => LocalPlaybackTarget::toggle_play_pause(app),
            Self::Remote(target) => target.toggle_play_pause(app),
            Self::Cast(_) => CastPlaybackTarget::toggle_play_pause(app),
        }
    }

    pub(in crate::app) fn stop(&self, app: &mut App) {
        match self {
            Self::Local(_) => LocalPlaybackTarget::stop(app),
            Self::Remote(target) => target.stop(app),
            Self::Cast(_) => CastPlaybackTarget::stop(app),
        }
    }

    pub(in crate::app) fn seek_relative(&self, app: &mut App, delta: f64) {
        match self {
            Self::Local(_) => LocalPlaybackTarget::seek_relative(app, delta),
            Self::Remote(target) => target.seek_relative(app, delta),
            Self::Cast(_) => CastPlaybackTarget::seek_relative(app, delta),
        }
    }

    pub(in crate::app) fn jump_track(&self, app: &mut App, step: i64, transport: &'static str) {
        match self {
            Self::Local(_) => LocalPlaybackTarget::jump_track(app, step),
            Self::Remote(target) => target.jump_track(app, step, transport),
            Self::Cast(_) => CastPlaybackTarget::jump_track(app, step),
        }
    }

    pub(in crate::app) fn toggle_command_mute(&self, app: &mut App) {
        match self {
            Self::Local(_) => LocalPlaybackTarget::toggle_command_mute(app),
            Self::Remote(_) => RemotePlaybackTarget::toggle_command_mute(app),
            Self::Cast(_) => CastPlaybackTarget::toggle_command_mute(app),
        }
    }

    pub(in crate::app) fn is_audio_item(&self, app: &App) -> bool {
        match self {
            Self::Local(_) => LocalPlaybackTarget::is_audio_item(app),
            Self::Remote(_) => RemotePlaybackTarget::is_audio_item(app),
            Self::Cast(_) => CastPlaybackTarget::is_audio_item(app),
        }
    }

    pub(in crate::app) fn toggle_soft_mute(&self, app: &mut App) {
        match self {
            Self::Local(_) => LocalPlaybackTarget::toggle_soft_mute(app),
            Self::Remote(target) => target.toggle_soft_mute(app),
            Self::Cast(_) => CastPlaybackTarget::toggle_soft_mute(app),
        }
    }

    pub(in crate::app) fn cycle_audio(&self, app: &mut App) {
        match self {
            Self::Local(_) => LocalPlaybackTarget::cycle_audio(app),
            Self::Remote(target) => target.cycle_audio(app),
            Self::Cast(_) => CastPlaybackTarget::cycle_audio(app),
        }
    }

    pub(in crate::app) fn adjust_volume(&self, app: &mut App, delta: i64) {
        match self {
            Self::Local(_) => LocalPlaybackTarget::adjust_volume(app, delta),
            Self::Remote(target) => target.adjust_volume(app, delta),
            Self::Cast(_) => CastPlaybackTarget::adjust_volume(app, delta),
        }
    }

    pub(in crate::app) fn cycle_sub(&self, app: &mut App) {
        match self {
            Self::Local(_) => LocalPlaybackTarget::cycle_sub(app),
            Self::Remote(target) => target.cycle_sub(app),
            Self::Cast(_) => CastPlaybackTarget::cycle_sub(app),
        }
    }

    pub(in crate::app) fn displayed_volume(&self, app: &App) -> i64 {
        match self {
            Self::Local(_) => LocalPlaybackTarget::displayed_volume(app),
            Self::Remote(_) => RemotePlaybackTarget::displayed_volume(app),
            Self::Cast(_) => CastPlaybackTarget::displayed_volume(app),
        }
    }

    pub(in crate::app) fn displayed_mute(&self, app: &App) -> bool {
        match self {
            Self::Local(_) => LocalPlaybackTarget::displayed_mute(app),
            Self::Remote(_) => RemotePlaybackTarget::displayed_mute(app),
            Self::Cast(_) => CastPlaybackTarget::displayed_mute(app),
        }
    }

    pub(in crate::app) fn indicator_data(&self, app: &App) -> Option<IndicatorData> {
        match self {
            Self::Local(_) => LocalPlaybackTarget::indicator_data(app),
            Self::Remote(_) => RemotePlaybackTarget::indicator_data(app),
            Self::Cast(_) => CastPlaybackTarget::indicator_data(app),
        }
    }
}

impl App {
    /// Returns the observed playback state for rendering.
    pub(in crate::app) fn effective_playback_state(&self) -> mbv_ui_model::playback::PlaybackState {
        // The attached cast target wins only while it actually reports (or is
        // optimistically awaiting) media. An attached receiver that is idle
        // must not shadow real playback: attach-on-selection attaches
        // optimistically before its status poll returns, so an idle
        // attachment would otherwise collapse the now-playing panel for
        // local and remote-session playback alike (cast-session-control's
        // "receiver is idle" scenario governs the *cast* presentation, not
        // every surface).
        match self.cast_effective_playback_state() {
            Some(state) if state.active => state,
            _ => self.non_cast_playback_state(),
        }
    }

    fn non_cast_playback_state(&self) -> mbv_ui_model::playback::PlaybackState {
        if let Some(ref remote) = self.connected_session_state {
            // The observed item is only a *local* playhead when the queue
            // holds it. A watched remote Session may be playing anything
            // (another device's own selection), so `active` follows the
            // Session and the slot stays `None`; presentation falls back to
            // the Session's own title and progress.
            let maybe_active_idx = remote.now_playing_item_id.as_ref().and_then(|id| {
                self.player_tab
                    .queue
                    .slots()
                    .iter()
                    .position(|s| s.item.id() == id)
            });
            let pos_ticks = {
                let elapsed_s = if remote.is_paused {
                    0.0
                } else {
                    self.remote.remote_pos_at.elapsed().as_secs_f64()
                };
                let remote_pos_s = mbv_emby_model::i64_to_f64_saturating(self.remote.remote_pos_s);
                let runtime_s = mbv_emby_model::i64_to_f64_saturating(remote.runtime_s);
                let pos_s = (remote_pos_s + elapsed_s).min(runtime_s);
                mbv_emby_model::seconds_to_ticks(pos_s)
            };
            mbv_ui_model::playback::PlaybackState {
                active: remote.now_playing.is_some(),
                active_idx: maybe_active_idx,
                position_ticks: pos_ticks,
                runtime_ticks: remote.runtime_s * mbv_emby_model::TICKS_PER_SECOND,
                paused: remote.is_paused,
            }
        } else {
            let s = self.player.status.lock().unwrap();
            let active_idx = self
                .playback_queue()
                .queue
                .active_index()
                .unwrap_or(s.current_idx);
            let (position_ticks, runtime_ticks) = (s.position_ticks, s.runtime_ticks);
            mbv_ui_model::playback::PlaybackState {
                active: s.active,
                active_idx: Some(active_idx),
                position_ticks,
                runtime_ticks,
                paused: s.paused,
            }
        }
    }

    /// Derives the now-playing header status from the effective playback
    /// state: active-and-unpaused `Playing`, active-and-paused `Paused`,
    /// inactive `Idle` (a paused flag on an inactive transport is
    /// unreachable and collapses to `Idle`).
    pub(in crate::app) fn now_playing_status(&self) -> NowPlayingStatus {
        let state = self.effective_playback_state();
        match (state.active, state.paused) {
            (true, false) => NowPlayingStatus::Playing,
            (true, true) => NowPlayingStatus::Paused,
            (false, _) => NowPlayingStatus::Idle,
        }
    }

    /// Whether the `QueueColumn`'s visual slot should be reserved and painted.
    pub(in crate::app) fn visual_slot_shown(&self) -> bool {
        self.now_playing_status() != NowPlayingStatus::Idle && !self.visual_slot_hidden
    }

    pub(in crate::app) fn pending_playback_slot(&self) -> Option<mbv_queue::QueueSlotId> {
        self.queue_for_scope(self.playing_queue_scope())
            .pending_playback_slot
    }

    /// The playback projection the queue ROWS are painted from. Bare mode
    /// blanks a queue fenced ahead of its local Player; out-of-process owner
    /// snapshots reconcile queue slots and playback coordinates together.
    pub(in crate::app) fn queue_row_playback_state(&self) -> mbv_ui_model::playback::PlaybackState {
        let mut state = self.displayed_queue_playback_state();
        if state.active && !self.local_queue_is_owner_queue(self.viewed_queue_scope()) {
            state.active = false;
        }
        state
    }

    /// The playhead presentation reads: the confirmed playback state, or --
    /// while a selected slot awaits the playback owner's report -- that slot
    /// with no progress. Presentation only: authority consumers (transport
    /// gates, effects, reporting) keep `effective_playback_state`.
    pub(in crate::app) fn displayed_playback_state(&self) -> mbv_ui_model::playback::PlaybackState {
        let state = self.effective_playback_state();
        let Some(index) = self.predicted_active_index() else {
            return state;
        };
        if state.active && state.active_idx == Some(index) {
            return state;
        }
        mbv_ui_model::playback::PlaybackState {
            active: true,
            active_idx: Some(index),
            // The owner is still playing the outgoing item, so its position
            // says nothing about this slot: paint a fresh start against the
            // selected item's own runtime until the owner confirms.
            position_ticks: 0,
            runtime_ticks: self
                .playback_queue()
                .item_at(index)
                .map_or(0, mbv_queue::QueueItem::runtime_ticks),
            paused: false,
        }
    }

    /// The position of the slot the user selected to play in the playing
    /// queue, while the playback owner has not yet confirmed it.
    fn predicted_active_index(&self) -> Option<usize> {
        let target = self.pending_playback_slot()?;
        self.queue_for_scope(self.playing_queue_scope())
            .slots()
            .iter()
            .position(|slot| slot.slot_id == target)
    }

    pub(in crate::app) fn displayed_queue_playback_state(
        &self,
    ) -> mbv_ui_model::playback::PlaybackState {
        if self.queue_scope_is_playback(self.viewed_queue_scope()) {
            self.effective_playback_state()
        } else {
            mbv_ui_model::playback::PlaybackState::default()
        }
    }
}

#[cfg(test)]
mod now_playing_status_tests {
    use super::*;
    use crate::app::tests::make_app_stub;

    fn app() -> App {
        make_app_stub()
    }

    fn set_player(app: &App, active: bool, paused: bool) {
        let mut status = app.player.status.lock().unwrap();
        status.active = active;
        status.paused = paused;
    }

    #[test]
    fn now_playing_status_covers_the_three_states() {
        // Idle: nothing active — an unreachable stale `paused` flag still
        // reads as Idle.
        let app = app();
        assert_eq!(app.now_playing_status(), NowPlayingStatus::Idle);
        set_player(&app, false, true);
        assert_eq!(app.now_playing_status(), NowPlayingStatus::Idle);

        // Playing.
        set_player(&app, true, false);
        assert_eq!(app.now_playing_status(), NowPlayingStatus::Playing);

        // Paused counts as active.
        set_player(&app, true, true);
        assert_eq!(app.now_playing_status(), NowPlayingStatus::Paused);
    }

    fn idle_cast_status() -> mbv_cast::client::CastStatus {
        mbv_cast::client::CastStatus {
            position_seconds: None,
            duration_seconds: None,
            playback_rate: 1.0,
            state: mbv_cast::client::CastPlaybackState::Idle,
            playing_content_id: None,
        }
    }

    /// Regression: a cast receiver reattached at launch is attached but idle
    /// (and its status poll may fail outright, leaving no status at all).
    /// That attachment must not shadow real local playback -- the now-playing
    /// panel collapsed to Idle a few seconds into every video until this was
    /// split.
    #[test]
    fn attached_but_idle_cast_does_not_shadow_local_playback() {
        let mut app = make_app_stub();
        app.attach_cast("device-1".to_string());

        // Receiver reports no active media.
        app.apply_cast_status("device-1", Ok(idle_cast_status()));
        set_player(&app, true, false);
        assert_eq!(app.now_playing_status(), NowPlayingStatus::Playing);

        // Connection lost before any status arrived (the observed failure:
        // "cast get_status returned no entries"), status stays None.
        let mut app = make_app_stub();
        app.attach_cast("device-1".to_string());
        app.apply_cast_status("device-1", Err(mbv_cast::CastError::status_no_entries()));
        set_player(&app, true, false);
        assert_eq!(app.now_playing_status(), NowPlayingStatus::Playing);
    }

    /// An engaged cast target keeps priority over the local player.
    #[test]
    fn playing_cast_still_wins_over_the_local_player() {
        let mut app = make_app_stub();
        app.attach_cast("device-1".to_string());
        app.apply_cast_status(
            "device-1",
            Ok(mbv_cast::client::CastStatus {
                position_seconds: Some(1.0),
                duration_seconds: Some(100.0),
                playback_rate: 1.0,
                state: mbv_cast::client::CastPlaybackState::Playing,
                playing_content_id: None,
            }),
        );
        set_player(&app, false, false);
        assert_eq!(app.now_playing_status(), NowPlayingStatus::Playing);
    }
}
