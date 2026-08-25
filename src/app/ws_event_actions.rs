use super::{notify_actions::ToastSeverity, App, LibEvent, PanelFocus};
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::player::PlayerCommand;
use mbv_core::ws::WsEvent;
use std::sync::Arc;

impl App {
    pub(super) fn handle_ws_event(&mut self, ev: WsEvent) {
        match ev {
            WsEvent::Play {
                item_ids,
                play_now,
                start_position_ticks,
                start_index,
            } => {
                log::info!(target: "ws", "Play: {} id(s), play_now={play_now}", item_ids.len());
                if !play_now {
                    return;
                }
                self.on_queue_replace_silent();
                let items = {
                    let Some(client) = self.emby_client() else {
                        self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
                        return;
                    };
                    let c = client.lock().unwrap();
                    match c.get_items_by_ids(&item_ids) {
                        Ok(v) => v,
                        Err(e) => {
                            let msg = format!("Couldn't play from remote: {e}");
                            drop(c);
                            self.flash(msg, ToastSeverity::Error);
                            return;
                        }
                    }
                };
                if items.is_empty() {
                    log::warn!(target: "ws", "Play: no items found for ids={}", item_ids.join(","));
                    return;
                }
                let start_idx = start_index.min(items.len().saturating_sub(1));
                self.set_panel_focus(PanelFocus::Queue);
                self.queue_source = crate::config::QueueSource::Remote;
                if items.len() == 1 {
                    let mut item = items[0].clone();
                    if start_position_ticks > 0 {
                        item.playback_position_ticks = start_position_ticks;
                    }
                    self.player_tab.set_items(vec![item.clone()], 0);
                    self.flash(item.playback_label(), ToastSeverity::Neutral);
                    let Some(c) = self.emby_snapshot().map(Arc::new) else {
                        self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
                        return;
                    };
                    self.player
                        .play(&item, self.queue_source.clone(), c, self.ui_volume);
                } else {
                    self.player_tab.set_items(items.clone(), start_idx);
                    let Some(c) = self.emby_snapshot().map(Arc::new) else {
                        self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
                        return;
                    };
                    log::info!(target: "ws", "Play multi: count={}, start_idx={start_idx}", items.len());
                    // Always hand the whole list to play_queue (not just the clicked
                    // item) so the remote-controlled queue continues past start_idx.
                    // play_queue already handles the "something is already playing"
                    // case in place via ReplaceQueue.
                    let mut items_with_pos = items.clone();
                    if start_position_ticks > 0 {
                        items_with_pos[start_idx].playback_position_ticks = start_position_ticks;
                    }
                    self.player.play_queue(
                        items_with_pos,
                        start_idx,
                        self.queue_source.clone(),
                        c,
                        self.ui_volume,
                    );
                }
                self.save_queue_state();
            }
            WsEvent::Stop => {
                self.player.stop();
            }
            WsEvent::Pause => {
                self.player.set_paused(true);
            }
            WsEvent::Unpause => {
                self.player.set_paused(false);
            }
            WsEvent::NextTrack => {
                self.player.next();
            }
            WsEvent::PreviousTrack => {
                self.player.previous();
            }
            WsEvent::TogglePause => {
                self.player.send_command(PlayerCommand::TogglePause);
            }
            WsEvent::Seek(ticks) => {
                self.player.send_command(PlayerCommand::SeekAbsolute(
                    ticks as f64 / TICKS_PER_SECOND as f64,
                ));
            }
            WsEvent::SeekRelative(secs) => {
                self.player.send_command(PlayerCommand::Seek(secs));
            }
            WsEvent::SetVolume(v) => {
                let vol_max = self.player.status.lock().unwrap().volume_max;
                self.player
                    .send_command(PlayerCommand::SetVolume(v.clamp(0, vol_max)));
            }
            WsEvent::VolumeUp => {
                let st = self.player.status.lock().unwrap();
                let v = (st.volume + 5).min(st.volume_max);
                drop(st);
                self.player.send_command(PlayerCommand::SetVolume(v));
            }
            WsEvent::VolumeDown => {
                let v = self.player.status.lock().unwrap().volume.saturating_sub(5);
                self.player.send_command(PlayerCommand::SetVolume(v));
            }
            WsEvent::SetMute(muted) => {
                self.mute_on = muted;
                self.player.send_command(PlayerCommand::SetMute(muted));
                self.save_prefs();
            }
            WsEvent::ToggleMute => {
                let muted = !self.player.status.lock().unwrap().muted;
                self.mute_on = muted;
                self.player.send_command(PlayerCommand::SetMute(muted));
                self.save_prefs();
            }
            WsEvent::SetAudio(index) => {
                self.player.send_command(PlayerCommand::SetAudio(index));
            }
            WsEvent::SetSub(index) => {
                let sid = self
                    .player
                    .status
                    .lock()
                    .unwrap()
                    .subtitle_stream_index_to_mpv_id(index);
                if let Some(sid) = sid {
                    self.player.send_command(PlayerCommand::SetSub(sid));
                }
            }
            WsEvent::UserDataChanged => {
                // The fetch runs synchronously (order-sensitive side
                // effects); the computed content travels to Model-owned
                // `home_content` via lib_tx (task 5.3d).
                if let Ok(content) = self.fetch_home() {
                    let _ = self
                        .lib_tx
                        .send(LibEvent::HomeContentRefreshed(Box::new(content)));
                }
            }
        }
    }
}
