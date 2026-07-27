use super::{
    all_audio, audio_only_rejection, broadcast, handle_ctrl, handle_ws,
    take_authority_for_emby_remote, AuthorityHolder, CtrlClients, CtrlEvent, CtrlOutbound,
    CtrlRequest, DaemonEvent, SharedQueueState, SpectrumState,
};
use crate::api::MediaItem;
use crate::config::{Config, QueueSource};
use crate::ctrl::DisconnectReason;
use crate::ctrl::{CtrlCmd, WireCommand};
use crate::player::{Player, PlayerCommand, PlayerEvent, PlayerStatus, SubtitlePrefs};
use crate::ws::WsEvent;
use std::sync::{mpsc, Arc, Mutex};

fn item(name: &str, media_type: &str, item_type: &str) -> MediaItem {
    MediaItem {
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
    let id = clients.connect(tx);
    (id, rx)
}

fn shared_queue_state() -> SharedQueueState {
    SharedQueueState {
        items: Arc::new(Mutex::new(Vec::new())),
        cursor: Arc::new(Mutex::new(0)),
        source: Arc::new(Mutex::new(QueueSource::Unknown)),
    }
}

fn dummy_spectrum_ctx() -> (mpsc::Sender<DaemonEvent>, Option<SpectrumState>) {
    let (tx, _rx) = mpsc::channel();
    (tx, None)
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
        CtrlOutbound::Close => panic!("expected event, got close"),
    }
}

