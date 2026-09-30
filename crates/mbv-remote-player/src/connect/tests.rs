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

mod queue;
