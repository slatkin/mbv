use super::{
    all_audio, audio_only_rejection, broadcast, handle_ctrl, handle_feed_consumed, handle_ws,
    take_authority_for_emby_remote, AuthorityHolder, CtrlClients, CtrlEvent, CtrlOutbound,
    CtrlRequest, CtrlTransport, DaemonEvent, PlaybackIntentState, SharedQueueState,
};
use crate::api::EmbyItem;
use crate::config::{Config, QueueSource};
use crate::ctrl::DisconnectReason;
use crate::ctrl::{
    CtrlCmd, PlaybackIntent, PlaybackIntentAction, PlaybackIntentOutcome, WireCommand,
};
use crate::playback_queue::FeedEntry;
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

#[test]
fn all_audio_accepts_audio_items() {
    assert!(all_audio(&[
        item("song1", "Audio", "Audio"),
        item("song2", "Audio", "Audio"),
    ]));
}

#[test]
fn all_audio_rejects_video_items() {
    assert!(!all_audio(&[
        item("song", "Audio", "Audio"),
        item("movie", "Video", "Movie"),
    ]));
}

#[test]
fn audio_only_daemon_rejects_non_audio_play_request() {
    let fetched = [item("movie", "Video", "Movie")];
    let rejection = audio_only_rejection(true, &fetched);
    assert!(rejection.is_some_and(|r| !r.is_empty()));
}

#[test]
fn audio_only_daemon_accepts_audio_play_request() {
    let fetched = [item("song", "Audio", "Audio")];
    assert!(audio_only_rejection(true, &fetched).is_none());
}

#[test]
fn non_audio_only_daemon_never_rejects() {
    let fetched = [item("movie", "Video", "Movie")];
    assert!(audio_only_rejection(false, &fetched).is_none());
}

/// Connects a client the same way the accept thread does. Under the
/// exclusive-connection model (ADR 0003 / #119) connecting *is* becoming
/// the driver — there is no separate "pending" step.
fn connect_client(clients: &mut CtrlClients) -> (u64, mpsc::Receiver<CtrlOutbound>) {
    let (tx, rx) = mpsc::channel();
    let id = clients.connect(tx, CtrlTransport::Local, true);
    (id, rx)
}

fn shared_queue_state() -> SharedQueueState {
    SharedQueueState {
        items: Arc::new(Mutex::new(Vec::new())),
        cursor: Arc::new(Mutex::new(0)),
        source: Arc::new(Mutex::new(QueueSource::Unknown)),
        feed_items: Arc::new(Mutex::new(Vec::new())),
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
    // Connecting *is* the takeover (ADR 0003): there is no window where a
    // client is attached but not yet driving, so a broadcast reaches a
    // freshly connected client with no separate "take over" step.
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
    // Emby remote authority is a notification, not a connection close.
    // The ctrl client stays connected and receives broadcasts, but its
    // commands are rejected until authority returns to Ctrl.
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
    // Connection stays open — no Close sent, client remains registered
    assert!(clients.has_driver());
    assert_eq!(clients.authority, AuthorityHolder::EmbyRemote);
}

#[test]
fn ctrl_connect_during_emby_authority_does_not_override_authority() {
    // Connecting a new ctrl client during Emby remote authority does NOT
    // override authority. The new client receives broadcasts but its
    // commands are rejected until authority returns to Ctrl.
    let mut clients = CtrlClients::default();
    let (_old_id, old_rx) = connect_client(&mut clients);
    clients.take_authority_for_emby_remote();

    match recv_event(&old_rx) {
        CtrlEvent::Disconnected { reason } => {
            assert_eq!(reason, DisconnectReason::TakenOverByEmbyRemote);
        }
        _ => panic!("expected structured disconnect notification"),
    }
    // Old client stays connected — notification only, no close
    assert_eq!(clients.authority, AuthorityHolder::EmbyRemote);

    let (new_id, new_rx) = connect_client(&mut clients);
    assert!(clients.has_client(new_id));
    // Authority stays EmbyRemote — connect does NOT override
    assert_eq!(clients.authority, AuthorityHolder::EmbyRemote);

    let registry = Arc::new(Mutex::new(clients));
    broadcast(
        &registry,
        &CtrlEvent::StatusOnly(PlayerStatus {
            volume: 66,
            ..PlayerStatus::default()
        }),
    );

    // Both clients should receive the broadcast
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
    // Daemon contract (CONTEXT.md): disconnecting the sole ctrl
    // connection must not stop playback or drop the queue. `CtrlClients`
    // holds no player/queue state at all, so `remove` can only ever
    // affect the connection registry — this test pins that shape so a
    // future change can't accidentally couple the two.
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
    let mut items = Vec::new();
    let mut cursor = 0;
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
        &mut items,
        &mut cursor,
        &mut source,
        &mut Vec::new(),
        &shared_queue_state(),
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    // The connection was already the driver from connect-time, so a
    // no-op command on a cold player neither promotes nor evicts anyone.
    assert!(registry.lock().unwrap().has_driver());
    assert!(sender_rx.try_recv().is_err());
}

#[test]
fn adopt_queue_rejection_sends_authoritative_state_to_sole_client() {
    // Residual guard (ADR 0003): AdoptQueue can still be rejected if the
    // Emby remote-control websocket warmed the daemon between the sole
    // client's baseline read and its adopt command. Because there is
    // exactly one connection to reconcile, the daemon must push its
    // authoritative State alongside the rejection so that client
    // conforms instead of lingering on its optimistic mutation.
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, _sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, reply_rx) = mpsc::channel();
    let mut items = vec![item("existing", "Video", "Movie")];
    let mut cursor = 0;
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
        &mut items,
        &mut cursor,
        &mut source,
        &mut Vec::new(),
        &shared_queue_state(),
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    // The daemon's real queue is untouched by the rejected adoption.
    assert_eq!(
        items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        vec!["existing"]
    );
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
    let mut items = vec![
        item("item-0", "Video", "Movie"),
        item("item-1", "Video", "Movie"),
        item("item-2", "Video", "Movie"),
    ];
    let mut cursor = 1;
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
        &mut items,
        &mut cursor,
        &mut source,
        &mut Vec::new(),
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
        items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        vec!["item-0", "item-2", "item-1"]
    );
    assert_eq!(cursor, 2);
    assert_eq!(
        shared_queue
            .items
            .lock()
            .unwrap()
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec!["item-0", "item-2", "item-1"]
    );
    assert_eq!(*shared_queue.cursor.lock().unwrap(), 2);
    match recv_event(&sender_rx) {
        CtrlEvent::State(state) => {
            assert_eq!(
                state
                    .items
                    .iter()
                    .map(|i| i.id.as_str())
                    .collect::<Vec<_>>(),
                vec!["item-0", "item-2", "item-1"]
            );
            assert_eq!(state.cursor, 2);
        }
        _ => panic!("expected queue state update"),
    }
}

