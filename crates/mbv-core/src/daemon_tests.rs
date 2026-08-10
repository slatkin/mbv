use super::{
    all_audio, audio_only_rejection, broadcast, handle_ctrl, handle_ws,
    take_authority_for_emby_remote, AuthorityHolder, CtrlClients, CtrlEvent, CtrlOutbound,
    CtrlRequest, CtrlTransport, DaemonEvent, PlaybackIntentState, SharedQueueState,
};
use crate::api::EmbyItem;
use crate::config::{Config, QueueSource};
use crate::ctrl::DisconnectReason;
use crate::ctrl::{
    CtrlCmd, PlaybackIntent, PlaybackIntentAction, PlaybackIntentOutcome, WireCommand,
};
use crate::playback_queue::{FeedEntry, PlaybackQueue, QueueItem};
use crate::player::{Player, PlayerCommand, PlayerEvent, PlayerStatus, SubtitlePrefs};
use crate::ws::WsEvent;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

fn item(name: &str, media_type: &str, item_type: &str) -> EmbyItem {
    EmbyItem {
        id: name.into(),
        name: name.into(),
        item_type: item_type.into(),
        is_folder: false,
        media_type: media_type.into(),
        collection_type: String::new(),
        runtime_ticks: 0,
        played: false,
        playback_position_ticks: 0,
        series_id: String::new(),
        series_name: String::new(),
        album_id: String::new(),
        album: String::new(),
        index_number: 0,
        parent_index_number: 0,
        unplayed_item_count: 0,
        path: String::new(),
        artist: String::new(),
        sort_name: String::new(),
        production_year: 0,
        end_year: 0,
        overview: String::new(),
        premiere_date: String::new(),
        date_added: String::new(),
        total_count: 0,
        container: String::new(),
        director: String::new(),
        video_info: String::new(),
        audio_info: String::new(),
        genre: String::new(),
        playlist_item_id: String::new(),
    }
}

fn emby_qi(name: &str, media_type: &str, item_type: &str) -> QueueItem {
    QueueItem::Emby(Box::new(item(name, media_type, item_type)))
}

fn video_feed_qi(guid: &str) -> QueueItem {
    QueueItem::Feed(FeedEntry {
        guid: guid.into(),
        title: guid.into(),
        enclosure_url: None,
        link: None,
        mime_type: Some("video/mp4".into()),
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(crate::config::FeedKind::Video),
        feed_id: None,
        position_ticks: 0,
        played: false,
    })
}

#[test]
fn all_audio_accepts_audio_items() {
    assert!(all_audio(&[
        emby_qi("song1", "Audio", "Audio"),
        emby_qi("song2", "Audio", "Audio"),
    ]));
}

#[test]
fn all_audio_rejects_video_items() {
    assert!(!all_audio(&[
        emby_qi("song", "Audio", "Audio"),
        emby_qi("movie", "Video", "Movie"),
    ]));
}

#[test]
fn all_audio_rejects_video_feed_items() {
    assert!(!all_audio(&[video_feed_qi("feed-1")]));
}

#[test]
fn audio_only_daemon_rejects_video_feed_play_request() {
    let fetched = [video_feed_qi("feed-1")];
    let rejection = audio_only_rejection(true, &fetched);
    assert!(rejection.is_some_and(|r| !r.is_empty()));
}

#[test]
fn audio_only_daemon_rejects_non_audio_play_request() {
    let fetched = [emby_qi("movie", "Video", "Movie")];
    let rejection = audio_only_rejection(true, &fetched);
    assert!(rejection.is_some_and(|r| !r.is_empty()));
}

#[test]
fn audio_only_daemon_accepts_audio_play_request() {
    let fetched = [emby_qi("song", "Audio", "Audio")];
    assert!(audio_only_rejection(true, &fetched).is_none());
}

#[test]
fn non_audio_only_daemon_never_rejects() {
    let fetched = [emby_qi("movie", "Video", "Movie")];
    assert!(audio_only_rejection(false, &fetched).is_none());
}

