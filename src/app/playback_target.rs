use super::{App, PlaybackTarget};
use crate::app::render::indicators::IndicatorData;

impl PlaybackTarget {
    pub(super) fn toggle_play_pause(&self, app: &mut App) {
        match self {
            Self::Local(target) => target.toggle_play_pause(app),
            Self::Remote(target) => target.toggle_play_pause(app),
        }
    }

    pub(super) fn stop(&self, app: &mut App) {
        match self {
            Self::Local(target) => target.stop(app),
            Self::Remote(target) => target.stop(app),
        }
    }

    pub(super) fn seek_relative(&self, app: &mut App, delta: f64) {
        match self {
            Self::Local(target) => target.seek_relative(app, delta),
            Self::Remote(target) => target.seek_relative(app, delta),
        }
    }

    pub(super) fn jump_track(&self, app: &mut App, step: i64, transport: &'static str) {
        match self {
            Self::Local(target) => target.jump_track(app, step),
            Self::Remote(target) => target.jump_track(app, step, transport),
        }
    }

    pub(super) fn toggle_command_mute(&self, app: &mut App) {
        match self {
            Self::Local(target) => target.toggle_command_mute(app),
            Self::Remote(target) => target.toggle_command_mute(app),
        }
    }

    pub(super) fn is_audio_item(&self, app: &App) -> bool {
        match self {
            Self::Local(target) => target.is_audio_item(app),
            Self::Remote(target) => target.is_audio_item(app),
        }
    }

    pub(super) fn toggle_soft_mute(&self, app: &mut App) {
        match self {
            Self::Local(target) => target.toggle_soft_mute(app),
            Self::Remote(target) => target.toggle_soft_mute(app),
        }
    }

    pub(super) fn cycle_audio(&self, app: &mut App) {
        match self {
            Self::Local(target) => target.cycle_audio(app),
            Self::Remote(target) => target.cycle_audio(app),
        }
    }

    pub(super) fn adjust_volume(&self, app: &mut App, delta: i64) {
        match self {
            Self::Local(target) => target.adjust_volume(app, delta),
            Self::Remote(target) => target.adjust_volume(app, delta),
        }
    }

    pub(super) fn cycle_sub(&self, app: &mut App) {
        match self {
            Self::Local(target) => target.cycle_sub(app),
            Self::Remote(target) => target.cycle_sub(app),
        }
    }

    pub(super) fn displayed_volume(&self, app: &App) -> i64 {
        match self {
            Self::Local(target) => target.displayed_volume(app),
            Self::Remote(target) => target.displayed_volume(app),
        }
    }

    pub(super) fn displayed_mute(&self, app: &App) -> bool {
        match self {
            Self::Local(target) => target.displayed_mute(app),
            Self::Remote(target) => target.displayed_mute(app),
        }
    }

    pub(super) fn indicator_data(&self, app: &App) -> Option<IndicatorData> {
        match self {
            Self::Local(target) => target.indicator_data(app),
            Self::Remote(target) => target.indicator_data(app),
        }
    }
}

impl App {
    pub(super) fn effective_playback_state(&self) -> super::PlaybackState {
        if let Some(ref remote) = self.connected_session_state {
            // For direct remote playback, look up the active item in the
            // remote queue (the playback authority), not the unrelated local
            // queue.  Moving an active item doesn't change queue length, so
            // a length-based reconciliation would clear the optimistic index
            // before the asynchronous move is acknowledged.
            let remote_queue = self
                .remote_player_tab
                .as_ref()
                .map(|tab| &tab.items)
                .unwrap_or(&self.player_tab.items);
            let maybe_active_idx = remote
                .now_playing_item_id
                .as_ref()
                .and_then(|id| remote_queue.iter().position(|it| &it.id == id));
            let active_idx = maybe_active_idx.unwrap_or(0);
            let pos_ticks = {
                let elapsed_s = if remote.is_paused {
                    0.0
                } else {
                    self.remote_pos_at.elapsed().as_secs_f64()
                };
                let pos_s = (self.remote_pos_s as f64 + elapsed_s).min(remote.runtime_s as f64);
                (pos_s * mbv_core::api::TICKS_PER_SECOND as f64) as i64
            };
            super::PlaybackState {
                active: remote.now_playing.is_some() && maybe_active_idx.is_some(),
                active_idx,
                position_ticks: pos_ticks,
                runtime_ticks: remote.runtime_s * mbv_core::api::TICKS_PER_SECOND,
                paused: remote.is_paused,
            }
        } else {
            let s = self.player.status.lock().unwrap();
            super::PlaybackState {
                active: s.active,
                active_idx: s.current_idx,
                position_ticks: s.position_ticks,
                runtime_ticks: s.runtime_ticks,
                paused: s.paused,
            }
        }
    }

    pub(super) fn displayed_queue_playback_state(&self) -> super::PlaybackState {
        if self.queue_scope_is_playback(self.visible_queue_scope()) {
            self.effective_playback_state()
        } else {
            super::PlaybackState::default()
        }
    }
}
