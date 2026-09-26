use crate::app::infra::ui_util::take_chars;
use crate::app::render::indicators::{short_resolution_label, IndicatorData, IndicatorFlags};
use crate::app::{App, LocalPlaybackTarget, RemotePlaybackTarget};

impl RemotePlaybackTarget {
    pub(in crate::app) fn toggle_play_pause(&self, app: &mut App) {
        let session_id = self.session_id.clone();
        app.do_session_command(move |c| c.session_transport(&session_id, "PlayPause"));
    }

    pub(in crate::app) fn stop(&self, app: &mut App) {
        let session_id = self.session_id.clone();
        app.do_session_command(move |c| c.session_transport(&session_id, "Stop"));
    }

    pub(in crate::app) fn seek_relative(&self, app: &mut App, delta: f64) {
        let pos_s = app
            .connected_session_state
            .as_ref()
            .map_or(0, |s| s.position_s);
        let target = App::remote_seek_ticks(pos_s, delta);
        let session_id = self.session_id.clone();
        app.do_session_command(move |c| c.session_seek(&session_id, target));
    }

    pub(in crate::app) fn jump_track(&self, app: &mut App, step: i64, transport: &'static str) {
        app.session_jump_track(&self.session_id, step, transport);
    }

    pub(in crate::app) fn toggle_command_mute(app: &mut App) {
        app.session_toggle_mute();
    }

    pub(in crate::app) fn is_audio_item(app: &App) -> bool {
        app.connected_session_state
            .as_ref()
            .is_some_and(|s| s.media_info.audio_only)
    }

    pub(in crate::app) fn toggle_soft_mute(&self, app: &mut App) {
        // No session-level mute primitive exists for `a`, so keep routing the
        // remote path through the audio-track cycle behavior.
        self.cycle_audio(app);
    }

    pub(in crate::app) fn cycle_audio(&self, app: &mut App) {
        let remote_indexes = app.remote_audio_indexes();
        let cur = app
            .connected_session_state
            .as_ref()
            .map_or(1, |s| s.audio_index);
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

    pub(in crate::app) fn adjust_volume(&self, app: &mut App, delta: i64) {
        let vol = app
            .connected_session_state
            .as_ref()
            .map_or(50, |s| s.volume);
        let new_vol = (vol + delta).clamp(0, 100);
        let session_id = self.session_id.clone();
        app.do_session_command(move |c| c.session_set_volume(&session_id, new_vol));
    }

    pub(in crate::app) fn cycle_sub(&self, app: &mut App) {
        let remote_indexes = app.remote_subtitle_indexes();
        if remote_indexes.is_empty() {
            app.toggle_sub();
            return;
        }
        let current = app
            .connected_session_state
            .as_ref()
            .map_or(-1, |s| s.sub_index);
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

    pub(in crate::app) fn displayed_volume(app: &App) -> i64 {
        app.connected_session_state
            .as_ref()
            .map_or_else(|| LocalPlaybackTarget::displayed_volume(app), |s| s.volume)
    }

    pub(in crate::app) fn displayed_mute(app: &App) -> bool {
        app.connected_session_state
            .as_ref()
            .map_or_else(|| LocalPlaybackTarget::displayed_mute(app), |s| s.muted)
    }

    pub(in crate::app) fn indicator_data(app: &App) -> Option<IndicatorData> {
        let remote = app.connected_session_state.as_ref()?;
        let audio_label = remote
            .media_info
            .audio_streams
            .iter()
            .find(|stream| stream.index == remote.audio_index)
            .map_or_else(
                || "---".to_string(),
                |stream| {
                    if stream.language.is_empty() {
                        take_chars(&stream.label.to_lowercase(), 2)
                    } else {
                        take_chars(&stream.language.to_lowercase(), 2)
                    }
                },
            );
        let sub_on = remote.sub_index >= 0;
        let sub_label = if sub_on {
            remote
                .media_info
                .subtitle_streams
                .iter()
                .find(|stream| stream.index == remote.sub_index)
                .map_or_else(
                    || "CC".to_string(),
                    |stream| {
                        if stream.language.is_empty() {
                            take_chars(&stream.label.to_lowercase(), 3)
                        } else {
                            take_chars(&stream.language.to_lowercase(), 3)
                        }
                    },
                )
        } else {
            "CC".to_string()
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
        let stripped = res_label
            .strip_suffix('p')
            .or_else(|| res_label.strip_suffix('P'))
            .unwrap_or(&res_label);
        let res_label = match stripped.parse::<u64>() {
            Ok(h) => short_resolution_label(h).to_string(),
            Err(_) if stripped.eq_ignore_ascii_case("4k") => "4K".to_string(),
            Err(_) => stripped.to_string(),
        };
        Some(IndicatorData {
            res_label: res_label.clone(),
            audio_label: audio_label.clone(),
            sub_label,
            flags: IndicatorFlags {
                res_dim: res_label == "---",
                audio_dim: audio_label == "---",
                audio_only: remote.media_info.audio_only,
                sub_on,
            },
        })
    }
}