/// Connects a client the same way the accept thread does.
fn connect_client(clients: &mut CtrlClients) -> (u64, mpsc::Receiver<CtrlOutbound>) {
    let (tx, rx) = mpsc::channel();
    let id = clients.connect(tx, CtrlTransport::Local, true, true);
    (id, rx)
}

fn shared_queue_state() -> SharedQueueState {
    SharedQueueState {
        queue: Arc::new(Mutex::new(PlaybackQueue::default())),
        source: Arc::new(Mutex::new(QueueSource::Unknown)),
    }
}

fn cold_player() -> Player {
    let (event_tx, _event_rx) = mpsc::channel::<PlayerEvent>();
    Player::new(
        String::new(),
        String::new(),
        false,
        false,
        true,
        false,
        false,
        SubtitlePrefs::default(),
        event_tx,
        None,
    )
}

fn recv_event(rx: &mpsc::Receiver<CtrlOutbound>) -> CtrlEvent {
    match rx.recv().unwrap() {
        CtrlOutbound::Event(json) => serde_json::from_str(&json).unwrap(),
        CtrlOutbound::Flush(_) => panic!("expected a control event"),
    }
}

/// Helper: builds a `PlaybackQueue` from a list of `EmbyItem`s with an active index.
fn queue_from_items(items: &[EmbyItem], active: usize) -> PlaybackQueue {
    let qi: Vec<QueueItem> = items
        .iter()
        .cloned()
        .map(|i| QueueItem::Emby(Box::new(i)))
        .collect();
    PlaybackQueue::from_queue_items(qi, Some(active))
}

#[test]
fn shutdown_notification_is_flushed_before_writers_are_released() {
    let mut clients = CtrlClients::default();
    let (_client_id, rx) = connect_client(&mut clients);
    let writer = std::thread::spawn(move || {
        match rx.recv().unwrap() {
            CtrlOutbound::Event(json) => {
                assert!(matches!(
                    serde_json::from_str::<CtrlEvent>(&json).unwrap(),
                    CtrlEvent::Disconnected {
                        reason: DisconnectReason::DaemonShutdown
                    }
                ));
            }
            CtrlOutbound::Flush(_) => panic!("shutdown event must precede the flush barrier"),
        }
        match rx.recv().unwrap() {
            CtrlOutbound::Flush(ack) => ack.send(()).unwrap(),
            CtrlOutbound::Event(_) => panic!("flush barrier must follow the shutdown event"),
        }
    });

    clients.notify_disconnected_all(DisconnectReason::DaemonShutdown);
    clients.flush_writers(Duration::from_secs(1));
    writer.join().unwrap();
}

#[test]
fn connecting_ctrl_client_becomes_driver_immediately() {
    let mut clients = CtrlClients::default();
    let (id, rx) = connect_client(&mut clients);
    assert!(clients.has_driver());
    assert!(clients.has_client(id));

    let registry = Arc::new(Mutex::new(clients));
    broadcast(
        &registry,
        &CtrlEvent::StatusOnly(PlayerStatus {
            volume: 55,
            ..PlayerStatus::default()
        }),
    );

    match recv_event(&rx) {
        CtrlEvent::StatusOnly(status) => assert_eq!(status.volume, 55),
        _ => panic!("expected status update"),
    }
}

#[test]
fn emby_remote_takeover_notifies_ctrl_client_and_keeps_connection() {
    let mut clients = CtrlClients::default();
    let (driver_id, driver_rx) = connect_client(&mut clients);
    assert!(clients.has_client(driver_id));

    clients.take_authority_for_emby_remote();

    match recv_event(&driver_rx) {
        CtrlEvent::Disconnected { reason } => {
            assert_eq!(reason, DisconnectReason::TakenOverByEmbyRemote);
        }
        _ => panic!("expected structured disconnect notification"),
    }
    assert!(clients.has_driver());
    assert_eq!(clients.authority, AuthorityHolder::EmbyRemote);
}

