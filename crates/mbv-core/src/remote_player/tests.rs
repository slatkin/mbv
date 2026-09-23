use super::*;
use crate::config::QueueSource;
use crate::playback_queue::QueueItem;

fn make_media_item(id: &str) -> EmbyItem {
    EmbyItem {
        id: id.into(),
        name: "Test Item".into(),
        item_type: "Episode".into(),
        is_folder: false,
        child_count: None,
        media_type: "Video".into(),
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
        artist_items: Vec::new(),
        sort_name: String::new(),
        production_year: 0,
        end_year: 0,
        overview: String::new(),
        premiere_date: String::new(),
        date_added: String::new(),
        total_count: 0,
        container: String::new(),
        video_info: String::new(),
        audio_info: String::new(),
        genres: Vec::new(),
        people: Vec::new(),
        external_urls: Vec::new(),
        playlist_item_id: String::new(),
        image_tags: Default::default(),
    }
}

fn status_with_idx(current_idx: usize) -> PlayerStatus {
    status_with_idx_and_len(current_idx, 0)
}

fn status_with_idx_and_len(current_idx: usize, queue_len: usize) -> PlayerStatus {
    RemotePlayer::stub_status(current_idx, queue_len)
}

fn connected_pair_for_disconnect_test() -> (
    RemotePlayer,
    mpsc::Receiver<PlayerEvent>,
    UnixStream,
) {
    use std::io::{BufRead, BufReader, Write};

    let (client, daemon) = UnixStream::pair().unwrap();
    let (daemon_tx, daemon_rx) = mpsc::channel();
    let peer = std::thread::spawn(move || {
        let mut writer = daemon.try_clone().unwrap();
        let mut reader = BufReader::new(daemon);
        let hello = serde_json::to_string(&CtrlEvent::Hello(CtrlHello::current())).unwrap();
        writeln!(writer, "{hello}").unwrap();
        let mut client_hello = String::new();
        reader.read_line(&mut client_hello).unwrap();
        let state = CtrlEvent::UnifiedQueueState(UnifiedQueueStateData {
            status: PlayerStatus::default(),
            slots: Vec::new(),
            active_slot: None,
            revision: 0,
            source: QueueSource::Unknown,
            in_flight_transition: None,
            queued_latest_transition: None,
        });
        writeln!(writer, "{}", serde_json::to_string(&state).unwrap()).unwrap();
        daemon_tx.send(writer).unwrap();
    });
    let (remote, events) = connect_stream(SocketStream::Unix(client)).unwrap();
    let daemon = daemon_rx.recv().unwrap();
    peer.join().unwrap();
    (remote, events, daemon)
}

#[test]
fn unsupported_idle_queue_load_is_rejected_without_staging_local_queue() {
    let (remote, _events, commands) = RemotePlayer::stub_with_command_rx(
        vec![make_media_item("confirmed")],
        0,
    );
    let result = remote.load_queue_idle(
        9,
        vec![],
        0,
        QueueSource::Album,
    );
    assert!(result.is_err());
    assert!(matches!(commands.try_recv(), Err(mpsc::TryRecvError::Empty)));
    assert_eq!(remote.items.lock().unwrap()[0].id, "confirmed");
    assert_eq!(*remote.queue_source.lock().unwrap(), QueueSource::Unknown);
    assert_eq!(remote.status.lock().unwrap().queue_len, 1);
}

#[test]
fn supported_idle_queue_load_sends_correlated_request_without_staging_queue() {
    let (mut remote, _events, commands) = RemotePlayer::stub_with_command_rx(
        vec![make_media_item("confirmed")],
        0,
    );
    remote.ctrl_compatibility.supports_owner_queue_load = true;
    remote
        .load_queue_idle(31, vec![], 0, QueueSource::Album)
        .unwrap();
    assert!(matches!(commands.try_recv(), Ok(CtrlCmd::UnifiedQueueLoadIdle { request_id: 31, .. })));
    assert_eq!(remote.items.lock().unwrap()[0].id, "confirmed");
    assert_eq!(*remote.queue_source.lock().unwrap(), QueueSource::Unknown);
    assert_eq!(remote.status.lock().unwrap().queue_len, 1);
}

