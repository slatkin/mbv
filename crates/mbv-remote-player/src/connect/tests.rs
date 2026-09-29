use super::*;
use crate::QueueOp;
use mbv_emby_model::EmbyImageTags;
use mbv_emby_model::EmbyItem;
use mbv_queue::QueueSource;
use mbv_queue::{FeedEntry, QueueItem};
use std::net::SocketAddr;

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
        image_tags: EmbyImageTags::default(),
    }
}

fn status_with_idx(current_idx: usize) -> PlayerStatus {
    status_with_idx_and_len(current_idx, 0)
}

fn status_with_idx_and_len(current_idx: usize, queue_len: usize) -> PlayerStatus {
    RemotePlayer::stub_status(current_idx, queue_len)
}

fn make_feed_entry(guid: &str) -> FeedEntry {
    FeedEntry {
        guid: guid.into(),
        title: guid.into(),
        enclosure_url: None,
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: None,
        feed_id: None,
        position_ticks: 0,
        played: false,
    }
}

fn connected_pair_for_disconnect_test() -> (RemotePlayer, mpsc::Receiver<PlayerEvent>, UnixStream) {
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
            lineage: mbv_queue::QueueLineage::default(),
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

fn admission_error(reason: DisconnectReason) -> crate::RemotePlayerError {
    let (client, daemon) = UnixStream::pair().unwrap();
    let peer = std::thread::spawn(move || {
        let mut writer = daemon.try_clone().unwrap();
        let mut reader = BufReader::new(daemon);
        writeln!(
            writer,
            "{}",
            serde_json::to_string(&CtrlEvent::Hello(CtrlHello::current())).unwrap()
        )
        .unwrap();
        let mut client_hello = String::new();
        reader.read_line(&mut client_hello).unwrap();
        writeln!(
            writer,
            "{}",
            serde_json::to_string(&CtrlEvent::Disconnected { reason }).unwrap()
        )
        .unwrap();
    });
    let result = perform_handshake(SocketStream::Unix(client), || {
        Ok("unused-control-token".to_string())
    });
    peer.join().unwrap();
    result.unwrap_err()
}

#[test]
fn service_setup_admin_handshake_advertises_an_admin_only_connection() {
    let (client, daemon) = UnixStream::pair().unwrap();
    let (hello_tx, hello_rx) = mpsc::channel();
    let peer = std::thread::spawn(move || {
        let mut writer = daemon.try_clone().unwrap();
        let mut reader = BufReader::new(daemon);
        writeln!(
            writer,
            "{}",
            serde_json::to_string(&CtrlEvent::Hello(CtrlHello::current())).unwrap()
        )
        .unwrap();
        let mut client_hello = String::new();
        reader.read_line(&mut client_hello).unwrap();
        hello_tx
            .send(serde_json::from_str::<CtrlCmd>(&client_hello).unwrap())
            .unwrap();
        writeln!(
            writer,
            "{}",
            serde_json::to_string(&CtrlEvent::UnifiedQueueState(UnifiedQueueStateData {
                status: PlayerStatus::default(),
                slots: Vec::new(),
                active_slot: None,
                revision: 0,
                source: QueueSource::Unknown,
                lineage: mbv_queue::QueueLineage::default(),
                in_flight_transition: None,
                queued_latest_transition: None,
            }))
            .unwrap()
        )
        .unwrap();
    });
    let (_, state, _) = perform_service_setup_admin_handshake(SocketStream::Unix(client), || {
        Ok("admin-control-token".to_string())
    })
    .unwrap();
    peer.join().unwrap();
    let Ok(CtrlCmd::Hello(hello)) = hello_rx.recv() else {
        panic!("expected client hello");
    };
    assert!(hello.supports_service_setup_admin());
    assert_eq!(hello.control_token.as_deref(), Some("admin-control-token"));
    assert!(matches!(state, CtrlEvent::UnifiedQueueState(_)));
}

#[test]
fn connect_endpoint_maps_owner_admission_refusals() {
    let exclusive = admission_error(DisconnectReason::ExclusiveOwner { pid: 1234 });
    assert_eq!(exclusive.kind_name(), "remote-player.exclusive_owner");
    assert_eq!(
        exclusive.to_string(),
        "local owner process 1234 already has a client"
    );

    let shutting_down = admission_error(DisconnectReason::OwnerShuttingDown);
    assert_eq!(
        shutting_down.kind_name(),
        "remote-player.owner_shutting_down"
    );
    assert_eq!(shutting_down.to_string(), "the owner is shutting down");
}

#[test]
fn unsupported_idle_queue_load_is_rejected_without_staging_local_queue() {
    let (remote, _events, commands) =
        RemotePlayer::stub_with_command_rx(vec![make_media_item("confirmed")], 0);
    let result = remote.load_queue_idle(9, vec![], 0, QueueSource::Album);
    assert!(result.is_err());
    assert!(matches!(
        commands.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    assert_eq!(remote.status.lock().unwrap().queue_len, 1);
    assert!(remote.unified_queue_state().is_none());
}

#[test]
fn supported_idle_queue_load_sends_correlated_request_without_staging_queue() {
    let (mut remote, _events, commands) =
        RemotePlayer::stub_with_command_rx(vec![make_media_item("confirmed")], 0);
    remote.ctrl_compatibility.supports_owner_queue_load = true;
    remote
        .load_queue_idle(31, vec![], 0, QueueSource::Album)
        .unwrap();
    assert!(matches!(
        commands.try_recv(),
        Ok(CtrlCmd::UnifiedQueueLoadIdle { request_id: 31, .. })
    ));
    assert_eq!(remote.status.lock().unwrap().queue_len, 1);
    assert!(remote.unified_queue_state().is_none());
}

#[test]
fn answered_queue_op_is_correlated_and_sent_on_the_wire() {
    let (mut remote, _events, commands) = RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    remote.ctrl_compatibility.supports_answered_queue_ops = true;
    let op = remote
        .send_queue_op(QueueOp::RemoveSlot { slot_id: 17 })
        .unwrap();
    let Some(expected) = op else {
        panic!("current peer advertises answered queue operations");
    };
    assert_eq!(expected.0, 1);
    assert!(matches!(
        commands.try_recv(),
        Ok(CtrlCmd::UnifiedQueueRemoveSlot {
            slot_id: 17,
            op: Some(id)
        }) if id == expected
    ));
}

#[test]
fn legacy_queue_op_sends_without_an_operation_id() {
    let (mut remote, _events, commands) = RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    remote.ctrl_compatibility.supports_answered_queue_ops = false;
    assert_eq!(
        remote
            .send_queue_op(QueueOp::RemoveSlot { slot_id: 17 })
            .unwrap(),
        None
    );
    assert!(matches!(
        commands.try_recv(),
        Ok(CtrlCmd::UnifiedQueueRemoveSlot {
            slot_id: 17,
            op: None
        })
    ));
}

// Regression for 2b22f7f42: Applied is the client's only post-op queue snapshot.
#[test]
fn inbound_queue_op_result_follows_prior_state_event() {
    use mbv_ctrl::UnifiedQueueSlot;

    let (remote, events, mut daemon) = connected_pair_for_disconnect_test();
    let slots = vec![
        UnifiedQueueSlot {
            slot_id: 10,
            item: QueueItem::Emby(Box::new(make_media_item("e0"))),
        },
        UnifiedQueueSlot {
            slot_id: 20,
            item: QueueItem::Emby(Box::new(make_media_item("e1"))),
        },
        UnifiedQueueSlot {
            slot_id: 30,
            item: QueueItem::Emby(Box::new(make_media_item("e2"))),
        },
    ];
    let mut state = UnifiedQueueStateData {
        status: status_with_idx_and_len(2, 3),
        slots,
        active_slot: Some(30),
        revision: 3,
        source: QueueSource::Unknown,
        lineage: mbv_queue::QueueLineage::default(),
        in_flight_transition: None,
        queued_latest_transition: None,
    };
    let mut earlier_state = state.clone();
    earlier_state.revision = 2;
    writeln!(
        daemon,
        "{}",
        serde_json::to_string(&CtrlEvent::UnifiedQueueState(earlier_state)).unwrap()
    )
    .unwrap();
    state.slots.remove(0);
    let mut stale_status = state.status.clone();
    stale_status.current_idx = 2;
    stale_status.queue_len = 3;
    state.status = stale_status;
    writeln!(
        daemon,
        "{}",
        serde_json::to_string(&CtrlEvent::QueueOpResult {
            op: mbv_ctrl::QueueOpId(7),
            outcome: mbv_ctrl::QueueOpOutcome::Applied(Box::new(state)),
        })
        .unwrap()
    )
    .unwrap();

    assert!(matches!(
        events.recv().unwrap(),
        PlayerEvent::UnifiedQueueUpdated(_)
    ));
    assert!(matches!(
        events.recv().unwrap(),
        PlayerEvent::UnifiedQueueUpdated(_)
    ));
    assert!(matches!(
        events.recv().unwrap(),
        PlayerEvent::QueueOpResult {
            op: mbv_ctrl::QueueOpId(7),
            ..
        }
    ));
    let status = remote.status.lock().unwrap();
    assert_eq!(status.queue_len, 2);
    assert_eq!(status.current_idx, 1);
    let queue = remote.unified_queue_state().unwrap();
    assert_eq!(queue.revision, 3);
    assert_eq!(queue.slots.len(), 2);
    assert_eq!(queue.active_slot, Some(30));
}

#[test]
fn adopt_queue_does_not_write_projection_before_owner_snapshot() {
    let (remote, _events, commands) =
        RemotePlayer::stub_with_command_rx(vec![make_media_item("existing")], 0);
    assert!(remote.adopt_queue(
        vec![QueueItem::Emby(Box::new(make_media_item("replacement")))],
        0,
        QueueSource::Album,
    ));
    let status = remote.status.lock().unwrap();
    assert_eq!(status.current_idx, 0);
    assert_eq!(status.queue_len, 1);
    assert!(status.active);
    drop(status);
    assert!(remote.unified_queue_state().is_none());
    assert!(matches!(
        commands.try_recv(),
        Ok(CtrlCmd::UnifiedAdoptQueue { .. })
    ));
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
            if message == mbv_ctrl::player::CONNECTION_LOST_MESSAGE
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
            if message == mbv_ctrl::player::CONNECTION_LOST_MESSAGE
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
            if message == mbv_ctrl::player::CONNECTION_LOST_MESSAGE
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
            if message == mbv_ctrl::player::CONNECTION_LOST_MESSAGE
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
fn resolve_library_route_has_no_wildcard_fallback() {
    let mut routes = std::collections::BTreeMap::new();
    routes.insert("music".to_string(), "tcp://192.168.0.104:47788".to_string());
    assert_eq!(
        resolve_library_route(&routes, "Music"),
        Some(DaemonEndpoint::Tcp("192.168.0.104:47788".parse().unwrap()))
    );
    assert_eq!(resolve_library_route(&routes, "movies"), None);
}

#[test]
fn resolve_library_route_rejects_a_bare_device_name_as_malformed() {
    // A stale pre-#256 config entry (device name, no scheme) must
    // NOT silently resolve -- DaemonEndpoint::parse would otherwise
    // accept it as a bogus Unix(PathBuf) socket path. Library routing
    // is tcp://-only (#239 addendum), so anything that doesn't parse
    // to Tcp(_) is treated as malformed: logged and skipped.
    let mut routes = std::collections::BTreeMap::new();
    routes.insert("music".to_string(), "living-room-pc".to_string());
    assert_eq!(resolve_library_route(&routes, "music"), None);
}

#[test]
fn resolve_library_route_rejects_unix_and_local_endpoints() {
    // Library routing is remote-only -- a unix:// or bare "local"
    // value is well-formed as a DaemonEndpoint but not a valid
    // library route, so it must still resolve to None.
    let mut routes = std::collections::BTreeMap::new();
    routes.insert("music".to_string(), "unix:///run/mbvd.sock".to_string());
    routes.insert("movies".to_string(), "local".to_string());
    assert_eq!(resolve_library_route(&routes, "music"), None);
    assert_eq!(resolve_library_route(&routes, "movies"), None);
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
        hello
            .capabilities
            .push(mbv_ctrl::CTRL_CAP_AUDIO_ONLY.to_string());
        hello.capabilities.push("future-capability".to_string());
        writeln!(
            writer,
            "{}",
            serde_json::to_string(&CtrlEvent::Hello(hello)).unwrap()
        )
        .unwrap();
        let mut client_hello = String::new();
        reader.read_line(&mut client_hello).unwrap();
        let state = CtrlEvent::UnifiedQueueState(UnifiedQueueStateData {
            status: PlayerStatus::default(),
            slots: Vec::new(),
            active_slot: None,
            revision: 0,
            source: QueueSource::Unknown,
            lineage: mbv_queue::QueueLineage::default(),
            in_flight_transition: None,
            queued_latest_transition: None,
        });
        writeln!(writer, "{}", serde_json::to_string(&state).unwrap()).unwrap();
    });

    let (_reader, _state, compatibility) =
        perform_handshake(SocketStream::Unix(client), || Ok("unused".to_string())).unwrap();
    assert!(compatibility.supports_audio_only);
    assert!(compatibility.supports_answered_queue_ops);
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
        let mut hello = CtrlHello::current();
        hello
            .capabilities
            .retain(|cap| cap != mbv_ctrl::CTRL_CAP_ANSWERED_QUEUE_OPS);
        writeln!(
            writer,
            "{}",
            serde_json::to_string(&CtrlEvent::Hello(hello)).unwrap()
        )
        .unwrap();
        let mut client_hello = String::new();
        reader.read_line(&mut client_hello).unwrap();
        let state = CtrlEvent::UnifiedQueueState(UnifiedQueueStateData {
            status: PlayerStatus::default(),
            slots: Vec::new(),
            active_slot: None,
            revision: 0,
            source: QueueSource::Unknown,
            lineage: mbv_queue::QueueLineage::default(),
            in_flight_transition: None,
            queued_latest_transition: None,
        });
        writeln!(writer, "{}", serde_json::to_string(&state).unwrap()).unwrap();
    });

    let (_reader, _state, compatibility) =
        perform_handshake(SocketStream::Unix(client), || Ok("unused".to_string())).unwrap();
    assert!(!compatibility.supports_audio_only);
    assert!(!compatibility.supports_answered_queue_ops);
    peer.join().unwrap();
}

