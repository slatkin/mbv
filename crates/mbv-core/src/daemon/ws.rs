use super::{
    audio_only_rejection, broadcast_queue_state, take_authority_for_emby_remote, ClientRegistry,
    SharedQueueState,
};
use crate::api::EmbyClient;
use crate::playback_queue::{PlaybackQueue, QueueItem};
use crate::player::{Player, PlayerCommand};
use mbv_ws::WsEvent;
use std::sync::{Arc, Mutex};

/// Start the remote play on the freshly replaced queue: a single item goes
/// through `play`, several through `play_queue` at the requested index, with
/// `start_position_ticks` overriding the item's own position when set.
trait RemotePlayback {
    fn play(&self, item: &crate::api::EmbyItem, client: Arc<EmbyClient>);
    fn play_queue(
        &self,
        items: Vec<crate::api::EmbyItem>,
        start_idx: usize,
        client: Arc<EmbyClient>,
    );
}

impl RemotePlayback for Player {
    fn play(&self, item: &crate::api::EmbyItem, client: Arc<EmbyClient>) {
        Player::play(self, item, client, 100);
    }

    fn play_queue(
        &self,
        items: Vec<crate::api::EmbyItem>,
        start_idx: usize,
        client: Arc<EmbyClient>,
    ) {
        Player::play_queue(self, items, start_idx, client, 100);
    }
}

fn start_remote_playback(
    player: &dyn RemotePlayback,
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
        player.play(&play_item, c);
    } else {
        let mut start_item = fetched[start_idx].clone();
        if start_position_ticks > 0 {
            start_item.playback_position_ticks = start_position_ticks;
        }
        let mut items_with_pos = fetched.to_vec();
        items_with_pos[start_idx] = start_item;
        let c = Arc::new(client.lock().unwrap().clone());
        player.play_queue(items_with_pos, start_idx, c);
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
    playback: &'a dyn RemotePlayback,
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
        playback,
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
    start_remote_playback(playback, client, &fetched, start_idx, start_position_ticks);
}

#[expect(
    clippy::cast_precision_loss,
    reason = "seek target ticks → seconds through f64; no lossless integer-path conversion exists (approved, issue #804)"
)]
fn handle_ws_control(
    ev: &WsEvent,
    player: &Player,
    queue: &PlaybackQueue,
    ctrl_clients: &ClientRegistry,
) {
    let changed = match *ev {
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
                playback: player,
            },
        ),
        event => handle_ws_control(&event, player, queue, ctrl_clients),
    }
}

pub(crate) fn all_audio<'a>(items: impl IntoIterator<Item = &'a QueueItem>) -> bool {
    items.into_iter().all(QueueItem::is_audio)
}

#[cfg(test)]
mod tests {
    use super::{handle_ws_play, websocket_play_start_index, RemotePlayback, WsPlayContext};
    use crate::api::{EmbyClient, EmbyItem};
    use crate::config::{Config, QueueSource};
    use crate::daemon::{CtrlClients, SharedQueueState};
    use crate::playback_queue::{PlaybackQueue, QueueItem};
    use crate::playback_transition::OwnerTransitionState;
    use crate::player::Player;
    use mbv_net::mock_http::MockHttp;
    use mbv_ws::WsEvent;
    use rstest::rstest;
    use std::sync::{Arc, Mutex};

    #[derive(Debug, PartialEq)]
    enum PlayCall {
        Single(String, i64),
        Queue(Vec<(String, i64)>, usize),
    }

    #[derive(Default)]
    struct MockPlayback(Mutex<Vec<PlayCall>>);

    impl RemotePlayback for MockPlayback {
        fn play(&self, item: &EmbyItem, _client: Arc<EmbyClient>) {
            self.0.lock().unwrap().push(PlayCall::Single(
                item.id.clone(),
                item.playback_position_ticks,
            ));
        }

        fn play_queue(&self, items: Vec<EmbyItem>, start_idx: usize, _client: Arc<EmbyClient>) {
            self.0.lock().unwrap().push(PlayCall::Queue(
                items
                    .into_iter()
                    .map(|item| (item.id, item.playback_position_ticks))
                    .collect(),
                start_idx,
            ));
        }
    }

