fn handle_ws(
    ev: WsEvent,
    client: Option<&Arc<Mutex<EmbyClient>>>,
    player: &Player,
    audio_only: bool,
    queue: &mut PlaybackQueue,
    source: &mut crate::config::QueueSource,
    shared_queue: &SharedQueueState,
    ctrl_clients: &ClientRegistry,
) {
    let Some(client) = client else {
        return;
    };
    match ev {
        WsEvent::Play {
            item_ids,
            play_now,
            start_position_ticks,
            start_index,
        } => {
            if !play_now {
                return;
            }
            let fetched = {
                let c = client.lock().unwrap();
                match c.get_items_by_ids(&item_ids) {
                    Ok(v) => v,
                    Err(e) => {
                        log::warn!(target: "daemon", "play error: {e}");
                        return;
                    }
                }
            };
            if fetched.is_empty() {
                return;
            }
            let start_idx = start_index.min(fetched.len().saturating_sub(1));
            let queue_items: Vec<QueueItem> = fetched
                .iter()
                .cloned()
                .map(|item| QueueItem::Emby(Box::new(item)))
                .collect();
            if let Some(reason) = audio_only_rejection(audio_only, &queue_items) {
                log::warn!(target: "daemon", "rejecting websocket play request: {reason}");
                return;
            }
            *queue = PlaybackQueue::from_queue_items(queue_items, Some(start_idx));
            *source = crate::config::QueueSource::Remote;
            take_authority_for_emby_remote(ctrl_clients);
            broadcast_queue_state(ctrl_clients, player, shared_queue, queue, source);
            if fetched.len() == 1 {
                let mut play_item = fetched[0].clone();
                if start_position_ticks > 0 {
                    play_item.playback_position_ticks = start_position_ticks;
                }
                let c = Arc::new(client.lock().unwrap().clone());
                player.play(&play_item, c, 100);
            } else {
                let mut start_item = fetched[start_idx].clone();
                if start_position_ticks > 0 {
                    start_item.playback_position_ticks = start_position_ticks;
                }
                let mut items_with_pos = fetched.clone();
                items_with_pos[start_idx] = start_item;
                let c = Arc::new(client.lock().unwrap().clone());
                player.play_queue(items_with_pos, start_idx, c, 100);
            }
        }
        WsEvent::Stop => {
            player.stop();
            if !queue.is_empty() {
                take_authority_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::Pause => {
            if player.set_paused(true) {
                take_authority_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::Unpause => {
            if player.set_paused(false) {
                take_authority_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::NextTrack => {
            if player.next() {
                take_authority_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::PreviousTrack => {
            if player.previous() {
                take_authority_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::Seek(ticks) => {
            use crate::api::TICKS_PER_SECOND;
            if player.send_command(PlayerCommand::SeekAbsolute(
                ticks as f64 / TICKS_PER_SECOND as f64,
            )) {
                take_authority_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::TogglePause => {
            if player.send_command(PlayerCommand::TogglePause) {
                take_authority_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::SeekRelative(secs) => {
            if player.send_command(PlayerCommand::Seek(secs)) {
                take_authority_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::SetVolume(v) => {
            let vol_max = player.status.lock().unwrap().volume_max;
            if player.send_command(PlayerCommand::SetVolume(v.clamp(0, vol_max))) {
                take_authority_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::VolumeUp => {
            let st = player.status.lock().unwrap();
            let v = (st.volume + 5).min(st.volume_max);
            drop(st);
            if player.send_command(PlayerCommand::SetVolume(v)) {
                take_authority_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::VolumeDown => {
            let v = (player.status.lock().unwrap().volume - 5).max(0);
            if player.send_command(PlayerCommand::SetVolume(v)) {
                take_authority_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::SetMute(muted) => {
            if player.send_command(PlayerCommand::SetMute(muted)) {
                take_authority_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::ToggleMute => {
            let muted = !player.status.lock().unwrap().muted;
            if player.send_command(PlayerCommand::SetMute(muted)) {
                take_authority_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::SetAudio(index) => {
            if player.send_command(PlayerCommand::SetAudio(index)) {
                take_authority_for_emby_remote(ctrl_clients);
            }
        }
        WsEvent::SetSub(index) => {
            let sid = player
                .status
                .lock()
                .unwrap()
                .subtitle_stream_index_to_mpv_id(index);
            if let Some(sid) = sid {
                if player.send_command(PlayerCommand::SetSub(sid)) {
                    take_authority_for_emby_remote(ctrl_clients);
                }
            } else {
                log::warn!(target: "daemon", "subtitle stream index {index} did not match any mpv subtitle track");
            }
        }
        WsEvent::UserDataChanged => {}
    }
}

fn all_audio(items: &[QueueItem]) -> bool {
    items.iter().all(QueueItem::is_audio)
}
