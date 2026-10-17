use super::handshake::{perform_handshake, perform_service_setup_admin_handshake};
use super::owner_actions::{run_owner_action_handshake, send_owner_action};
use super::*;
use crate::QueueOp;
use mbv_ctrl::OwnerAction;
use mbv_emby_model::EmbyImageTags;
use mbv_emby_model::EmbyItem;
use mbv_queue::QueueSource;
use mbv_queue::{FeedEntry, QueueItem};
use std::net::SocketAddr;
use std::path::PathBuf;

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
        track_artist: String::new(),
        artist_items: Vec::new(),
        sort_name: String::new(),
        production_year: 0,
        end_year: 0,
        studios: Vec::new(),
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
    let (remote, events) =
        connect_stream(SocketStream::Unix(client), PeerBuild::Any, false, None).unwrap();
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

/// Spec `owner-actions` "Owner refuses": the CLI shows the Owner's reason on
/// stderr and exits 1, so the error text must carry that reason through
/// (tray-pin-swap task 4.5).
#[test]
fn owner_action_refused_reply_surfaces_the_owner_reason() {
    use mbv_ctrl::CTRL_CAP_OWNER_ACTION;

    let (client, daemon) = UnixStream::pair().unwrap();
    let (request_tx, request_rx) = mpsc::channel();
    let peer = std::thread::spawn(move || {
        let mut writer = daemon.try_clone().unwrap();
        let mut reader = BufReader::new(daemon);
        let mut hello = CtrlHello::current();
        hello.capabilities.push(CTRL_CAP_OWNER_ACTION.to_string());
        writeln!(
            writer,
            "{}",
            serde_json::to_string(&CtrlEvent::Hello(hello)).unwrap()
        )
        .unwrap();
        let mut client_hello = String::new();
        reader.read_line(&mut client_hello).unwrap();
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
        let mut request = String::new();
        reader.read_line(&mut request).unwrap();
        request_tx
            .send(serde_json::from_str::<CtrlCmd>(&request).unwrap())
            .unwrap();
        writeln!(
            writer,
            "{}",
            serde_json::to_string(&CtrlEvent::OwnerActionRefused {
                reason: "a panel swap is already running".to_string()
            })
            .unwrap()
        )
        .unwrap();
    });
    let mut reader = BufReader::new(SocketStream::Unix(client));
    run_owner_action_handshake(&mut reader, OwnerAction::SwapPanel, || {
        Ok("owner-action-token".to_string())
    })
    .unwrap();
    let error = send_owner_action(&mut reader, OwnerAction::SwapPanel).unwrap_err();
    peer.join().unwrap();
    assert!(matches!(
        request_rx.recv().unwrap(),
        CtrlCmd::RunOwnerAction(OwnerAction::SwapPanel)
    ));
    assert!(
        error
            .to_string()
            .contains("a panel swap is already running")
    );
}

/// Contract: spec owner-actions "Owner does not support this action" — an
/// Owner that advertises `owner-action` but predates `OpenTerminal` replies
/// `CommandRejected`; the CLI reports a restart requirement without waiting
/// for the 6 s read timeout.
#[test]
fn owner_action_command_rejected_reply_reports_a_restart_requirement() {
    use mbv_ctrl::CTRL_CAP_OWNER_ACTION;

    let (client, daemon) = UnixStream::pair().unwrap();
    let peer = std::thread::spawn(move || {
        let mut writer = daemon.try_clone().unwrap();
        let mut reader = BufReader::new(daemon);
        let mut hello = CtrlHello::current();
        hello.capabilities.push(CTRL_CAP_OWNER_ACTION.to_string());
        writeln!(
            writer,
            "{}",
            serde_json::to_string(&CtrlEvent::Hello(hello)).unwrap()
        )
        .unwrap();
        let mut client_hello = String::new();
        reader.read_line(&mut client_hello).unwrap();
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
        let mut request = String::new();
        reader.read_line(&mut request).unwrap();
        writeln!(
            writer,
            "{}",
            serde_json::to_string(&CtrlEvent::CommandRejected(
                "unknown command open-terminal".to_string()
            ))
            .unwrap()
        )
        .unwrap();
    });
    let mut reader = BufReader::new(SocketStream::Unix(client));
    run_owner_action_handshake(&mut reader, OwnerAction::OpenTerminal, || {
        Ok("owner-action-token".to_string())
    })
    .unwrap();
    let error = send_owner_action(&mut reader, OwnerAction::OpenTerminal).unwrap_err();
    peer.join().unwrap();
    assert_eq!(
        error.to_string(),
        "the running mbv is too old for --open; restart it"
    );
}