#[test]
fn failed_ctrl_write_marks_remote_disconnected_and_rejects_later_commands() {
    use std::net::Shutdown;
    use std::time::Duration;

    let (remote, events, daemon) = connected_pair_for_disconnect_test();
    daemon.shutdown(Shutdown::Read).unwrap();
    assert!(!remote.is_disconnected());
    assert!(remote.send_ctrl_cmd(CtrlCmd::Stop));
    assert!(matches!(
        events.recv_timeout(Duration::from_secs(2)).unwrap(),
        PlayerEvent::RemoteDisconnected(message)
            if message == crate::player::CONNECTION_LOST_MESSAGE
    ));
    assert!(remote.is_disconnected());
    assert!(!remote.send_ctrl_cmd(CtrlCmd::Stop));
}

#[test]
fn failed_writer_write_emits_connection_lost_once() {
    use std::net::Shutdown;
    use std::time::Duration;

    let (remote, events, daemon) = connected_pair_for_disconnect_test();
    daemon.shutdown(Shutdown::Read).unwrap();
    assert!(remote.send_ctrl_cmd(CtrlCmd::Stop));

    assert!(matches!(
        events.recv_timeout(Duration::from_secs(2)).unwrap(),
        PlayerEvent::RemoteDisconnected(message)
            if message == crate::player::CONNECTION_LOST_MESSAGE
    ));
    assert!(remote.is_disconnected());
    assert!(matches!(events.try_recv(), Err(mpsc::TryRecvError::Empty)));
}

#[test]
fn reader_eof_emits_connection_lost_instead_of_stopped() {
    use std::net::Shutdown;
    use std::time::Duration;

    let (remote, events, daemon) = connected_pair_for_disconnect_test();
    daemon.shutdown(Shutdown::Write).unwrap();

    assert!(matches!(
        events.recv_timeout(Duration::from_secs(2)).unwrap(),
        PlayerEvent::RemoteDisconnected(message)
            if message == crate::player::CONNECTION_LOST_MESSAGE
    ));
    assert!(remote.is_disconnected());
    assert!(matches!(events.try_recv(), Err(mpsc::TryRecvError::Empty)));
}

#[test]
fn writer_and_reader_loss_emit_only_one_disconnect_event() {
    use std::net::Shutdown;
    use std::time::Duration;

    let (remote, events, daemon) = connected_pair_for_disconnect_test();
    daemon.shutdown(Shutdown::Read).unwrap();
    assert!(remote.send_ctrl_cmd(CtrlCmd::Stop));
    daemon.shutdown(Shutdown::Write).unwrap();

    assert!(matches!(
        events.recv_timeout(Duration::from_secs(2)).unwrap(),
        PlayerEvent::RemoteDisconnected(message)
            if message == crate::player::CONNECTION_LOST_MESSAGE
    ));
    assert!(remote.is_disconnected());
    assert!(matches!(
        events.recv_timeout(Duration::from_secs(2)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
}

#[test]
fn announced_shutdown_emits_only_its_dedicated_event() {
    use std::io::Write;
    use std::net::Shutdown;
    use std::time::Duration;

    let (remote, events, mut daemon) = connected_pair_for_disconnect_test();
    let shutdown = CtrlEvent::Disconnected {
        reason: DisconnectReason::DaemonShutdown,
    };
    writeln!(daemon, "{}", serde_json::to_string(&shutdown).unwrap()).unwrap();
    daemon.shutdown(Shutdown::Write).unwrap();

    assert!(matches!(
        events.recv_timeout(Duration::from_secs(2)).unwrap(),
        PlayerEvent::DaemonShutdownAnnounced
    ));
    assert!(matches!(events.try_recv(), Err(mpsc::TryRecvError::Empty)));
    assert!(remote.is_shutdown_announced());
}

#[test]
fn daemon_endpoint_parses_local_and_unix_paths() {
    assert_eq!(
        DaemonEndpoint::parse("local").unwrap(),
        DaemonEndpoint::Local
    );
    assert_eq!(DaemonEndpoint::parse("").unwrap(), DaemonEndpoint::Local);
    assert_eq!(
        DaemonEndpoint::parse("unix:///tmp/mbv.sock").unwrap(),
        DaemonEndpoint::Unix(PathBuf::from("/tmp/mbv.sock"))
    );
    assert_eq!(
        DaemonEndpoint::parse("/tmp/mbv.sock").unwrap(),
        DaemonEndpoint::Unix(PathBuf::from("/tmp/mbv.sock"))
    );
    assert_eq!(
        DaemonEndpoint::parse("tcp://localhost:1234").unwrap(),
        DaemonEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], 1234)))
    );
    assert_eq!(
        DaemonEndpoint::parse("tcp://127.0.0.1:1234").unwrap(),
        DaemonEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], 1234)))
    );
    assert_eq!(
        DaemonEndpoint::parse("tcp://127.0.0.2:1234").unwrap(),
        DaemonEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 2], 1234)))
    );
}