#[test]
fn daemon_endpoint_rejects_unsupported_schemes() {
    assert_eq!(
        DaemonEndpoint::parse("tcp://10.0.0.1:1234").unwrap(),
        DaemonEndpoint::Tcp(SocketAddr::from(([10, 0, 0, 1], 1234)))
    );
    DaemonEndpoint::parse("tcp://[::1]:4321").unwrap_err();
    DaemonEndpoint::parse("unix://").unwrap_err();
    DaemonEndpoint::parse("http://localhost:1234").unwrap_err();
}

#[test]
fn status_only_preserves_event_confirmed_current_index() {
    let status = Arc::new(Mutex::new(status_with_idx(3)));
    let unified_queue = Arc::new(Mutex::new(None));
    let (tx, _rx) = mpsc::channel();

    apply_ctrl_event(
        CtrlEvent::StatusOnly(status_with_idx(5)),
        &status,
        &unified_queue,
        &tx,
        &Arc::new(Mutex::new(std::collections::HashMap::new())),
        true,
    );

    assert_eq!(status.lock().unwrap().current_idx, 3);
}

#[test]
fn status_only_preserves_current_idx_and_queue_len() {
    let status = Arc::new(Mutex::new(status_with_idx_and_len(3, 7)));
    let unified_queue = Arc::new(Mutex::new(None));
    let (tx, _rx) = mpsc::channel();

    apply_ctrl_event(
        CtrlEvent::StatusOnly(status_with_idx_and_len(5, 2)),
        &status,
        &unified_queue,
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
    let unified_queue = Arc::new(Mutex::new(None));
    let (tx, _rx) = mpsc::channel();

    apply_ctrl_event(
        CtrlEvent::Player(PlayerEvent::TrackChanged {
            slot_id: mbv_queue::QueueSlotId::from_raw(2),
            transition: None,
        }),
        &status,
        &unified_queue,
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
    let unified_queue = Arc::new(Mutex::new(None));
    let (tx, rx) = mpsc::channel();

    apply_ctrl_event(
        CtrlEvent::CommandRejected("daemon is audio-only".to_string()),
        &status,
        &unified_queue,
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
    use mbv_ctrl::{UnifiedQueueSlot, UnifiedQueueStateData};

    let status = Arc::new(Mutex::new(status_with_idx_and_len(0, 0)));
    let unified_queue = Arc::new(Mutex::new(None));
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
        lineage: mbv_queue::QueueLineage::default(),
        in_flight_transition: None,
        queued_latest_transition: None,
    };

    apply_ctrl_event(
        CtrlEvent::UnifiedQueueState(reconnect_snapshot),
        &status,
        &unified_queue,
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
    assert_eq!(
        stored
            .slots
            .iter()
            .map(|slot| slot.slot_id)
            .collect::<Vec<_>>(),
        vec![11, 22]
    );
    assert_eq!(stored.active_slot, Some(22));
    assert_eq!(stored.status.current_idx, 1);
    assert_eq!(stored.status.queue_len, 2);
    assert_eq!(status.current_idx, stored.status.current_idx);
    assert_eq!(status.queue_len, stored.status.queue_len);
    assert_eq!(status.active, stored.status.active);
    assert_eq!(event_snapshot.revision, stored.revision);
    assert_eq!(
        event_snapshot
            .slots
            .iter()
            .map(|slot| slot.slot_id)
            .collect::<Vec<_>>(),
        vec![11, 22]
    );
    assert_eq!(event_snapshot.active_slot, stored.active_slot);
    assert_eq!(event_snapshot.status.current_idx, status.current_idx);
}

#[test]
fn unified_queue_state_preserves_canonical_coordinates_and_source() {
    use mbv_ctrl::{UnifiedQueueSlot, UnifiedQueueStateData};

    let status = Arc::new(Mutex::new(status_with_idx(0)));
    let unified_queue = Arc::new(Mutex::new(None));
    let (tx, rx) = mpsc::channel();

    // Mixed queue: [Emby(e0), Feed(f1), Emby(e2), Feed(f3)]. The active
    // Feed slot retains its canonical index in the unified snapshot.
    let e0 = make_media_item("e0");
    let e2 = make_media_item("e2");
    let f1 = make_feed_entry("f1");
    let f3 = make_feed_entry("f3");

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
        lineage: mbv_queue::QueueLineage::default(),
        in_flight_transition: None,
        queued_latest_transition: None,
    };

    apply_ctrl_event(
        CtrlEvent::UnifiedQueueState(unified),
        &status,
        &unified_queue,
        &tx,
        &Arc::new(Mutex::new(std::collections::HashMap::new())),
        true,
    );

    // queue_len from canonical slots
    assert_eq!(status.lock().unwrap().queue_len, 4);

    assert_eq!(status.lock().unwrap().current_idx, 1);

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
