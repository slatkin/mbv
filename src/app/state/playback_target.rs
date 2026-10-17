mod local;

use crate::app::App;
use mbv_ui_model::playback_target::NowPlayingStatus;

#[derive(Clone, Copy)]
pub(in crate::app) struct LocalPlaybackTarget;

impl App {
    /// Returns the observed playback state for rendering.
    pub(in crate::app) fn effective_playback_state(&self) -> mbv_ui_model::playback::PlaybackState {
        let s = self.player.status_snapshot();
        let active_idx = s.current_idx;
        let (position_ticks, runtime_ticks) = (s.position_ticks, s.runtime_ticks);
        mbv_ui_model::playback::PlaybackState {
            active: s.active,
            active_idx: Some(active_idx),
            position_ticks,
            runtime_ticks,
            paused: s.paused,
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
            .pending_playback_slot()
    }

    /// The playhead presentation reads: the confirmed playback state, or --
    /// while a selected slot awaits the playback owner's report -- that slot
    /// with no progress. Presentation only: authority consumers (transport
    /// gates, effects, reporting) keep `effective_playback_state`.
    pub(in crate::app) fn displayed_playback_state(&self) -> mbv_ui_model::playback::PlaybackState {
        let state = self.effective_playback_state();
        let Some(index) = self.pending_playback_index() else {
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

    /// The adopted pending slot's position while the owner has not confirmed it.
    fn pending_playback_index(&self) -> Option<usize> {
        let target = self.pending_playback_slot()?;
        self.queue_for_scope(self.playing_queue_scope())
            .slot_index(target)
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
        app.player.update_status(|status| {
            status.active = active;
            status.paused = paused;
        });
    }

    #[test]
    fn now_playing_status_covers_the_three_states() {
        // Idle: nothing active — an unreachable stale `paused` flag still
        // reads as Idle. The stub player starts active (a remote-owner
        // stand-in), so park it idle first: this test owns the status.
        let app = app();
        set_player(&app, false, false);
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
}