#[test]
fn ctrl_connect_during_emby_authority_does_not_override_authority() {
    let mut clients = CtrlClients::default();
    let (_old_id, old_rx) = connect_client(&mut clients);
    clients.take_authority_for_emby_remote();

    match recv_event(&old_rx) {
        CtrlEvent::Disconnected { reason } => {
            assert_eq!(reason, DisconnectReason::TakenOverByEmbyRemote);
        }
        _ => panic!("expected structured disconnect notification"),
    }
    assert_eq!(clients.authority, AuthorityHolder::EmbyRemote);

    let (new_id, new_rx) = connect_client(&mut clients);
    assert!(clients.has_client(new_id));
    assert_eq!(clients.authority, AuthorityHolder::EmbyRemote);

    let registry = Arc::new(Mutex::new(clients));
    broadcast(
        &registry,
        &CtrlEvent::StatusOnly(PlayerStatus {
            volume: 66,
            ..PlayerStatus::default()
        }),
    );

    match recv_event(&old_rx) {
        CtrlEvent::StatusOnly(status) => assert_eq!(status.volume, 66),
        _ => panic!("expected status update on old client"),
    }
    match recv_event(&new_rx) {
        CtrlEvent::StatusOnly(status) => assert_eq!(status.volume, 66),
        _ => panic!("expected status update on new client"),
    }
}

#[test]
fn emby_remote_takeover_without_ctrl_client_still_records_authority() {
    let mut clients = CtrlClients::default();

    clients.take_authority_for_emby_remote();

    assert!(!clients.has_driver());
    assert_eq!(clients.authority, AuthorityHolder::EmbyRemote);
}

#[test]
fn sole_client_disconnect_clears_registry_without_touching_playback() {
    let mut clients = CtrlClients::default();
    let (id, _rx) = connect_client(&mut clients);
    assert!(clients.has_driver());

    clients.remove(id);

    assert!(!clients.has_driver());
    assert!(!clients.has_client(id));
}

#[test]
fn cold_ctrl_player_command_keeps_connection_as_driver() {
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, _reply_rx) = mpsc::channel();
    let mut queue = PlaybackQueue::default();
    let mut source = QueueSource::Unknown;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::PlayerCmd(WireCommand::from(PlayerCommand::TogglePause)),
        1,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut queue,
        &mut source,
        &shared_queue_state(),
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    assert!(registry.lock().unwrap().has_driver());
    assert!(sender_rx.try_recv().is_err());
}

#[test]
fn adopt_queue_rejection_sends_authoritative_state_to_sole_client() {
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, _sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, reply_rx) = mpsc::channel();
    let mut queue = queue_from_items(&[item("existing", "Video", "Movie")], 0);
    let mut source = QueueSource::Remote;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::AdoptQueue {
            items: vec![item("stale", "Video", "Movie")],
            cursor: 0,
            source: QueueSource::Unknown,
        },
        1,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut queue,
        &mut source,
        &shared_queue_state(),
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    assert_eq!(queue.len(), 1);
    assert_eq!(queue.slots()[0].item.id(), "existing");
    match recv_event(&reply_rx) {
        CtrlEvent::CommandRejected(reason) => {
            assert_eq!(reason, "daemon already has a queue; adoption skipped");
        }
        _ => panic!("expected command rejection"),
    }
    match recv_event(&reply_rx) {
        CtrlEvent::State(state) => {
            assert_eq!(
                state
                    .items
                    .iter()
                    .map(|i| i.id.as_str())
                    .collect::<Vec<_>>(),
                vec!["existing"]
            );
            assert_eq!(state.cursor, 0);
        }
        _ => panic!("expected authoritative state resync"),
    }
}

#[test]
fn unified_adopt_queue_seeds_status_without_starting_playback_when_cold() {
    let player = cold_player();
    let player_cmd_rx = player.spy_on_commands();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (reply_tx, _reply_rx) = mpsc::channel();
    let mut queue = PlaybackQueue::default();
    let mut source = QueueSource::Unknown;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::UnifiedAdoptQueue {
            items: vec![emby_qi("adopted", "Video", "Movie")],
            cursor: 0,
            source: QueueSource::Remote,
        },
        1,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut queue,
        &mut source,
        &shared_queue_state(),
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    assert_eq!(queue.len(), 1);
    assert_eq!(queue.slots()[0].item.id(), "adopted");
    assert!(!player.status.lock().unwrap().active);
    assert!(player_cmd_rx.try_recv().is_err());
}