#[test]
fn handshake_records_audio_only_capability_and_ignores_unknown_capability() {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;

    let (client, daemon) = UnixStream::pair().unwrap();
    let peer = std::thread::spawn(move || {
        let mut writer = daemon.try_clone().unwrap();
        let mut reader = BufReader::new(daemon);
        let mut hello = CtrlHello::current();
        hello.capabilities.push(crate::ctrl::CTRL_CAP_AUDIO_ONLY.to_string());
        hello.capabilities.push("future-capability".to_string());
        writeln!(writer, "{}", serde_json::to_string(&CtrlEvent::Hello(hello)).unwrap()).unwrap();
        let mut client_hello = String::new();
        reader.read_line(&mut client_hello).unwrap();
        let state = CtrlEvent::UnifiedQueueState(UnifiedQueueStateData {
            status: PlayerStatus::default(),
            slots: Vec::new(),
            active_slot: None,
            revision: 0,
            source: QueueSource::Unknown,
            in_flight_transition: None,
            queued_latest_transition: None,
        });
        writeln!(writer, "{}", serde_json::to_string(&state).unwrap()).unwrap();
    });

    let (_reader, _state, compatibility) = perform_handshake(
        SocketStream::Unix(client),
        || Ok("unused".to_string()),
    )
    .unwrap();
    assert!(compatibility.supports_audio_only);
    peer.join().unwrap();
}

#[test]
fn handshake_without_audio_only_capability_defaults_to_video_capable() {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;

    let (client, daemon) = UnixStream::pair().unwrap();
    let peer = std::thread::spawn(move || {
        let mut writer = daemon.try_clone().unwrap();
        let mut reader = BufReader::new(daemon);
        let hello = CtrlEvent::Hello(CtrlHello::current());
        writeln!(writer, "{}", serde_json::to_string(&hello).unwrap()).unwrap();
        let mut client_hello = String::new();
        reader.read_line(&mut client_hello).unwrap();
        let state = CtrlEvent::UnifiedQueueState(UnifiedQueueStateData {
            status: PlayerStatus::default(),
            slots: Vec::new(),
            active_slot: None,
            revision: 0,
            source: QueueSource::Unknown,
            in_flight_transition: None,
            queued_latest_transition: None,
        });
        writeln!(writer, "{}", serde_json::to_string(&state).unwrap()).unwrap();
    });

    let (_reader, _state, compatibility) = perform_handshake(
        SocketStream::Unix(client),
        || Ok("unused".to_string()),
    )
    .unwrap();
    assert!(!compatibility.supports_audio_only);
    peer.join().unwrap();
}

#[test]
fn daemon_endpoint_rejects_unsupported_schemes() {
    assert_eq!(
        DaemonEndpoint::parse("tcp://10.0.0.1:1234").unwrap(),
        DaemonEndpoint::Tcp(SocketAddr::from(([10, 0, 0, 1], 1234)))
    );
    assert!(DaemonEndpoint::parse("tcp://[::1]:4321").is_err());
    assert!(DaemonEndpoint::parse("unix://").is_err());
    assert!(DaemonEndpoint::parse("http://localhost:1234").is_err());
}

#[test]
fn status_only_preserves_event_confirmed_current_index() {
    let status = Arc::new(Mutex::new(status_with_idx(3)));
    let items = Arc::new(Mutex::new(Vec::new()));
    let unified_queue = Arc::new(Mutex::new(None));
    let queue_source = Arc::new(Mutex::new(QueueSource::Unknown));
    let (tx, _rx) = mpsc::channel();

    apply_ctrl_event(
        CtrlEvent::StatusOnly(status_with_idx(5)),
        &status,
        &items,
        &unified_queue,
        &queue_source,
        &tx,
        &Arc::new(Mutex::new(std::collections::HashMap::new())),
        true,
    );

    assert_eq!(status.lock().unwrap().current_idx, 3);
}

#[test]
fn status_only_preserves_current_idx_and_queue_len() {
    let status = Arc::new(Mutex::new(status_with_idx_and_len(3, 7)));
    let items = Arc::new(Mutex::new(Vec::new()));
    let unified_queue = Arc::new(Mutex::new(None));
    let queue_source = Arc::new(Mutex::new(QueueSource::Unknown));
    let (tx, _rx) = mpsc::channel();

    apply_ctrl_event(
        CtrlEvent::StatusOnly(status_with_idx_and_len(5, 2)),
        &status,
        &items,
        &unified_queue,
        &queue_source,
        &tx,
        &Arc::new(Mutex::new(std::collections::HashMap::new())),
        true,
    );

    let s = status.lock().unwrap();
    assert_eq!(s.current_idx, 3);
    assert_eq!(s.queue_len, 7);
}