#[test]
fn ctrl_queue_append_updates_authoritative_queue_and_broadcasts_state() {
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
    let mut items = vec![
        item("item-0", "Video", "Movie"),
        item("item-1", "Video", "Movie"),
    ];
    let mut cursor = 1;
    let mut source = QueueSource::Remote;
    let appended = vec![item("item-2", "Video", "Movie")];
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::PlayerCmd(WireCommand::from(PlayerCommand::QueueAppend {
            items: appended.clone(),
        })),
        1,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut items,
        &mut cursor,
        &mut source,
        &mut Vec::new(),
        &shared_queue,
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    assert!(matches!(
        player_cmd_rx.try_recv(),
        Ok(PlayerCommand::QueueAppend { items })
            if items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>()
                == appended.iter().map(|i| i.id.as_str()).collect::<Vec<_>>()
    ));
    assert_eq!(
        items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        vec!["item-0", "item-1", "item-2"]
    );
    assert_eq!(cursor, 1);
    assert_eq!(
        shared_queue
            .items
            .lock()
            .unwrap()
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec!["item-0", "item-1", "item-2"]
    );
    assert_eq!(*shared_queue.cursor.lock().unwrap(), 1);
    match recv_event(&sender_rx) {
        CtrlEvent::State(state) => {
            assert_eq!(
                state
                    .items
                    .iter()
                    .map(|i| i.id.as_str())
                    .collect::<Vec<_>>(),
                vec!["item-0", "item-1", "item-2"]
            );
            assert_eq!(state.cursor, 1);
        }
        _ => panic!("expected queue state update"),
    }
}

#[test]
fn ctrl_queue_remove_updates_authoritative_queue_and_broadcasts_state() {
    // A direct mbv controller sends QueueRemove over the ctrl wire protocol
    // after removing an item from its Remote-scope queue. The daemon's queue
    // is authoritative: it must make the same removal and broadcast the
    // resulting State so every controller reconciles to the same queue.
    // Removing the QueueRemove receiver arm must make this fail.
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
    let mut items = vec![
        item("item-0", "Video", "Movie"),
        item("item-1", "Video", "Movie"),
        item("item-2", "Video", "Movie"),
    ];
    // Removing the selected item keeps the same index when its successor
    // shifts into that position.
    let mut cursor = 1;
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
        &mut items,
        &mut cursor,
        &mut source,
        &mut Vec::new(),
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
        items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        vec!["item-0", "item-2"]
    );
    assert_eq!(cursor, 1);
    assert_eq!(
        shared_queue
            .items
            .lock()
            .unwrap()
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec!["item-0", "item-2"]
    );
    assert_eq!(*shared_queue.cursor.lock().unwrap(), 1);
    match recv_event(&sender_rx) {
        CtrlEvent::State(state) => {
            assert_eq!(
                state
                    .items
                    .iter()
                    .map(|i| i.id.as_str())
                    .collect::<Vec<_>>(),
                vec!["item-0", "item-2"]
            );
            assert_eq!(state.cursor, 1);
        }
        _ => panic!("expected queue state update"),
    }
}

#[test]
fn stale_ctrl_queue_move_is_rejected_and_resyncs_sender() {
    let player = cold_player();
    let player_cmd_rx = player.spy_on_commands();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    let mut items = vec![
        item("item-0", "Video", "Movie"),
        item("item-1", "Video", "Movie"),
    ];
    let mut cursor = 1;
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
        &mut items,
        &mut cursor,
        &mut source,
        &mut Vec::new(),
        &shared_queue,
        &registry,
        &mut PlaybackIntentState::default(),
        None,
        &dummy_merged_tx,
    );

    assert!(player_cmd_rx.try_recv().is_err());
    assert_eq!(
        items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
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
    let mut items = Vec::new();
    let mut cursor = 0;
    let mut source = QueueSource::Unknown;

    handle_ws(
        WsEvent::TogglePause,
        &client,
        &player,
        false,
        &mut items,
        &mut cursor,
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