#[test]
fn unified_adopt_queue_rejection_sends_authoritative_state_to_sole_client() {
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, _sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, reply_rx) = mpsc::channel();
    let mut queue = queue_from_items(&[item("existing", "Video", "Movie")], 0);
    let mut source = QueueSource::Remote;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::UnifiedAdoptQueue {
            items: vec![emby_qi("stale", "Video", "Movie")],
            cursor: 0,
            source: QueueSource::Unknown,
        },
        1,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut queue,
        &mut source,
        &shared_queue_state(),
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    assert_eq!(queue.len(), 1);
    assert_eq!(queue.slots()[0].item.id(), "existing");
    match recv_event(&reply_rx) {
        CtrlEvent::CommandRejected(reason) => {
            assert_eq!(reason, "daemon already has a queue; adoption skipped");
        }
        _ => panic!("expected command rejection"),
    }
    match recv_event(&reply_rx) {
        CtrlEvent::State(state) => {
            assert_eq!(
                state
                    .items
                    .iter()
                    .map(|i| i.id.as_str())
                    .collect::<Vec<_>>(),
                vec!["existing"]
            );
            assert_eq!(state.cursor, 0);
        }
        _ => panic!("expected authoritative state resync"),
    }
}

#[test]
fn ctrl_queue_move_updates_authoritative_queue_and_broadcasts_state() {
    let player = cold_player();
    let player_cmd_rx = player.spy_on_commands();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, _reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    let mut queue = queue_from_items(
        &[
            item("item-0", "Video", "Movie"),
            item("item-1", "Video", "Movie"),
            item("item-2", "Video", "Movie"),
        ],
        1,
    );
    let mut source = QueueSource::Remote;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::PlayerCmd(WireCommand::from(PlayerCommand::QueueMove(1, 2))),
        1,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut queue,
        &mut source,
        &shared_queue,
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    assert!(matches!(
        player_cmd_rx.try_recv(),
        Ok(PlayerCommand::QueueMove(1, 2))
    ));
    assert_eq!(
        queue
            .slots()
            .iter()
            .map(|s| s.item.id())
            .collect::<Vec<_>>(),
        vec!["item-0", "item-2", "item-1"]
    );
    // Active slot follows the moved item (identity-based).
    assert_eq!(queue.active_index(), Some(2));
    // Reconnect snapshot updated.
    {
        let q = shared_queue.queue.lock().unwrap();
        assert_eq!(
            q.slots().iter().map(|s| s.item.id()).collect::<Vec<_>>(),
            vec!["item-0", "item-2", "item-1"]
        );
    }
    match recv_event(&sender_rx) {
        CtrlEvent::UnifiedQueueState(state) => {
            assert_eq!(
                state.slots.iter().map(|s| s.item.id()).collect::<Vec<_>>(),
                vec!["item-0", "item-2", "item-1"]
            );
        }
        _ => panic!("expected unified queue state update"),
    }
}

#[test]
fn ctrl_queue_append_updates_authoritative_queue_and_broadcasts_state() {
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, _reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    let mut queue = queue_from_items(
        &[
            item("item-0", "Video", "Movie"),
            item("item-1", "Video", "Movie"),
        ],
        1,
    );
    let mut source = QueueSource::Remote;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::PlayerCmd(WireCommand::from(PlayerCommand::QueueAppend {
            items: vec![QueueItem::Emby(Box::new(item("item-2", "Video", "Movie")))],
        })),
        1,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut queue,
        &mut source,
        &shared_queue,
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    assert_eq!(
        queue
            .slots()
            .iter()
            .map(|s| s.item.id())
            .collect::<Vec<_>>(),
        vec!["item-0", "item-1", "item-2"]
    );
    assert_eq!(queue.active_index(), Some(1));
    match recv_event(&sender_rx) {
        CtrlEvent::UnifiedQueueState(state) => {
            assert_eq!(
                state.slots.iter().map(|s| s.item.id()).collect::<Vec<_>>(),
                vec!["item-0", "item-1", "item-2"]
            );
        }
        _ => panic!("expected unified queue state update"),
    }
}