#[test]
fn track_changed_leaves_status_mirror_for_app_to_rederive() {
    // `TrackChanged` now carries a `QueueSlotId`; the RemotePlayer read loop
    // has no queue to resolve it against and no longer mutates the status
    // mirror. `App::handle_player_event` re-derives `current_idx` from its
    // canonical queue instead (client-side mirror removal is Section 4).
    let status = Arc::new(Mutex::new(status_with_idx_and_len(0, 5)));
    let items = Arc::new(Mutex::new(Vec::new()));
    let unified_queue = Arc::new(Mutex::new(None));
    let queue_source = Arc::new(Mutex::new(QueueSource::Unknown));
    let (tx, _rx) = mpsc::channel();

    apply_ctrl_event(
        CtrlEvent::Player(PlayerEvent::TrackChanged {
            slot_id: crate::playback_queue::QueueSlotId::from_raw(2),
            transition: None,
        }),
        &status,
        &items,
        &unified_queue,
        &queue_source,
        &tx,
        &Arc::new(Mutex::new(std::collections::HashMap::new())),
        true,
    );

    let s = status.lock().unwrap();
    assert_eq!(s.current_idx, 0);
    assert_eq!(s.queue_len, 5);
}

#[test]
fn command_rejected_forwards_reason_as_player_event() {
    let status = Arc::new(Mutex::new(status_with_idx(0)));
    let items = Arc::new(Mutex::new(Vec::new()));
    let unified_queue = Arc::new(Mutex::new(None));
    let queue_source = Arc::new(Mutex::new(QueueSource::Unknown));
    let (tx, rx) = mpsc::channel();

    apply_ctrl_event(
        CtrlEvent::CommandRejected("daemon is audio-only".to_string()),
        &status,
        &items,
        &unified_queue,
        &queue_source,
        &tx,
        &Arc::new(Mutex::new(std::collections::HashMap::new())),
        true,
    );

    match rx.recv().unwrap() {
        PlayerEvent::CommandRejected(reason) => {
            assert_eq!(reason, "daemon is audio-only");
        }
        _ => panic!("expected CommandRejected"),
    }
}

#[test]
fn reconnect_replaces_queue_and_status_from_one_playback_snapshot() {
    use crate::ctrl::{UnifiedQueueSlot, UnifiedQueueStateData};

    let status = Arc::new(Mutex::new(status_with_idx_and_len(0, 0)));
    let items = Arc::new(Mutex::new(Vec::<EmbyItem>::new()));
    let unified_queue = Arc::new(Mutex::new(None));
    let queue_source = Arc::new(Mutex::new(QueueSource::Unknown));
    let (tx, rx) = mpsc::channel();
    let pending_playback = Arc::new(Mutex::new(std::collections::HashMap::new()));

    let reconnect_snapshot = UnifiedQueueStateData {
        status: status_with_idx_and_len(0, 2),
        slots: vec![
            UnifiedQueueSlot {
                slot_id: 11,
                item: QueueItem::Emby(Box::new(make_media_item("a"))),
            },
            UnifiedQueueSlot {
                slot_id: 22,
                item: QueueItem::Emby(Box::new(make_media_item("b"))),
            },
        ],
        active_slot: Some(22),
        revision: 9,
        source: QueueSource::Remote,
        in_flight_transition: None,
        queued_latest_transition: None,
    };

    apply_ctrl_event(
        CtrlEvent::UnifiedQueueState(reconnect_snapshot),
        &status,
        &items,
        &unified_queue,
        &queue_source,
        &tx,
        &pending_playback,
        true,
    );

    let stored = unified_queue.lock().unwrap().clone().unwrap();
    let status = status.lock().unwrap().clone();
    let event = rx.recv().unwrap();
    let PlayerEvent::UnifiedQueueUpdated(event_snapshot) = event else {
        panic!("expected unified queue snapshot");
    };

    assert_eq!(stored.revision, 9);
    assert_eq!(stored.slots.iter().map(|slot| slot.slot_id).collect::<Vec<_>>(), vec![11, 22]);
    assert_eq!(stored.active_slot, Some(22));
    assert_eq!(stored.status.current_idx, 1);
    assert_eq!(stored.status.queue_len, 2);
    assert_eq!(status.current_idx, stored.status.current_idx);
    assert_eq!(status.queue_len, stored.status.queue_len);
    assert_eq!(status.active, stored.status.active);
    assert_eq!(event_snapshot.revision, stored.revision);
    assert_eq!(event_snapshot.slots.iter().map(|slot| slot.slot_id).collect::<Vec<_>>(), vec![11, 22]);
    assert_eq!(event_snapshot.active_slot, stored.active_slot);
    assert_eq!(event_snapshot.status.current_idx, status.current_idx);
}