#[test]
fn connect_endpoint_maps_owner_admission_refusals() {
    let exclusive = admission_error(DisconnectReason::ExclusiveOwner { pid: 1234 });
    assert!(exclusive.is_exclusive_owner());
    assert_eq!(
        exclusive.to_string(),
        "local owner process 1234 already has a client"
    );

    let shutting_down = admission_error(DisconnectReason::OwnerShuttingDown);
    assert!(shutting_down.is_owner_shutting_down());
    assert_eq!(shutting_down.to_string(), "the owner is shutting down");
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
// Issue #559: the build gate fires on the Owner's hello, before the Client
// sends its own hello/control credential (design D1).
fn local_handshake_rejects_different_owner_build_before_client_hello() {
    use std::io::{BufRead, Write};

    let (client, daemon) = UnixStream::pair().unwrap();
    let (received_tx, received_rx) = mpsc::channel();
    let owner_version = "different-owner-build";
    let peer = std::thread::spawn(move || {
        let mut writer = daemon.try_clone().unwrap();
        let mut reader = BufReader::new(daemon);
        let mut hello = CtrlHello::current();
        hello.app_version = owner_version.to_string();
        writeln!(
            writer,
            "{}",
            serde_json::to_string(&CtrlEvent::Hello(hello)).unwrap()
        )
        .unwrap();
        let mut client_hello = String::new();
        reader.read_line(&mut client_hello).unwrap();
        received_tx.send(client_hello).unwrap();
    });

    let error = perform_handshake_with_role(
        SocketStream::Unix(client),
        || Ok("unused".to_string()),
        HandshakeRole::Client,
        PeerBuild::MustMatch,
        false,
        None,
    )
    .unwrap_err();

    assert_eq!(error.mismatched_owner_version(), Some(owner_version));
    peer.join().unwrap();
    assert_eq!(received_rx.recv().unwrap(), "");
}

#[test]
// Issue #559: a matching Owner build attaches normally under the build gate.
fn local_handshake_accepts_identical_owner_build() {
    use std::io::{BufRead, BufReader, Write};

    let (client, daemon) = UnixStream::pair().unwrap();
    let peer = std::thread::spawn(move || {
        let mut writer = daemon.try_clone().unwrap();
        let mut reader = BufReader::new(daemon);
        let mut hello = CtrlHello::current();
        hello.app_version = env!("CARGO_PKG_VERSION").to_string();
        writeln!(
            writer,
            "{}",
            serde_json::to_string(&CtrlEvent::Hello(hello)).unwrap()
        )
        .unwrap();
        let mut client_hello = String::new();
        reader.read_line(&mut client_hello).unwrap();
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

    perform_handshake_with_role(
        SocketStream::Unix(client),
        || Ok("unused".to_string()),
        HandshakeRole::Client,
        PeerBuild::MustMatch,
        false,
        None,
    )
    .unwrap();
    peer.join().unwrap();
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

/// A fake daemon whose Hello advertises `playback-output`, whose Client
/// hello is captured for assertion, and which — after the initial state —
/// optionally cold-seeds a Cast output, then holds the socket open.
fn connect_with_advertised_hello(
    seed: Option<mbv_ctrl::OutputState>,
) -> (
    crate::RemotePlayer,
    mpsc::Receiver<PlayerEvent>,
    std::thread::JoinHandle<()>,
) {
    let (client, daemon) = UnixStream::pair().unwrap();
    let peer = std::thread::spawn(move || {
        let mut writer = daemon.try_clone().unwrap();
        let mut reader = BufReader::new(daemon);
        let mut hello = CtrlHello::current();
        hello
            .capabilities
            .push(mbv_ctrl::CTRL_CAP_PLAYBACK_OUTPUT.to_string());
        writeln!(
            writer,
            "{}",
            serde_json::to_string(&CtrlEvent::Hello(hello)).unwrap()
        )
        .unwrap();
        let mut client_hello = String::new();
        reader.read_line(&mut client_hello).unwrap();
        assert!(
            client_hello.contains(mbv_ctrl::CTRL_CAP_PLAYBACK_OUTPUT),
            "a Client-role hello must claim playback-output — the claim is what tells the Owner to cold-seed and broadcast to this connection"
        );
        assert!(
            client_hello.contains(mbv_ctrl::CTRL_CAP_EMBY_SESSION_OUTPUT),
            "live-test defect: a Client-role hello must claim emby-session-output — without it the Owner masks the selected Emby Session output to Local"
        );
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
        if let Some(output) = seed {
            let seed_event = CtrlEvent::PlaybackOutput(output);
            writeln!(writer, "{}", serde_json::to_string(&seed_event).unwrap()).unwrap();
        }
        let mut buf = [0u8; 256];
        while let Ok(n) = reader.get_mut().read(&mut buf) {
            if n == 0 {
                break;
            }
        }
    });
    let (player, event_rx) =
        connect_stream(SocketStream::Unix(client), PeerBuild::Any, false, None).unwrap();
    (player, event_rx, peer)
}

#[test]
fn playback_output_seed_is_retained_from_the_handshake() {
    // cast-owner-output 8.2 fix: a Client that claims `playback-output` is
    // cold-seeded right after the initial state (core_ctrl_spawn order:
    // state first, then the seed). The handshake consumes that line
    // synchronously, so the connection retains the state — readable before
    // any event drain, which is what lets launch auto-reconnect's handback
    // decision see Cast — and the event still forwards to the channel so
    // the App mirror receives it on the first drain.
    let (player, event_rx, peer) =
        connect_with_advertised_hello(Some(mbv_ctrl::OutputState::Cast {
            receiver_id: "r-1".to_string(),
            name: "Living Room".to_string(),
            connection: mbv_ctrl::OutputConnection::Connected,
        }));

    let expected = mbv_ctrl::OutputState::Cast {
        receiver_id: "r-1".to_string(),
        name: "Living Room".to_string(),
        connection: mbv_ctrl::OutputConnection::Connected,
    };
    assert_eq!(
        player.playback_output_state(),
        Some(expected.clone()),
        "the handshake must retain the cold seed on the connection, readable before any drain"
    );
    match event_rx.recv().unwrap() {
        PlayerEvent::PlaybackOutput(state) => assert_eq!(state, expected),
        other => panic!("expected PlaybackOutput, got {other:?}"),
    }
    // No Drop impl: disconnect() is what shuts the client end down, so
    // the fake daemon's read loop observes EOF and the join completes.
    player.disconnect();
    peer.join().unwrap();
}

/// Live-test defect: Enter on a Brave web-client Emby Session did nothing.
/// The production hello must claim `emby-session-output` (asserted by the harness).
#[test]
fn production_client_hello_advertises_emby_session_output() {
    let (player, _event_rx, peer) = connect_with_advertised_hello(None);

    player.disconnect();
    peer.join().unwrap();
}

#[test]
fn connect_survives_a_missing_playback_output_seed() {
    // cast-owner-output 8.2 fix, tolerance half: a peer that advertises
    // `playback-output` but never sends the cold seed (a stale or
    // misbehaving daemon) must not fail or hang the connect. The tolerant
    // handshake read gives up after its short timeout and leaves the
    // silence to the reader thread, so the connect succeeds with no
    // retained output.
    let (player, _event_rx, peer) = connect_with_advertised_hello(None);

    assert_eq!(
        player.playback_output_state(),
        None,
        "no seed means no retained output, and the connect must still succeed"
    );
    player.disconnect();
    peer.join().unwrap();
}

#[test]
fn a_late_partial_seed_line_is_never_consumed() {
    // 8.2 fix 2 (review P2): a `read_line` under a read timeout consumes
    // the prefix of a line split across two arrivals and loses it when the
    // second half times out — the reader thread would then see only the
    // remainder and the seed would be lost for the whole session. The seed
    // wait must never consume a partial line: it gives up at its deadline
    // without touching the socket, and the reader thread then reads the
    // whole line as one event.
    use std::time::Duration;

    let cast_output = mbv_ctrl::OutputState::Cast {
        receiver_id: "r-1".to_string(),
        name: "Living Room".to_string(),
        connection: mbv_ctrl::OutputConnection::Connected,
    };
    // The line as the daemon sends it: JSON plus its terminating newline,
    // so both halves still end the line correctly when reassembled.
    let seed_line = format!(
        "{}\n",
        serde_json::to_string(&CtrlEvent::PlaybackOutput(cast_output.clone())).unwrap()
    );
    // The payload is ASCII, so a plain split never cuts a code point.
    // Owned halves: the daemon thread takes them by value.
    let split = seed_line.len() / 2;
    let (seed_prefix, seed_suffix) = (seed_line[..split].to_owned(), seed_line[split..].to_owned());

    let (client, daemon) = UnixStream::pair().unwrap();
    let peer = std::thread::spawn(move || {
        let mut writer = daemon.try_clone().unwrap();
        let mut reader = BufReader::new(daemon);
        let mut hello = CtrlHello::current();
        hello
            .capabilities
            .push(mbv_ctrl::CTRL_CAP_PLAYBACK_OUTPUT.to_string());
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
        // Half the seed now, the other half after a pause twice the
        // handshake's seed deadline: the wait must give up on the partial
        // line without consuming it, and the reader thread must later see
        // the two halves as one line.
        writer.write_all(seed_prefix.as_bytes()).unwrap();
        writer.flush().unwrap();
        std::thread::sleep(Duration::from_secs(1));
        writer.write_all(seed_suffix.as_bytes()).unwrap();
        writer.flush().unwrap();
        // Hold until the client end closes: nothing is sent, so `lines`
        // just waits for EOF (no `Read` import needed).
        for held in reader.lines() {
            let _ = held;
        }
    });

    let (player, event_rx) =
        connect_stream(SocketStream::Unix(client), PeerBuild::Any, false, None).unwrap();

    let event = event_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the whole seed line must reach the reader thread as one event");
    match event {
        PlayerEvent::PlaybackOutput(state) => assert_eq!(state, cast_output),
        other => panic!("expected PlaybackOutput, got {other:?}"),
    }
    assert_eq!(
        player.playback_output_state(),
        Some(mbv_ctrl::OutputState::Cast {
            receiver_id: "r-1".to_string(),
            name: "Living Room".to_string(),
            connection: mbv_ctrl::OutputConnection::Connected,
        }),
        "the reader thread retains the late seed too"
    );
    // No Drop impl: disconnect() is what shuts the client end down, so
    // the fake daemon's read loop observes EOF and the join completes.
    player.disconnect();
    peer.join().unwrap();
}

mod queue;