#[test]
fn ctrl_queue_remove_updates_authoritative_queue_and_broadcasts_state() {
    let player = cold_player();
    let player_cmd_rx = player.spy_on_commands();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, _reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    let mut queue = queue_from_items(
        &[
            item("item-0", "Video", "Movie"),
            item("item-1", "Video", "Movie"),
            item("item-2", "Video", "Movie"),
        ],
        1,
    );
    let mut source = QueueSource::Remote;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::PlayerCmd(WireCommand::from(PlayerCommand::QueueRemove(1))),
        1,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut queue,
        &mut source,
        &shared_queue,
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    assert!(matches!(
        player_cmd_rx.try_recv(),
        Ok(PlayerCommand::QueueRemove(1))
    ));
    assert_eq!(
        queue
            .slots()
            .iter()
            .map(|s| s.item.id())
            .collect::<Vec<_>>(),
        vec!["item-0", "item-2"]
    );
    // Active slot moved to successor after removal.
    assert_eq!(queue.active_index(), Some(1));
    match recv_event(&sender_rx) {
        CtrlEvent::UnifiedQueueState(state) => {
            assert_eq!(
                state.slots.iter().map(|s| s.item.id()).collect::<Vec<_>>(),
                vec!["item-0", "item-2"]
            );
        }
        _ => panic!("expected unified queue state update"),
    }
}

#[test]
fn stale_ctrl_queue_move_is_rejected_and_resyncs_sender() {
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    let mut queue = queue_from_items(
        &[
            item("item-0", "Video", "Movie"),
            item("item-1", "Video", "Movie"),
        ],
        1,
    );
    let mut source = QueueSource::Remote;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::PlayerCmd(WireCommand::from(PlayerCommand::QueueMove(1, 2))),
        1,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut queue,
        &mut source,
        &shared_queue,
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    // Queue unchanged.
    assert_eq!(
        queue
            .slots()
            .iter()
            .map(|s| s.item.id())
            .collect::<Vec<_>>(),
        vec!["item-0", "item-1"]
    );
    assert!(sender_rx.try_recv().is_err());
    match recv_event(&reply_rx) {
        CtrlEvent::CommandRejected(reason) => {
            assert_eq!(reason, "remote queue changed; move skipped");
        }
        _ => panic!("expected command rejection"),
    }
    match recv_event(&reply_rx) {
        CtrlEvent::State(state) => {
            assert_eq!(
                state
                    .items
                    .iter()
                    .map(|i| i.id.as_str())
                    .collect::<Vec<_>>(),
                vec!["item-0", "item-1"]
            );
            assert_eq!(state.cursor, 1);
        }
        _ => panic!("expected queue state resync"),
    }
}

#[test]
fn cold_websocket_noop_does_not_evict_ctrl_driver() {
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (driver_id, driver_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let mut queue = PlaybackQueue::default();
    let mut source = QueueSource::Unknown;

    handle_ws(
        WsEvent::TogglePause,
        &client,
        &player,
        false,
        &mut queue,
        &mut source,
        &shared_queue_state(),
        &registry,
    );

    let clients = registry.lock().unwrap();
    assert!(clients.has_client(driver_id));
    drop(clients);
    assert!(driver_rx.try_recv().is_err());
}

#[test]
fn websocket_takeover_helper_records_emby_remote_authority() {
    let registry = Arc::new(Mutex::new(CtrlClients::default()));

    take_authority_for_emby_remote(&registry);

    let clients = registry.lock().unwrap();
    assert!(!clients.has_driver());
    assert_eq!(clients.authority, AuthorityHolder::EmbyRemote);
}
