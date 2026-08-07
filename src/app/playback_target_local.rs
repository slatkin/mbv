use super::ui_util::take_chars;
use super::{App, LocalPlaybackTarget};
use crate::app::render::indicators::IndicatorData;
use mbv_core::player::PlayerCommand;

impl LocalPlaybackTarget {
    pub(super) fn toggle_play_pause(&self, app: &mut App) {
        if app.player.is_remote() {
            let paused = !app.player.status.lock().unwrap().paused;
            app.flash_status(if paused {
                "Pause requested".to_string()
            } else {
                "Resume requested".to_string()
            });
            app.player.set_paused(paused);
        } else {
            app.player.send_command(PlayerCommand::TogglePause);
        }
    }

    pub(super) fn stop(&self, app: &mut App) {
        if app.player.is_remote() {
            app.flash_status("Stop requested".to_string());
        }
        app.player.stop();
    }

    pub(super) fn seek_relative(&self, app: &mut App, delta: f64) {
        app.player.send_command(PlayerCommand::Seek(delta));
    }

    pub(super) fn jump_track(&self, app: &mut App, step: i64) {
        if step >= 0 {
            if app.player.is_remote() {
                app.flash_status("Next requested".to_string());
            }
            app.player.next();
        } else {
            if app.player.is_remote() {
                app.flash_status("Previous requested".to_string());
            }
            app.player.previous();
        }
    }

    pub(super) fn toggle_command_mute(&self, app: &mut App) {
        app.mute_on = !app.mute_on;
        app.player.send_command(PlayerCommand::SetMute(app.mute_on));
        app.save_prefs();
    }

    pub(super) fn is_audio_item(&self, app: &App) -> bool {
        let idx = app.player_tab.queue_cursor;
        app.player_tab
            .items
            .get(idx)
            .map(|i| i.media_type == "Audio" || i.item_type == "Audio")
            .unwrap_or(false)
    }

    pub(super) fn toggle_soft_mute(&self, app: &mut App) {
        if app.ui_volume == 0 {
            if let Some(v) = app.pre_mute_volume.take() {
                app.player.send_command(PlayerCommand::SetVolume(v as i64));
                app.ui_volume = v;
            }
        } else {
            app.pre_mute_volume = Some(app.ui_volume);
            app.player.send_command(PlayerCommand::SetVolume(0));
            app.ui_volume = 0;
        }
        app.save_prefs();
    }

    pub(super) fn cycle_audio(&self, app: &mut App) {
        let (tracks, current_id) = {
            let s = app.player.status.lock().unwrap();
            (s.audio_tracks.clone(), s.audio_id)
        };
        if tracks.is_empty() {
            return;
        }
        let mut entries: Vec<i64> = vec![0];
        entries.extend(tracks.iter().map(|(id, _)| *id));
        let cur = entries.iter().position(|&id| id == current_id).unwrap_or(0);
        let next = (cur + 1) % entries.len();
        let next_id = entries[next];
        if next_id == 0 {
            app.pre_mute_volume = Some(app.ui_volume);
            app.player.send_command(PlayerCommand::SetVolume(0));
            app.ui_volume = 0;
        } else if current_id == 0 {
            if let Some(v) = app.pre_mute_volume.take() {
                app.player.send_command(PlayerCommand::SetVolume(v as i64));
                app.ui_volume = v;
            }
        }
        app.player.send_command(PlayerCommand::SetAudio(next_id));
    }

    pub(super) fn adjust_volume(&self, app: &mut App, delta: i64) {
        let active = app.player.status.lock().unwrap().active;
        if active {
            let st = app.player.status.lock().unwrap();
            let v = (st.volume + delta).clamp(0, st.volume_max) as u8;
            drop(st);
            app.player.send_command(PlayerCommand::SetVolume(v as i64));
            app.ui_volume = v;
        } else {
            app.ui_volume = (app.ui_volume as i64 + delta).clamp(0, 200) as u8;
        }
        app.save_prefs();
    }

    pub(super) fn cycle_sub(&self, app: &mut App) {
        let (active, tracks, current_id) = {
            let s = app.player.status.lock().unwrap();
            (s.active, s.sub_tracks.clone(), s.sub_id)
        };
        if !active {
            app.cycle_subtitle_mode();
            return;
        }
        if tracks.is_empty() {
            return;
        }
        let mut entries: Vec<i64> = vec![0];
        entries.extend(tracks.iter().map(|(id, _, _)| *id));
        let next_id = App::next_subtitle_entry(&entries, current_id);
        app.player.send_command(PlayerCommand::SetSub(next_id));
        app.save_prefs();
    }

    pub(super) fn displayed_volume(&self, app: &App) -> i64 {
        let s = app.player.status.lock().unwrap();
        if s.active {
            if s.muted {
                0
            } else {
                s.volume
            }
        } else {
            app.ui_volume as i64
        }
    }

    pub(super) fn displayed_mute(&self, app: &App) -> bool {
        app.mute_on
    }

    pub(super) fn indicator_data(&self, app: &App) -> Option<IndicatorData> {
        let pst = app.player.status.lock().unwrap();
        if !pst.active {
            return None;
        }
        let video_is_image = pst.video_is_image;
        let res_h = pst.video_height;
        let is_audio_only = video_is_image || res_h == 0;
        let res_str = if video_is_image || res_h == 0 {
            if pst.audio_codec.is_empty() {
                "--".to_string()
            } else {
                pst.audio_codec.to_uppercase()
            }
        } else {
            res_h.to_string()
        };
        let res_dim = res_str == "--";
        let raw_lang = pst.audio_lang.to_lowercase();
        let (audio_label, audio_dim): (String, bool) = if raw_lang.is_empty() {
            ("x".into(), true)
        } else {
            (take_chars(&raw_lang, 2), false)
        };
        let sub_id = pst.sub_id;
        let raw_sub_lang = pst.sub_lang.to_lowercase();
        drop(pst);
        let sub_label = if sub_id == 0 {
            "off".into()
        } else if !raw_sub_lang.is_empty() {
            take_chars(&raw_sub_lang, 3)
        } else {
            "CC".into()
        };
        Some(IndicatorData {
            res_label: res_str,
            res_dim,
            audio_label,
            audio_dim,
            audio_only: is_audio_only,
            sub_label,
        })
    }
}
