use super::{
    auto_select_tracks, mpv_err_str, mpv_load_opts, mpv_title_opt, mpv_url_for_queue_item,
    queue_load_indices, queue_load_location, reassert_queue_layout, refresh_tracks,
    reject_stale_jump, resolve_jump_target, seek_decision, send_ep_info, shift_index_for_move,
    spawn_progress_reporter, start_queue_playback, volume_decision, LoadState, PlaybackOrigin,
    PlaybackRun, ProgressGuard, StopReport,
};
use crate::api::EmbyItem;
use crate::playback_execution_sequence::{ExecSlot, ExecutionSequence};
use crate::playback_queue::{QueueItem, QueueSlotId};
use crate::player::{PlayerCommand, PlayerEvent};
use libmpv2::Mpv;
use std::time::Instant;

impl PlaybackRun {
    pub(in crate::player) fn handle_command(
        &mut self,
        cmd: PlayerCommand,
        mpv: &Mpv,
        progress: &mut ProgressGuard,
    ) -> bool {
        let mut cancel_stop = false;
        match cmd {
            PlayerCommand::JumpTo {
                slot_id,
                request_id,
                generation,
                resume_ticks,
            } => {
                self.cmd_jump_to(slot_id, request_id, generation, resume_ticks, mpv);
            }
            PlayerCommand::QueueAppend { items } => {
                self.cmd_append_queue(items, mpv);
            }
            PlayerCommand::QueueRemove(slot_id) => {
                self.cmd_queue_remove(slot_id, mpv);
            }
            PlayerCommand::QueueMove(slot_id, to) => {
                self.cmd_queue_move(slot_id, to, mpv);
            }
            PlayerCommand::SetAudio(id) => {
                if id > 0 {
                    let _ = mpv.set_property("aid", id);
                } else {
                    let _ = mpv.set_property("aid", "no".to_string());
                }
                self.status.lock().unwrap().audio_id = id;
                refresh_tracks(mpv, &self.status);
            }
            PlayerCommand::SetSub(id) => {
                if id == 0 {
                    let _ = mpv.set_property("sid", "no".to_string());
                } else {
                    let _ = mpv.set_property("sid", id);
                }
                refresh_tracks(mpv, &self.status);
                self.status.lock().unwrap().sub_id = id;
            }
            PlayerCommand::SetSubtitlePrefs {
                mode,
                subtitle_lang,
                audio_lang,
            } => {
                {
                    let mut p = self.subtitle_prefs.lock().unwrap();
                    p.mode = mode;
                    p.subtitle_lang = subtitle_lang;
                    p.audio_lang = audio_lang;
                };
                let prefs = self.subtitle_prefs.lock().unwrap().clone();
                auto_select_tracks(mpv, &self.status, &prefs);
            }
            PlayerCommand::SetMute(m) => {
                let _ = mpv.set_property("mute", m);
                self.status.lock().unwrap().muted = m;
            }
            PlayerCommand::LoadNew {
                url,
                start_pos,
                item,
            } => {
                self.cmd_load_new(&url, start_pos, &item, mpv, progress);
                cancel_stop = true;
            }
            PlayerCommand::SubmitQueue { items, start_idx } => {
                if !items.is_empty() {
                    self.run_identity = self.status.lock().unwrap().sequence_generation;
                }
                self.cmd_submit_queue(items, start_idx, mpv, progress);
                cancel_stop = true;
            }
            command => self.handle_simple_command(command, mpv),
        }
        cancel_stop
    }

    fn handle_simple_command(&mut self, cmd: PlayerCommand, mpv: &Mpv) {
        match cmd {
            PlayerCommand::NextUpShow {
                item_id,
                show_title,
                ep_title,
                artist,
            } => {
                log::warn!(target: "player", "next-up: sending script-message mbv-next-up id={item_id} show={show_title} ep={ep_title}");
                let result = mpv.command(
                    "script-message",
                    &["mbv-next-up", &item_id, &show_title, &ep_title, &artist],
                );
                log::warn!(target: "player", "next-up: script-message result={result:?}");
            }
            PlayerCommand::TogglePause => {
                let paused = self.status.lock().unwrap().paused;
                let _ = mpv.set_property("pause", !paused);
            }
            PlayerCommand::Next => {
                let target = self.relative_step_base() + 1;
                if target < self.queue_len() {
                    self.step_to_index(target, mpv);
                }
            }
            PlayerCommand::Previous => {
                if let Some(target) = self.relative_step_base().checked_sub(1) {
                    self.step_to_index(target, mpv);
                }
            }
            command @ (PlayerCommand::NextUpDismiss | PlayerCommand::SkipIntroDismiss) => {
                let message = if matches!(command, PlayerCommand::NextUpDismiss) {
                    "mbv-next-up-dismiss"
                } else {
                    "mbv-skip-intro-dismiss"
                };
                let _ = mpv.command("script-message", &[message]);
            }
            PlayerCommand::SetVolume(volume) => {
                let vol_max = self.status.lock().unwrap().volume_max;
                let (volume, raw) = volume_decision(volume, vol_max);
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "computed volume (i64) → f64 for the mpv volume property; mpv stores volume as a float (approved, issue #804)"
                )]
                let _ = mpv.set_property("volume", raw as f64);
                self.status.lock().unwrap().volume = volume;
                let _ = mpv.command("show-text", &[&format!("Volume: {volume}%"), "1500"]);
            }
            command @ (PlayerCommand::Seek(secs) | PlayerCommand::SeekAbsolute(secs)) => {
                let absolute = matches!(command, PlayerCommand::SeekAbsolute(_));
                let (mode, seconds) = seek_decision(secs, absolute);
                let _ = mpv.command("seek", &[&seconds, mode]);
                self.last_seek_at = Some(Instant::now());
            }
            command => log::error!(target: "player", "unhandled command {command:?}"),
        }
    }
}

mod load;
mod queue;
