mod cast;
mod local;
mod remote;

use crate::app::render::indicators::IndicatorData;
use crate::app::{
    App, CastPlaybackTarget, LocalPlaybackTarget, PlaybackTarget, RemotePlaybackTarget,
};

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

/// The now-playing status word's source (design D10; folded change D2):
/// derived once per frame next to `effective_playback_state()` and consumed
/// by the Queue playback panel's header row and the idle collapse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum NowPlayingStatus {
    /// Active and not paused.
    Playing,
    /// Active and paused.
    Paused,
    /// No transport active. A stale `paused` flag on an inactive transport
    /// is unreachable in practice and reads as `Idle`.
    Idle,
}

impl App {
    /// Returns the observed playback state for rendering.
    pub(in crate::app) fn effective_playback_state(&self) -> crate::app::PlaybackState {
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

    fn non_cast_playback_state(&self) -> crate::app::PlaybackState {
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
            #[expect(
                clippy::cast_precision_loss,
                clippy::cast_possible_truncation,
                reason = "seconds↔ticks conversion through f64; no lossless integer-path conversion exists (approved, issue #804)"
            )]
            let pos_ticks = {
                let elapsed_s = if remote.is_paused {
                    0.0
                } else {
                    self.remote.remote_pos_at.elapsed().as_secs_f64()
                };
                let pos_s =
                    (self.remote.remote_pos_s as f64 + elapsed_s).min(remote.runtime_s as f64);
                (pos_s * mbv_core::api::TICKS_PER_SECOND as f64) as i64
            };
            crate::app::PlaybackState {
                active: remote.now_playing.is_some(),
                active_idx: maybe_active_idx,
                position_ticks: pos_ticks,
                runtime_ticks: remote.runtime_s * mbv_core::api::TICKS_PER_SECOND,
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
            crate::app::PlaybackState {
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

    pub(in crate::app) fn pending_playback_slot(
        &self,
    ) -> Option<mbv_core::playback_queue::QueueSlotId> {
        self.queue_for_scope(self.playing_queue_scope())
            .pending_playback_slot
            .or_else(|| self.bare_in_flight_slot())
    }

    /// The playback projection the queue ROWS are painted from. Bare mode
    /// blanks a queue fenced ahead of its local Player; out-of-process owner
    /// snapshots reconcile queue slots and playback coordinates together.
    pub(in crate::app) fn queue_row_playback_state(&self) -> crate::app::PlaybackState {
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
    pub(in crate::app) fn displayed_playback_state(&self) -> crate::app::PlaybackState {
        let state = self.effective_playback_state();
        let Some(index) = self.predicted_active_index() else {
            return state;
        };
        if state.active && state.active_idx == Some(index) {
            return state;
        }
        crate::app::PlaybackState {
            active: true,
            active_idx: Some(index),
            // The owner is still playing the outgoing item, so its position
            // says nothing about this slot: paint a fresh start against the
            // selected item's own runtime until the owner confirms.
            position_ticks: 0,
            runtime_ticks: self
                .playback_queue()
                .item_at(index)
                .map_or(0, mbv_core::playback::QueueItem::runtime_ticks),
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

    /// The Bare owner's desired-transition slot. The shell owns the local
    /// transition it just dispatched, exactly as the daemon owner owns the
    /// in-flight transition it publishes, so a locally selected slot projects
    /// as now-playing before the Playback run reports the change
    /// (queue-canonical-list, "Selecting a different item to play").
    fn bare_in_flight_slot(&self) -> Option<mbv_core::playback_queue::QueueSlotId> {
        if self.player.is_remote() {
            return None;
        }
        self.bare_owner.in_flight_transition_slot()
    }

    pub(in crate::app) fn displayed_queue_playback_state(&self) -> crate::app::PlaybackState {
        if self.queue_scope_is_playback(self.viewed_queue_scope()) {
            self.effective_playback_state()
        } else {
            crate::app::PlaybackState::default()
        }
    }
}

#[cfg(test)]
mod now_playing_status_tests {
    use super::*;
    use crate::app::tests::{make_app_stub, make_items, make_remote_app_stub};

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

    fn idle_cast_status() -> mbv_core::cast::client::CastStatus {
        mbv_core::cast::client::CastStatus {
            position_seconds: None,
            duration_seconds: None,
            playback_rate: 1.0,
            state: mbv_core::cast::client::CastPlaybackState::Idle,
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
        app.apply_cast_status("device-1", Err("get_status returned no entries".into()));
        set_player(&app, true, false);
        assert_eq!(app.now_playing_status(), NowPlayingStatus::Playing);
    }

    /// Regression: with a stay-alive local daemon still playing the previous
    /// Bare mode's local playhead must not claim a row of a fenced replacement.
    #[test]
    fn fenced_queue_claims_no_now_playing_row_while_bare_player_plays_the_old_queue() {
        let mut app = make_app_stub();
        app.player_tab.set_items(make_items(3), 0);
        // The local Player is playing the first item of its previous queue.
        {
            let mut status = app.player.status.lock().unwrap();
            status.active = true;
            status.current_idx = 0;
        };

        app.replace_playback_queue(make_items(2), 0);

        let state = app.queue_row_playback_state();
        assert!(
            !state.active,
            "the owner's playhead must not claim a row of a fenced replacement queue"
        );
    }

    /// An engaged cast target keeps priority over the local player.
    #[test]
    fn playing_cast_still_wins_over_the_local_player() {
        let mut app = make_app_stub();
        app.attach_cast("device-1".to_string());
        app.apply_cast_status(
            "device-1",
            Ok(mbv_core::cast::client::CastStatus {
                position_seconds: Some(1.0),
                duration_seconds: Some(100.0),
                playback_rate: 1.0,
                state: mbv_core::cast::client::CastPlaybackState::Playing,
                playing_content_id: None,
            }),
        );
        set_player(&app, false, false);
        assert_eq!(app.now_playing_status(), NowPlayingStatus::Playing);
    }

    /// Regression (0.19.3): the reseat generation fence blanked remote
    /// (mbvd) queue rows. A daemon owner bumps `sequence_generation` on
    /// every accepted submit but the client never stamps the tab for a
    /// remote scope (`submit_tab_queue` skips it, `from_unified_state`
    /// resets it to 0), so the fence saw a permanent mismatch and cleared
    /// `active` — no play icon, no foam progress. The fence is for the
    /// bare run only; daemon snapshots are authoritative.
    #[test]
    fn queue_row_playback_state_stays_active_for_direct_remote_queue() {
        let app = make_remote_app_stub(make_items(1), make_items(3));
        assert!(app.player.is_remote());
        {
            let mut status = app.player.status.lock().unwrap();
            status.active = true;
            status.current_idx = 1;
            status.position_ticks = 42;
            status.runtime_ticks = 84;
            // Owner advanced past the never-stamped tab generation.
            status.sequence_generation = 7;
        };
        let state = app.queue_row_playback_state();
        assert!(state.active);
        assert_eq!(state.active_idx, Some(1));
    }
}