#[test]
fn unified_queue_state_preserves_canonical_coordinates_and_source() {
    use crate::ctrl::{UnifiedQueueSlot, UnifiedQueueStateData};

    let status = Arc::new(Mutex::new(status_with_idx(0)));
    let items = Arc::new(Mutex::new(Vec::<EmbyItem>::new()));
    let unified_queue = Arc::new(Mutex::new(None));
    let queue_source = Arc::new(Mutex::new(QueueSource::Unknown));
    let (tx, rx) = mpsc::channel();

    // Mixed queue: [Emby(e0), Feed(f1), Emby(e2), Feed(f3)]. The active
    // Feed slot must retain its canonical index; it cannot be represented by
    // an index into the legacy Emby-only projection.
    let e0 = make_media_item("e0");
    let e2 = make_media_item("e2");
    let f1 = crate::playback_queue::FeedEntry {
        guid: "f1".into(),
        title: "f1".into(),
        enclosure_url: None,
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: None,
        feed_id: None,
        position_ticks: 0,
        played: false,
    };
    let f3 = crate::playback_queue::FeedEntry {
        guid: "f3".into(),
        title: "f3".into(),
        enclosure_url: None,
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: None,
        feed_id: None,
        position_ticks: 0,
        played: false,
    };

    let unified = UnifiedQueueStateData {
        status: status_with_idx_and_len(1, 4),
        slots: vec![
            UnifiedQueueSlot {
                slot_id: 10,
                item: QueueItem::Emby(Box::new(e0)),
            },
            UnifiedQueueSlot {
                slot_id: 20,
                item: QueueItem::Feed(f1),
            },
            UnifiedQueueSlot {
                slot_id: 30,
                item: QueueItem::Emby(Box::new(e2)),
            },
            UnifiedQueueSlot {
                slot_id: 40,
                item: QueueItem::Feed(f3),
            },
        ],
        active_slot: Some(20), // canonical slot_id for f1
        revision: 1,
        source: QueueSource::Playlist {
            id: Some("pl-1".into()),
            name: "My Playlist".into(),
        },
        in_flight_transition: None,
        queued_latest_transition: None,
    };

    apply_ctrl_event(
        CtrlEvent::UnifiedQueueState(unified),
        &status,
        &items,
        &unified_queue,
        &queue_source,
        &tx,
        &Arc::new(Mutex::new(std::collections::HashMap::new())),
        true,
    );

    // queue_source carried from unified state
    assert!(
        matches!(
            *queue_source.lock().unwrap(),
            QueueSource::Playlist {
                id: Some(ref id),
                ..
            } if id == "pl-1"
        ),
        "queue_source should be carried from UnifiedQueueStateData"
    );

    // queue_len from canonical slots
    assert_eq!(status.lock().unwrap().queue_len, 4);

    assert_eq!(status.lock().unwrap().current_idx, 1);

    // Emby-only items: Feed entries stripped
    let emby = items.lock().unwrap();
    assert_eq!(emby.len(), 2, "only Emby items in legacy projection");
    assert_eq!(emby[0].id, "e0");
    assert_eq!(emby[1].id, "e2");

    let canonical = unified_queue.lock().unwrap().clone().unwrap();
    assert_eq!(canonical.active_slot, Some(20));
    assert_eq!(canonical.slots[1].slot_id, 20);

    // UnifiedQueueUpdated event emitted with full canonical data
    match rx.recv().unwrap() {
        PlayerEvent::UnifiedQueueUpdated(state) => {
            assert_eq!(state.slots.len(), 4);
            assert_eq!(state.active_slot, Some(20));
            assert_eq!(
                state.source,
                QueueSource::Playlist {
                    id: Some("pl-1".into()),
                    name: "My Playlist".into(),
                }
            );
            // Slot IDs preserved
            assert_eq!(state.slots[0].slot_id, 10);
            assert_eq!(state.slots[1].slot_id, 20);
            assert_eq!(state.slots[2].slot_id, 30);
            assert_eq!(state.slots[3].slot_id, 40);
        }
        _ => panic!("expected UnifiedQueueUpdated"),
    }
}