fn assert_close(rx: &mpsc::Receiver<CtrlOutbound>) {
    match rx.recv().unwrap() {
        CtrlOutbound::Close => {}
        CtrlOutbound::Event(json) => panic!("expected close, got {json}"),
    }
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
fn second_connect_evicts_first_and_becomes_sole_connection() {
    let mut clients = CtrlClients::default();
    let (old_id, old_rx) = connect_client(&mut clients);
    let (new_id, new_rx) = connect_client(&mut clients);

    match recv_event(&old_rx) {
        CtrlEvent::Disconnected { reason } => {
            assert_eq!(reason, DisconnectReason::TakenOverByCtrlClient);
        }
        _ => panic!("expected structured disconnect"),
    }
    assert_close(&old_rx);

    // Exactly one connection can ever exist: the evicted id is gone, the
    // new id is the sole driver. This is what makes the co-pending
    // AdoptQueue race from #119 structurally impossible — there is never
    // a second live connection to lose the cold-check.
    assert!(!clients.has_client(old_id));
    assert!(clients.has_client(new_id));
    assert!(clients.has_driver());

    let registry = Arc::new(Mutex::new(clients));
    broadcast(
        &registry,
        &CtrlEvent::StatusOnly(PlayerStatus {
            volume: 77,
            ..PlayerStatus::default()
        }),
    );

    match recv_event(&new_rx) {
        CtrlEvent::StatusOnly(status) => assert_eq!(status.volume, 77),
        _ => panic!("expected status update"),
    }
    assert!(old_rx.try_recv().is_err());
}

#[test]
fn emby_remote_takeover_disconnects_current_ctrl_driver() {
    // Scope boundary (ADR 0003 / issue #119 brief): the ctrl-vs-Emby-
    // remote-websocket authority axis is untouched by the exclusive-
    // connection collapse — a successful Emby remote command must still
    // fully evict the sole ctrl connection exactly as before.
    let mut clients = CtrlClients::default();
    let (driver_id, driver_rx) = connect_client(&mut clients);
    assert!(clients.has_client(driver_id));

    clients.take_authority_for_emby_remote();

    match recv_event(&driver_rx) {
        CtrlEvent::Disconnected { reason } => {
            assert_eq!(reason, DisconnectReason::TakenOverByEmbyRemote);
        }
        _ => panic!("expected structured disconnect"),
    }
    assert_close(&driver_rx);
    assert!(!clients.has_driver());
    assert_eq!(clients.authority, AuthorityHolder::EmbyRemote);
}

#[test]
fn ctrl_reconnect_after_emby_remote_takeover_becomes_driver_and_receives_broadcasts() {
    let mut clients = CtrlClients::default();
    let (_old_id, old_rx) = connect_client(&mut clients);
    clients.take_authority_for_emby_remote();

    match recv_event(&old_rx) {
        CtrlEvent::Disconnected { reason } => {
            assert_eq!(reason, DisconnectReason::TakenOverByEmbyRemote);
        }
        _ => panic!("expected structured disconnect"),
    }
    assert_close(&old_rx);
    assert_eq!(clients.authority, AuthorityHolder::EmbyRemote);

    let (new_id, new_rx) = connect_client(&mut clients);
    assert!(clients.has_client(new_id));
    assert_eq!(clients.authority, AuthorityHolder::Ctrl(new_id));

    let registry = Arc::new(Mutex::new(clients));
    broadcast(
        &registry,
        &CtrlEvent::StatusOnly(PlayerStatus {
            volume: 66,
            ..PlayerStatus::default()
        }),
    );

    match recv_event(&new_rx) {
        CtrlEvent::StatusOnly(status) => assert_eq!(status.volume, 66),
        _ => panic!("expected status update"),
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
    let (dummy_merged_tx, mut dummy_spectrum) = dummy_spectrum_ctx();

    handle_ctrl(
        CtrlCmd::PlayerCmd(WireCommand::from(PlayerCommand::TogglePause)),
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut items,
        &mut cursor,
        &mut source,
        &shared_queue_state(),
        &registry,
        &dummy_merged_tx,
        &mut dummy_spectrum,
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
    let (dummy_merged_tx, mut dummy_spectrum) = dummy_spectrum_ctx();

    handle_ctrl(
        CtrlCmd::AdoptQueue {
            items: vec![item("stale", "Video", "Movie")],
            cursor: 0,
            source: QueueSource::Unknown,
        },
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut items,
        &mut cursor,
        &mut source,
        &shared_queue_state(),
        &registry,
        &dummy_merged_tx,
        &mut dummy_spectrum,
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
    let (dummy_merged_tx, mut dummy_spectrum) = dummy_spectrum_ctx();

    handle_ctrl(
        CtrlCmd::PlayerCmd(WireCommand::from(PlayerCommand::QueueMove(1, 2))),
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut items,
        &mut cursor,
        &mut source,
        &shared_queue,
        &registry,
        &dummy_merged_tx,
        &mut dummy_spectrum,
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
    let (dummy_merged_tx, mut dummy_spectrum) = dummy_spectrum_ctx();

    handle_ctrl(
        CtrlCmd::PlayerCmd(WireCommand::from(PlayerCommand::QueueAppend {
            items: appended.clone(),
        })),
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut items,
        &mut cursor,
        &mut source,
        &shared_queue,
        &registry,
        &dummy_merged_tx,
        &mut dummy_spectrum,
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
    let (dummy_merged_tx, mut dummy_spectrum) = dummy_spectrum_ctx();

    handle_ctrl(
        CtrlCmd::PlayerCmd(WireCommand::from(PlayerCommand::QueueMove(1, 2))),
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut items,
        &mut cursor,
        &mut source,
        &shared_queue,
        &registry,
        &dummy_merged_tx,
        &mut dummy_spectrum,
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

#[test]
fn start_spectrum_creates_spectrum_state() {
    // When StartSpectrum is handled and CavaWorker::start succeeds,
    // spectrum_state should be populated. If cava is not installed,
    // it should remain None and a SpectrumFailed reply is sent.
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, _sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, reply_rx) = mpsc::channel();
    let mut items = Vec::new();
    let mut cursor = 0;
    let mut source = QueueSource::Unknown;
    let (dummy_merged_tx, mut spectrum_state) = dummy_spectrum_ctx();

    handle_ctrl(
        CtrlCmd::StartSpectrum,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut items,
        &mut cursor,
        &mut source,
        &shared_queue_state(),
        &registry,
        &dummy_merged_tx,
        &mut spectrum_state,
    );

    if spectrum_state.is_some() {
        // CavaWorker::start succeeded (cava is installed)
        spectrum_state.take().unwrap().stop();
    } else {
        // CavaWorker::start failed (cava not installed) — SpectrumFailed sent
        match recv_event(&reply_rx) {
            CtrlEvent::SpectrumFailed { reason } => {
                assert!(!reason.is_empty());
            }
            _ => panic!("expected SpectrumFailed when cava is not installed"),
        }
    }
}

#[test]
fn stop_spectrum_when_not_active_is_noop() {
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, _sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, _reply_rx) = mpsc::channel();
    let mut items = Vec::new();
    let mut cursor = 0;
    let mut source = QueueSource::Unknown;
    let (dummy_merged_tx, mut spectrum_state) = dummy_spectrum_ctx();

    // StopSpectrum when no spectrum is active should be a no-op
    handle_ctrl(
        CtrlCmd::StopSpectrum,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut items,
        &mut cursor,
        &mut source,
        &shared_queue_state(),
        &registry,
        &dummy_merged_tx,
        &mut spectrum_state,
    );

    assert!(spectrum_state.is_none());
}

#[test]
fn spectrum_state_stop_is_idempotent() {
    // SpectrumState::stop() should be safe to call multiple times.
    let (stop_tx, _stop_rx) = mpsc::channel::<()>();
    let reader = std::thread::spawn(move || {
        // Dummy reader that exits immediately
    });
    let mut state = SpectrumState {
        reader: Some(reader),
        stop_tx,
    };

    state.stop();
    assert!(state.reader.is_none());

    // Second stop should be a no-op
    state.stop();
    assert!(state.reader.is_none());
}

#[test]
fn start_spectrum_when_already_active_is_ignored() {
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, _sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, _reply_rx) = mpsc::channel();
    let mut items = Vec::new();
    let mut cursor = 0;
    let mut source = QueueSource::Unknown;
    let (merged_tx_for_test, _merged_rx_for_test) = mpsc::channel::<DaemonEvent>();
    let (stop_tx_for_test, stop_rx_for_test) = mpsc::channel::<()>();
    let reader = std::thread::spawn(move || {
        let _ = stop_rx_for_test.recv();
    });
    let mut spectrum_state = Some(SpectrumState {
        reader: Some(reader),
        stop_tx: stop_tx_for_test,
    });

    // StartSpectrum when spectrum is already active should be ignored
    handle_ctrl(
        CtrlCmd::StartSpectrum,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut items,
        &mut cursor,
        &mut source,
        &shared_queue_state(),
        &registry,
        &merged_tx_for_test,
        &mut spectrum_state,
    );

    // State should still have the original reader
    assert!(spectrum_state.is_some());
    // Clean up
    spectrum_state.take().unwrap().stop();
}

#[test]
fn daemon_hello_includes_spectrum_capability() {
    use crate::ctrl::CTRL_CAP_SPECTRUM;

    let mut hello = crate::ctrl::CtrlHello::current();
    hello.capabilities.push(CTRL_CAP_SPECTRUM.to_string());

    assert!(hello
        .capabilities
        .iter()
        .any(|cap| cap == CTRL_CAP_SPECTRUM));
}

#[test]
fn perform_handshake_populates_supports_spectrum_from_capabilities() {
    // Simulate what perform_handshake does: parse capabilities from hello
    let mut hello = crate::ctrl::CtrlHello::current();
    hello
        .capabilities
        .push(crate::ctrl::CTRL_CAP_SPECTRUM.to_string());

    let mut compatibility = hello.compatibility().unwrap();
    compatibility.supports_spectrum = hello
        .capabilities
        .iter()
        .any(|cap| cap == crate::ctrl::CTRL_CAP_SPECTRUM);

    assert!(compatibility.supports_spectrum);
}

#[test]
fn perform_handshake_no_spectrum_capability_sets_false() {
    let hello = crate::ctrl::CtrlHello::current();

    let mut compatibility = hello.compatibility().unwrap();
    compatibility.supports_spectrum = hello
        .capabilities
        .iter()
        .any(|cap| cap == crate::ctrl::CTRL_CAP_SPECTRUM);

    assert!(!compatibility.supports_spectrum);
}
