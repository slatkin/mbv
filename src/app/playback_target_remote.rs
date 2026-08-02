use super::ui_util::take_chars;
use super::{App, LocalPlaybackTarget, RemotePlaybackTarget};
use crate::app::render::indicators::IndicatorData;

impl RemotePlaybackTarget {
    pub(super) fn toggle_play_pause(&self, app: &mut App) {
        let session_id = self.session_id.clone();
        app.do_session_command(move |c| c.session_transport(&session_id, "PlayPause"));
    }

    pub(super) fn stop(&self, app: &mut App) {
        let session_id = self.session_id.clone();
        app.do_session_command(move |c| c.session_transport(&session_id, "Stop"));
    }

    pub(super) fn seek_relative(&self, app: &mut App, delta: f64) {
        let pos_s = app
            .connected_session_state
            .as_ref()
            .map(|s| s.position_s)
            .unwrap_or(0);
        let target = App::remote_seek_ticks(pos_s, delta);
        let session_id = self.session_id.clone();
        app.do_session_command(move |c| c.session_seek(&session_id, target));
    }

    pub(super) fn jump_track(&self, app: &mut App, step: i64, transport: &'static str) {
        app.session_jump_track(&self.session_id, step, transport);
    }

    pub(super) fn toggle_command_mute(&self, app: &mut App) {
        app.session_toggle_mute();
    }

    pub(super) fn is_audio_item(&self, app: &App) -> bool {
        app.connected_session_state
            .as_ref()
            .map(|s| s.media_info.audio_only)
            .unwrap_or(false)
    }

    pub(super) fn toggle_soft_mute(&self, app: &mut App) {
        // No session-level mute primitive exists for `a`, so keep routing the
        // remote path through the audio-track cycle behavior.
        self.cycle_audio(app);
    }

    pub(super) fn cycle_audio(&self, app: &mut App) {
        let remote_indexes = app.remote_audio_indexes();
        let cur = app
            .connected_session_state
            .as_ref()
            .map(|s| s.audio_index)
            .unwrap_or(1);
        let next = if remote_indexes.is_empty() {
            if cur <= 1 {
                2
            } else {
                1
            }
        } else {
            let cur_pos = remote_indexes
                .iter()
                .position(|&idx| idx == cur)
                .unwrap_or(0);
            remote_indexes[(cur_pos + 1) % remote_indexes.len()]
        };
        if let Some(ref mut state) = app.connected_session_state {
            state.audio_index = next;
        }
        let session_id = self.session_id.clone();
        app.do_session_command(move |c| c.session_set_audio_index(&session_id, next));
    }

    pub(super) fn adjust_volume(&self, app: &mut App, delta: i64) {
        let vol = app
            .connected_session_state
            .as_ref()
            .map(|s| s.volume)
            .unwrap_or(50);
        let new_vol = (vol + delta).clamp(0, 100);
        let session_id = self.session_id.clone();
        app.do_session_command(move |c| c.session_set_volume(&session_id, new_vol));
    }

    pub(super) fn cycle_sub(&self, app: &mut App) {
        let remote_indexes = app.remote_subtitle_indexes();
        if remote_indexes.is_empty() {
            app.toggle_sub();
            return;
        }
        let current = app
            .connected_session_state
            .as_ref()
            .map(|s| s.sub_index)
            .unwrap_or(-1);
        let mut entries = Vec::with_capacity(remote_indexes.len() + 1);
        entries.push(-1);
        entries.extend(remote_indexes);
        let next = App::next_subtitle_entry(&entries, current);
        if let Some(ref mut state) = app.connected_session_state {
            state.sub_index = next;
        }
        let session_id = self.session_id.clone();
        app.do_session_command(move |c| c.session_set_subtitle_index(&session_id, next));
    }

    pub(super) fn displayed_volume(&self, app: &App) -> i64 {
        app.connected_session_state
            .as_ref()
            .map(|s| s.volume)
            .unwrap_or_else(|| LocalPlaybackTarget.displayed_volume(app))
    }

    pub(super) fn displayed_mute(&self, app: &App) -> bool {
        app.connected_session_state
            .as_ref()
            .map(|s| s.muted)
            .unwrap_or_else(|| LocalPlaybackTarget.displayed_mute(app))
    }

    pub(super) fn indicator_data(&self, app: &App) -> Option<IndicatorData> {
        let remote = app.connected_session_state.as_ref()?;
        let audio_label = remote
            .media_info
            .audio_streams
            .iter()
            .find(|stream| stream.index == remote.audio_index)
            .map(|stream| {
                if !stream.language.is_empty() {
                    take_chars(&stream.language.to_lowercase(), 2)
                } else {
                    take_chars(&stream.label.to_lowercase(), 2)
                }
            })
            .unwrap_or_else(|| "---".to_string());
        let sub_label = if remote.sub_index < 0 {
            "off".to_string()
        } else {
            remote
                .media_info
                .subtitle_streams
                .iter()
                .find(|stream| stream.index == remote.sub_index)
                .map(|stream| {
                    if !stream.language.is_empty() {
                        take_chars(&stream.language.to_lowercase(), 3)
                    } else {
                        take_chars(&stream.label.to_lowercase(), 3)
                    }
                })
                .unwrap_or_else(|| "CC".to_string())
        };
        let res_label = if remote.media_info.video_label.is_empty() {
            "---".to_string()
        } else if remote.media_info.audio_only {
            remote
                .media_info
                .video_label
                .split("  |  ")
                .next()
                .unwrap_or(&remote.media_info.video_label)
                .to_string()
        } else {
            remote
                .media_info
                .video_label
                .split_whitespace()
                .next()
                .unwrap_or(&remote.media_info.video_label)
                .to_string()
        };
        let res_label = res_label
            .strip_suffix('p')
            .or_else(|| res_label.strip_suffix('P'))
            .unwrap_or(&res_label)
            .to_string();
        Some(IndicatorData {
            res_label: res_label.clone(),
            res_dim: res_label == "---",
            audio_label: audio_label.clone(),
            audio_dim: audio_label == "---",
            audio_only: remote.media_info.audio_only,
            sub_label,
        })
    }
}