    fn play_context<'a>(
        client: &'a Arc<Mutex<EmbyClient>>,
        player: &'a Player,
        audio_only: bool,
        queue: &'a mut PlaybackQueue,
        source: &'a mut QueueSource,
        transitions: &'a mut OwnerTransitionState,
        shared_queue: &'a SharedQueueState,
        ctrl_clients: &'a Arc<Mutex<CtrlClients>>,
        playback: &'a dyn RemotePlayback,
    ) -> WsPlayContext<'a> {
        WsPlayContext {
            client,
            player,
            audio_only,
            queue,
            source,
            transitions,
            shared_queue,
            ctrl_clients,
            playback,
        }
    }

    fn mock_client(http: &MockHttp) -> Arc<Mutex<EmbyClient>> {
        let mut client = EmbyClient::new(Config {
            server_url: "http://127.0.0.1:1".into(),
            ..Config::default()
        })
        .with_test_agent(http.agent());
        client.user_id = "user".into();
        client.token = "test".into();
        Arc::new(Mutex::new(client))
    }

    fn items_response(items: &[(&str, &str, &str)]) -> String {
        serde_json::json!({
            "Items": items.iter().map(|(id, kind, media)| serde_json::json!({
                "Id": id,
                "Name": id,
                "Type": kind,
                "MediaType": media,
                "UserData": {"PlaybackPositionTicks": 7}
            })).collect::<Vec<_>>(),
            "TotalRecordCount": items.len()
        })
        .to_string()
    }

    #[rstest]
    #[case(vec![("a", "Audio", "Audio")], 9, 77, vec![PlayCall::Single("a".into(), 77)], 1, 0)]
    #[case(vec![("a", "Audio", "Audio"), ("b", "Audio", "Audio")], 9, 77, vec![PlayCall::Queue(vec![("a".into(), 7), ("b".into(), 77)], 1)], 2, 1)]
    fn play_fetches_items_and_dispatches_clamped_start(
        #[case] items: Vec<(&str, &str, &str)>,
        #[case] requested_index: usize,
        #[case] start_position: i64,
        #[case] expected_calls: Vec<PlayCall>,
        #[case] expected_len: usize,
        #[case] expected_active: usize,
    ) {
        let http = MockHttp::new();
        http.respond(200, &items_response(&items));
        let client = mock_client(&http);
        let player = crate::daemon::tests::cold_player();
        let playback = MockPlayback::default();
        let mut queue = PlaybackQueue::default();
        let mut source = QueueSource::Unknown;
        let mut transitions = OwnerTransitionState::default();
        let shared = crate::daemon::tests::shared_queue_state();
        let clients = Arc::new(Mutex::new(CtrlClients::default()));
        let ids: Vec<String> = items.iter().map(|(id, _, _)| (*id).to_string()).collect();

        assert!(!ids.is_empty());
        handle_ws_play(
            WsEvent::Play {
                item_ids: ids,
                play_now: true,
                start_position_ticks: start_position,
                start_index: requested_index,
            },
            play_context(
                &client,
                &player,
                false,
                &mut queue,
                &mut source,
                &mut transitions,
                &shared,
                &clients,
                &playback,
            ),
        );

        assert_eq!(http.request_count(), 1, "request: {:?}", http.requests());
        assert_eq!(playback.0.lock().unwrap().as_slice(), expected_calls);
        assert_eq!(queue.len(), expected_len);
        assert_eq!(queue.active_index(), Some(expected_active));
        assert_eq!(source, QueueSource::Remote);
    }

    #[test]
    fn audio_only_play_rejection_preserves_the_current_queue() {
        let http = MockHttp::new();
        http.respond(200, &items_response(&[("video", "Movie", "Video")]));
        let client = mock_client(&http);
        let player = crate::daemon::tests::cold_player();
        let playback = MockPlayback::default();
        let mut queue = PlaybackQueue::from_queue_items(
            vec![QueueItem::Emby(Box::new(crate::daemon::tests::item(
                "existing", "Audio", "Audio",
            )))],
            Some(0),
        );
        let previous_slots = queue
            .slots()
            .iter()
            .map(|slot| slot.slot_id)
            .collect::<Vec<_>>();
        let mut source = QueueSource::Album;
        let mut transitions = OwnerTransitionState::default();
        let shared = crate::daemon::tests::shared_queue_state();
        let clients = Arc::new(Mutex::new(CtrlClients::default()));

        handle_ws_play(
            WsEvent::Play {
                item_ids: vec!["video".into()],
                play_now: true,
                start_position_ticks: 0,
                start_index: 0,
            },
            play_context(
                &client,
                &player,
                true,
                &mut queue,
                &mut source,
                &mut transitions,
                &shared,
                &clients,
                &playback,
            ),
        );

        assert_eq!(http.request_count(), 1);
        assert!(playback.0.lock().unwrap().is_empty());
        assert_eq!(
            queue
                .slots()
                .iter()
                .map(|slot| slot.slot_id)
                .collect::<Vec<_>>(),
            previous_slots
        );
        assert_eq!(source, QueueSource::Album);
    }

    #[test]
    fn fetch_failure_does_not_replace_or_play_the_current_queue() {
        let http = MockHttp::new();
        http.fail(std::io::ErrorKind::ConnectionRefused);
        let client = mock_client(&http);
        let player = crate::daemon::tests::cold_player();
        let playback = MockPlayback::default();
        let mut queue = PlaybackQueue::from_queue_items(
            vec![QueueItem::Emby(Box::new(crate::daemon::tests::item(
                "existing", "Audio", "Audio",
            )))],
            Some(0),
        );
        let mut source = QueueSource::Album;
        let mut transitions = OwnerTransitionState::default();
        let shared = crate::daemon::tests::shared_queue_state();
        let clients = Arc::new(Mutex::new(CtrlClients::default()));

        handle_ws_play(
            WsEvent::Play {
                item_ids: vec!["missing".into()],
                play_now: true,
                start_position_ticks: 0,
                start_index: 0,
            },
            play_context(
                &client,
                &player,
                false,
                &mut queue,
                &mut source,
                &mut transitions,
                &shared,
                &clients,
                &playback,
            ),
        );

        assert_eq!(http.request_count(), 1);
        assert!(playback.0.lock().unwrap().is_empty());
        assert_eq!(queue.slots()[0].item.id(), "existing");
        assert_eq!(source, QueueSource::Album);
    }

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
