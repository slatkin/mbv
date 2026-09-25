use super::*;
use crate::api::EmbyClient;
use crate::playback_queue::{PlaybackQueue, QueueItem};
use crate::player::{Player, PlayerCommand};
use crate::ws::WsEvent;
use std::sync::{Arc, Mutex};

/// Start the remote play on the freshly replaced queue: a single item goes
/// through `play`, several through `play_queue` at the requested index, with
/// `start_position_ticks` overriding the item's own position when set.
fn start_remote_playback(
    player: &Player,
    client: &Arc<Mutex<EmbyClient>>,
    fetched: &[crate::api::EmbyItem],
    start_idx: usize,
    start_position_ticks: i64,
) {
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
        let mut items_with_pos = fetched.to_vec();
        items_with_pos[start_idx] = start_item;
        let c = Arc::new(client.lock().unwrap().clone());
        player.play_queue(items_with_pos, start_idx, c, 100);
    }
}

fn websocket_play_start_index(
    play_now: bool,
    fetched_len: usize,
    start_index: usize,
) -> Option<usize> {
    (play_now && fetched_len > 0).then(|| start_index.min(fetched_len - 1))
}

struct WsPlayContext<'a> {
    client: &'a Arc<Mutex<EmbyClient>>,
    player: &'a Player,
    audio_only: bool,
    queue: &'a mut PlaybackQueue,
    source: &'a mut crate::config::QueueSource,
    transitions: &'a mut crate::playback_transition::OwnerTransitionState,
    shared_queue: &'a SharedQueueState,
    ctrl_clients: &'a ClientRegistry,
}

fn handle_ws_play(event: WsEvent, context: WsPlayContext<'_>) {
    let WsEvent::Play {
        item_ids,
        play_now,
        start_position_ticks,
        start_index,
    } = event
    else {
        unreachable!("handle_ws_play only accepts Play events");
    };
    let WsPlayContext {
        client,
        player,
        audio_only,
        queue,
        source,
        transitions,
        shared_queue,
        ctrl_clients,
    } = context;
    if websocket_play_start_index(play_now, item_ids.len(), start_index).is_none() {
        return;
    }
    let fetched = {
        let c = client.lock().unwrap();
        match c.get_items_by_ids(&item_ids) {
            Ok(items) => items,
            Err(error) => {
                log::warn!(target: "daemon", "play error: {error}");
                return;
            }
        }
    };
    let Some(start_idx) = websocket_play_start_index(play_now, fetched.len(), start_index) else {
        return;
    };
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
    transitions.reset();
    broadcast_queue_state(
        ctrl_clients,
        player,
        shared_queue,
        queue,
        source,
        transitions,
    );
    start_remote_playback(player, client, &fetched, start_idx, start_position_ticks);
}

fn handle_ws_control(
    ev: WsEvent,
    player: &Player,
    queue: &PlaybackQueue,
    ctrl_clients: &ClientRegistry,
) {
    let changed = match ev {
        WsEvent::Stop => {
            player.stop();
            !queue.is_empty()
        }
        WsEvent::Pause => player.set_paused(true),
        WsEvent::Unpause => player.set_paused(false),
        WsEvent::NextTrack => player.next(),
        WsEvent::PreviousTrack => player.previous(),
        WsEvent::Seek(ticks) => {
            use crate::api::TICKS_PER_SECOND;
            player.send_command(PlayerCommand::SeekAbsolute(
                ticks as f64 / TICKS_PER_SECOND as f64,
            ))
        }
        WsEvent::TogglePause => player.send_command(PlayerCommand::TogglePause),
        WsEvent::SeekRelative(secs) => player.send_command(PlayerCommand::Seek(secs)),
        WsEvent::SetVolume(volume) => {
            let max = player.status.lock().unwrap().volume_max;
            player.send_command(PlayerCommand::SetVolume(volume.clamp(0, max)))
        }
        WsEvent::VolumeUp => {
            let status = player.status.lock().unwrap();
            let volume = (status.volume + 5).min(status.volume_max);
            drop(status);
            player.send_command(PlayerCommand::SetVolume(volume))
        }
        WsEvent::VolumeDown => {
            let volume = (player.status.lock().unwrap().volume - 5).max(0);
            player.send_command(PlayerCommand::SetVolume(volume))
        }
        WsEvent::SetMute(muted) => player.send_command(PlayerCommand::SetMute(muted)),
        WsEvent::ToggleMute => {
            let muted = !player.status.lock().unwrap().muted;
            player.send_command(PlayerCommand::SetMute(muted))
        }
        WsEvent::SetAudio(index) => player.send_command(PlayerCommand::SetAudio(index)),
        WsEvent::SetSub(index) => {
            let sid = player
                .status
                .lock()
                .unwrap()
                .subtitle_stream_index_to_mpv_id(index);
            if let Some(sid) = sid {
                player.send_command(PlayerCommand::SetSub(sid))
            } else {
                log::warn!(target: "daemon", "subtitle stream index {index} did not match any mpv subtitle track");
                false
            }
        }
        WsEvent::UserDataChanged => false,
        WsEvent::Play { .. } => unreachable!("Play is handled by handle_ws_play"),
    };
    if changed {
        take_authority_for_emby_remote(ctrl_clients);
    }
}

pub(crate) fn handle_ws(
    ev: WsEvent,
    client: Option<&Arc<Mutex<EmbyClient>>>,
    player: &Player,
    audio_only: bool,
    queue: &mut PlaybackQueue,
    source: &mut crate::config::QueueSource,
    transitions: &mut crate::playback_transition::OwnerTransitionState,
    shared_queue: &SharedQueueState,
    ctrl_clients: &ClientRegistry,
) {
    let Some(client) = client else {
        return;
    };
    match ev {
        event @ WsEvent::Play { .. } => handle_ws_play(
            event,
            WsPlayContext {
                client,
                player,
                audio_only,
                queue,
                source,
                transitions,
                shared_queue,
                ctrl_clients,
            },
        ),
        event => handle_ws_control(event, player, queue, ctrl_clients),
    }
}

pub(crate) fn all_audio<'a>(items: impl IntoIterator<Item = &'a QueueItem>) -> bool {
    items.into_iter().all(QueueItem::is_audio)
}

#[cfg(test)]
mod tests {
    use super::websocket_play_start_index;
    use rstest::rstest;

    #[rstest]
    #[case(false, 2, 0, None)]
    #[case(true, 0, 0, None)]
    #[case(true, 1, 9, Some(0))]
    #[case(true, 3, 1, Some(1))]
    #[case(true, 3, 9, Some(2))]
    fn websocket_play_start_index_clamps_to_fetched_queue(
        #[case] play_now: bool,
        #[case] fetched_len: usize,
        #[case] requested: usize,
        #[case] expected: Option<usize>,
    ) {
        assert_eq!(
            websocket_play_start_index(play_now, fetched_len, requested),
            expected
        );
    }
}
