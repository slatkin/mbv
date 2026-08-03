use super::*;
use crate::config::QueueSource;
use crate::ctrl::CtrlState;
use crate::ctrl::WireCommand;
use crate::player::PlayerCommand;

fn make_media_item(id: &str) -> MediaItem {
    MediaItem {
        id: id.into(),
        name: "Test Item".into(),
        item_type: "Episode".into(),
        is_folder: false,
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

fn status_with_idx(current_idx: usize) -> PlayerStatus {
    status_with_idx_and_len(current_idx, 0)
}

fn status_with_idx_and_len(current_idx: usize, queue_len: usize) -> PlayerStatus {
    RemotePlayer::stub_status(current_idx, queue_len)
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
    let queue_source = Arc::new(Mutex::new(QueueSource::Unknown));
    let (tx, _rx) = mpsc::channel();

    apply_ctrl_event(
        CtrlEvent::StatusOnly(status_with_idx(5)),
        &status,
        &items,
        &queue_source,
        &tx,
        &Arc::new(Mutex::new(std::collections::HashMap::new())),
        true,
    );

    assert_eq!(status.lock().unwrap().current_idx, 3);
}

#[test]
fn state_uses_cursor_as_current_index() {
    let status = Arc::new(Mutex::new(status_with_idx(0)));
    let items = Arc::new(Mutex::new(Vec::new()));
    let queue_source = Arc::new(Mutex::new(QueueSource::Unknown));
    let (tx, rx) = mpsc::channel();

    apply_ctrl_event(
        CtrlEvent::State(CtrlState {
            status: status_with_idx(5),
            items: Vec::new(),
            cursor: 3,
            source: QueueSource::Unknown,
        }),
        &status,
        &items,
        &queue_source,
        &tx,
        &Arc::new(Mutex::new(std::collections::HashMap::new())),
        true,
    );

    assert_eq!(status.lock().unwrap().current_idx, 3);
    assert!(matches!(
        rx.recv().unwrap(),
        PlayerEvent::QueueUpdated { cursor: 3, .. }
    ));
}

#[test]
fn status_only_preserves_current_idx_and_queue_len() {
    let status = Arc::new(Mutex::new(status_with_idx_and_len(3, 7)));
    let items = Arc::new(Mutex::new(Vec::new()));
    let queue_source = Arc::new(Mutex::new(QueueSource::Unknown));
    let (tx, _rx) = mpsc::channel();

    apply_ctrl_event(
        CtrlEvent::StatusOnly(status_with_idx_and_len(5, 2)),
        &status,
        &items,
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
fn state_derives_queue_len_from_items_not_status() {
    let status = Arc::new(Mutex::new(status_with_idx_and_len(0, 0)));
    let items = Arc::new(Mutex::new(Vec::new()));
    let queue_source = Arc::new(Mutex::new(QueueSource::Unknown));
    let (tx, _rx) = mpsc::channel();

    // s.status.queue_len (99) is stale relative to s.items.len() (2) — the
    // daemon broadcasts CtrlState before calling play_queue(...), so
    // items/cursor are authoritative over status at broadcast time.
    apply_ctrl_event(
        CtrlEvent::State(CtrlState {
            status: status_with_idx_and_len(5, 99),
            items: vec![make_media_item("a"), make_media_item("b")],
            cursor: 1,
            source: QueueSource::Unknown,
        }),
        &status,
        &items,
        &queue_source,
        &tx,
        &Arc::new(Mutex::new(std::collections::HashMap::new())),
        true,
    );

    assert_eq!(status.lock().unwrap().queue_len, 2);
}

#[test]
fn track_changed_updates_current_idx_but_not_queue_len() {
    let status = Arc::new(Mutex::new(status_with_idx_and_len(0, 5)));
    let items = Arc::new(Mutex::new(Vec::new()));
    let queue_source = Arc::new(Mutex::new(QueueSource::Unknown));
    let (tx, _rx) = mpsc::channel();

    apply_ctrl_event(
        CtrlEvent::Player(PlayerEvent::TrackChanged(2)),
        &status,
        &items,
        &queue_source,
        &tx,
        &Arc::new(Mutex::new(std::collections::HashMap::new())),
        true,
    );

    let s = status.lock().unwrap();
    assert_eq!(s.current_idx, 2);
    assert_eq!(s.queue_len, 5);
}

#[test]
fn command_rejected_forwards_reason_as_player_event() {
    let status = Arc::new(Mutex::new(status_with_idx(0)));
    let items = Arc::new(Mutex::new(Vec::new()));
    let queue_source = Arc::new(Mutex::new(QueueSource::Unknown));
    let (tx, rx) = mpsc::channel();

    apply_ctrl_event(
        CtrlEvent::CommandRejected("daemon is audio-only".to_string()),
        &status,
        &items,
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
fn adopt_queue_returns_false_when_ctrl_socket_is_dead() {
    // #119 task 5: `adopt_queue`'s return value is the only signal that
    // the daemon never actually received the adoption — the call site
    // must not discard it and silently carry on with optimistic state.
    let (remote, _event_rx, cmd_rx) = RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    drop(cmd_rx);

    let adopted = remote.adopt_queue(vec![make_media_item("1")], 0, QueueSource::Unknown);

    assert!(!adopted);
}

#[test]
fn control_stream_shutdown_unblocks_a_concurrent_blocking_read() {
    // #233: shutdown() must affect the *shared underlying socket*, not
    // just the fd this particular ControlStream clone holds -- that's
    // the whole point of using shutdown() instead of Drop. Prove it by
    // shutting down one clone and confirming a DIFFERENT clone's
    // blocking read unblocks (returns Ok(0), i.e. EOF) as a result.
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let accept_thread = std::thread::spawn(move || listener.accept().unwrap().0);

    let client_stream = ControlStream::Tcp(TcpStream::connect(addr).unwrap());
    let _server_stream = accept_thread.join().unwrap();

    let reader_clone = client_stream.try_clone().unwrap();
    let read_thread = std::thread::spawn(move || {
        let mut reader_clone = reader_clone;
        let mut buf = [0u8; 8];
        reader_clone.read(&mut buf)
    });

    // Give the read thread a moment to actually block in read() before
    // we shut down the OTHER clone.
    std::thread::sleep(Duration::from_millis(50));
    client_stream.shutdown().unwrap();

    let result = read_thread
        .join()
        .expect("read thread must exit, not hang, once the socket is shut down");
    assert_eq!(
        result.unwrap(),
        0,
        "a shut-down socket must unblock a concurrent read with EOF (Ok(0))"
    );
}

#[test]
fn disconnect_causes_the_reader_thread_to_observe_the_shutdown_and_exit() {
    // #233: the only pre-existing teardown was an implicit Drop of the
    // writer thread's fd duplicate, which never affected the reader
    // thread's *separate* duplicate of the same socket -- so the
    // reader thread's blocking `read()` inside `reader.lines()` never
    // unblocked, leaking the thread forever. `disconnect()` must fix
    // this: after calling it, the reader thread must observe EOF/an
    // error on its own read and exit, which is exactly what flips
    // `is_disconnected()` to true (see the reader thread's loop-exit
    // code in `connect_endpoint`).
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let daemon = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut writer = stream.try_clone().unwrap();
        let mut reader = BufReader::new(stream);

        let hello = serde_json::to_string(&CtrlEvent::Hello(CtrlHello::current())).unwrap();
        writeln!(writer, "{hello}").unwrap();
        let mut client_hello = String::new();
        reader.read_line(&mut client_hello).unwrap();

        let initial_state = serde_json::to_string(&CtrlEvent::State(CtrlState {
            status: PlayerStatus::default(),
            items: Vec::new(),
            cursor: 0,
            source: crate::config::QueueSource::Unknown,
        }))
        .unwrap();
        writeln!(writer, "{initial_state}").unwrap();

        // Keep the daemon-side handle open well past the point the
        // client calls disconnect(), so the test can distinguish "the
        // client's shutdown() itself caused the reader to exit" from
        // "the daemon happened to hang up around the same time."
        std::thread::sleep(Duration::from_secs(2));
    });

    let (remote, _event_rx) =
        RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr), "token").unwrap();
    assert!(!remote.is_disconnected());

    remote.disconnect();

    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while !remote.is_disconnected() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        remote.is_disconnected(),
        "reader thread must observe the shutdown and exit, flipping is_disconnected()"
    );

    drop(daemon); // let the daemon thread's sleep finish in the background
}

#[test]
fn disconnect_is_idempotent() {
    // A second call must not panic (Task 2's Option::take() makes the
    // stored stream handle single-use).
    let (remote, _event_rx) = RemotePlayer::stub(Vec::new(), 0);
    remote.disconnect();
    remote.disconnect();
}

fn spawn_test_daemon_up_to_state(
    listener: std::net::TcpListener,
    after_state: impl FnOnce(&mut TcpStream) + Send + 'static,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut writer = stream.try_clone().unwrap();
        let mut reader = BufReader::new(stream);

        let hello = serde_json::to_string(&CtrlEvent::Hello(CtrlHello::current())).unwrap();
        writeln!(writer, "{hello}").unwrap();
        let mut client_hello = String::new();
        reader.read_line(&mut client_hello).unwrap();

        let initial_state = serde_json::to_string(&CtrlEvent::State(CtrlState {
            status: PlayerStatus::default(),
            items: Vec::new(),
            cursor: 0,
            source: crate::config::QueueSource::Unknown,
        }))
        .unwrap();
        writeln!(writer, "{initial_state}").unwrap();

        after_state(&mut writer);
    })
}

#[test]
fn announced_daemon_shutdown_sets_is_shutdown_announced_and_emits_no_stopped_event() {
    // Task 7.4: the reader thread's `is_structured_disconnect` branch (task
    // 1.5) must route an announced `DaemonShutdown` to `is_shutdown_announced()`
    // and `PlayerEvent::DaemonShutdownAnnounced`, never a synthetic `Stopped`
    // -- getting this backwards means a spurious crash modal on every clean
    // `mbv -q` shutdown (see `player_event.rs`'s `DaemonShutdownAnnounced` arm).
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let daemon = spawn_test_daemon_up_to_state(listener, |writer| {
        let disconnected = serde_json::to_string(&CtrlEvent::Disconnected {
            reason: DisconnectReason::DaemonShutdown,
        })
        .unwrap();
        writeln!(writer, "{disconnected}").unwrap();
        // Stream closes as `writer` (and the accepted `stream`) drop here.
    });

    let (remote, event_rx) =
        RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr), "token").unwrap();

    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while !remote.is_disconnected() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(remote.is_disconnected());
    assert!(
        remote.is_shutdown_announced(),
        "an announced DaemonShutdown must mark is_shutdown_announced()"
    );

    let events: Vec<_> = event_rx.try_iter().collect();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, PlayerEvent::DaemonShutdownAnnounced)),
        "expected a DaemonShutdownAnnounced event, got {} events",
        events.len()
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, PlayerEvent::Stopped { .. })),
        "an announced shutdown must never emit a synthetic Stopped event, got {} events",
        events.len()
    );

    daemon.join().unwrap();
}

#[test]
fn unannounced_disconnect_leaves_is_shutdown_announced_false_and_emits_stopped() {
    // The other half of the boundary: a daemon that vanishes with no
    // `Disconnected` event (a crash) must not be mistaken for a clean
    // shutdown -- getting this backwards means a silent exit on a real
    // crash instead of the unannounced-loss modal (`player_event.rs`'s
    // `Stopped` arm).
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let daemon = spawn_test_daemon_up_to_state(listener, |_writer| {
        // Drop the connection with no `Disconnected` event -- a crash, not
        // a deliberate shutdown.
    });

    let (remote, event_rx) =
        RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr), "token").unwrap();

    // Wait for the synthetic Stopped event with a hard deadline, instead
    // of racing a sleep against the reader thread's
    // `disconnected.store(true)` -> `event_tx.send(Stopped)` sequence.
    // Receiving Stopped proves both have happened, so the post-conditions
    // are stable to read.
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    let mut saw_stopped = false;
    while std::time::Instant::now() < deadline {
        match event_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(PlayerEvent::Stopped { .. }) => {
                saw_stopped = true;
                break;
            }
            Ok(_) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    assert!(
        saw_stopped,
        "expected a synthetic Stopped event before the 2s deadline"
    );

    assert!(remote.is_disconnected());
    assert!(
        !remote.is_shutdown_announced(),
        "a bare, unannounced disconnect must not be mistaken for an announced shutdown"
    );

    // Drain anything that arrived after Stopped and assert the negative:
    // an unannounced disconnect never emits DaemonShutdownAnnounced.
    let tail: Vec<_> = event_rx.try_iter().collect();
    assert!(
        !tail
            .iter()
            .any(|e| matches!(e, PlayerEvent::DaemonShutdownAnnounced)),
        "an unannounced disconnect must never emit DaemonShutdownAnnounced, got {} tail events",
        tail.len()
    );

    daemon.join().unwrap();
}

#[test]
fn connect_endpoint_propagates_active_remote_playback_status() {
    // #175: a local `mbv` connected as the ctrl client of a remote
    // `mbvd` must mirror the daemon's active playback into
    // `RemotePlayer.status` -- that's the shared `Arc<Mutex<PlayerStatus>>`
    // MPRIS polls directly (see `src/mpris.rs::start`). This drives the
    // *real* TCP protocol path (hello exchange, initial `State`, then a
    // `StatusOnly` push) end-to-end, rather than calling
    // `apply_ctrl_event` directly, so it catches propagation bugs in the
    // reader thread / connect handshake that a unit-level test of
    // `apply_ctrl_event` alone would miss.
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let daemon = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut writer = stream.try_clone().unwrap();
        let mut reader = BufReader::new(stream);

        // Protocol hello.
        let hello = serde_json::to_string(&CtrlEvent::Hello(CtrlHello::current())).unwrap();
        writeln!(writer, "{hello}").unwrap();

        // Read the client's hello back (unused beyond draining the line).
        let mut client_hello = String::new();
        reader.read_line(&mut client_hello).unwrap();

        // Initial baseline state: idle, nothing playing yet.
        let initial_state = serde_json::to_string(&CtrlEvent::State(CtrlState {
            status: PlayerStatus::default(),
            items: Vec::new(),
            cursor: 0,
            source: crate::config::QueueSource::Unknown,
        }))
        .unwrap();
        writeln!(writer, "{initial_state}").unwrap();

        // Now the daemon reports active playback, exactly like the #175
        // repro: an active `StatusOnly` push after the initial handshake.
        let active_status = serde_json::to_string(&CtrlEvent::StatusOnly(PlayerStatus {
            active: true,
            paused: false,
            title: "Song".to_string(),
            position_ticks: 5_000_000,
            runtime_ticks: 100_000_000,
            ..PlayerStatus::default()
        }))
        .unwrap();
        writeln!(writer, "{active_status}").unwrap();

        // Keep the connection open until the test thread is done with it.
        std::thread::sleep(Duration::from_millis(500));
    });

    let (remote, _event_rx) =
        RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr), "token").unwrap();

    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        if remote.status.lock().unwrap().active {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "remote status never reflected the daemon's active playback"
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    let status = remote.status.lock().unwrap().clone();
    assert!(status.active);
    assert!(!status.paused);
    assert_eq!(status.title, "Song");

    daemon.join().unwrap();
}

#[test]
fn perform_handshake_times_out_when_daemon_never_sends_hello() {
    // #191 fix 5: a daemon that accepts a TCP connection but never
    // speaks (never sends the protocol hello) must not hang the caller
    // forever -- the hard bound wrapping `perform_handshake` inside
    // `connect_endpoint` must kick in. This drives the real bounded
    // handshake path (real socket, real thread) rather than asserting
    // on config values, since ureq-style timeout knobs don't have
    // getters and a real stalled-listener test is the only way to prove
    // the join-timeout logic actually fires.
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let daemon = std::thread::spawn(move || {
        // Accept the connection and then say nothing -- long enough to
        // outlive the test's much shorter hard bound below.
        let (_stream, _) = listener.accept().unwrap();
        std::thread::sleep(Duration::from_secs(2));
    });

    let stream = ControlStream::Tcp(TcpStream::connect(addr).unwrap());
    let result = crate::bounded::run_with_hard_bound(
        move || perform_handshake(stream, "token").map_err(|e| e.to_string()),
        Duration::from_millis(50),
    );

    match result {
        Err(e) => assert_eq!(e, "timed out after 0s"),
        Ok(_) => panic!("expected perform_handshake to time out, got Ok"),
    }

    daemon.join().unwrap();
}

#[test]
fn perform_handshake_succeeds_promptly_when_daemon_responds() {
    // Companion to the timeout test above: the fast/success path must
    // still work end-to-end through the same bounded wrapper used in
    // `connect_endpoint`, not just in isolation.
    use std::io::BufRead;
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let daemon = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut writer = stream.try_clone().unwrap();
        let mut reader = BufReader::new(stream);

        let hello = serde_json::to_string(&CtrlEvent::Hello(CtrlHello::current())).unwrap();
        writeln!(writer, "{hello}").unwrap();

        let mut client_hello = String::new();
        reader.read_line(&mut client_hello).unwrap();

        let initial_state = serde_json::to_string(&CtrlEvent::State(CtrlState {
            status: PlayerStatus::default(),
            items: Vec::new(),
            cursor: 0,
            source: crate::config::QueueSource::Unknown,
        }))
        .unwrap();
        writeln!(writer, "{initial_state}").unwrap();
    });

    let stream = ControlStream::Tcp(TcpStream::connect(addr).unwrap());
    let result = crate::bounded::run_with_hard_bound(
        move || perform_handshake(stream, "token").map_err(|e| e.to_string()),
        Duration::from_secs(5),
    );

    let (_reader, state_event, compatibility) = match result {
        Ok(v) => v,
        Err(e) => panic!("expected Ok, got Err({e})"),
    };
    assert!(matches!(state_event, CtrlEvent::State(_)));
    assert_eq!(
        compatibility.peer_protocol_version,
        crate::ctrl::CTRL_PROTOCOL_VERSION
    );

    daemon.join().unwrap();
}

#[test]
fn request_shutdown_returns_accepted_once_the_completer_and_reader_thread_share_the_same_arc() {
    // Before the fix, the RemotePlayer returned by connect_endpoint held a
    // different Arc<Mutex<Option<Sender<..>>>> than the reader thread wrote
    // into, so the reader thread's take() could never find the caller's
    // completer and request_shutdown always returned TimedOut even when the
    // daemon replied immediately. Drive the real handshake and a real
    // ShutdownAccepted reply through a real socket to prove they now share
    // the same Arc.
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let daemon = spawn_test_daemon_up_to_state(listener, |writer| {
        let mut reader = BufReader::new(writer.try_clone().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert!(
            line.contains("RequestShutdown"),
            "expected RequestShutdown, got {line:?}"
        );

        let accepted = serde_json::to_string(&CtrlEvent::ShutdownAccepted).unwrap();
        writeln!(writer, "{accepted}").unwrap();
    });

    let (remote, _event_rx) =
        RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr), "token").unwrap();

    let response = remote.request_shutdown(Duration::from_secs(2));
    assert_eq!(response, crate::remote_player::ShutdownResponse::Accepted);

    daemon.join().unwrap();
}

#[test]
fn request_shutdown_is_unsupported_and_sends_nothing_when_daemon_lacks_capability() {
    // The other half of the gate: a daemon that never advertises
    // lifecycle-shutdown must get Unsupported without ever seeing a
    // RequestShutdown command on the wire -- that's the whole point of
    // negotiating the capability instead of sending a command into a
    // timeout against an old daemon.
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (received_tx, received_rx) = mpsc::channel::<String>();
    let daemon = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut writer = stream.try_clone().unwrap();
        let mut reader = BufReader::new(stream);

        let mut hello_info = CtrlHello::current();
        hello_info
            .capabilities
            .retain(|c| c != crate::ctrl::CTRL_CAP_LIFECYCLE_SHUTDOWN);
        let hello = serde_json::to_string(&CtrlEvent::Hello(hello_info)).unwrap();
        writeln!(writer, "{hello}").unwrap();

        let mut client_hello = String::new();
        reader.read_line(&mut client_hello).unwrap();

        let initial_state = serde_json::to_string(&CtrlEvent::State(CtrlState {
            status: PlayerStatus::default(),
            items: Vec::new(),
            cursor: 0,
            source: crate::config::QueueSource::Unknown,
        }))
        .unwrap();
        writeln!(writer, "{initial_state}").unwrap();

        // Whatever the client sends next (there should be nothing) is
        // reported back to the test thread. A closed connection (no
        // RequestShutdown ever sent) unblocks this read_line with EOF,
        // i.e. an empty string.
        let mut next_line = String::new();
        let _ = reader.read_line(&mut next_line);
        let _ = received_tx.send(next_line);
    });

    let (remote, _event_rx) =
        RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr), "token").unwrap();

    let response = remote.request_shutdown(Duration::from_secs(2));
    assert_eq!(
        response,
        crate::remote_player::ShutdownResponse::Unsupported
    );

    // Close the connection so the daemon's blocking read_line unblocks with
    // EOF instead of hanging forever waiting on a line that was never sent.
    remote.disconnect();

    let line = received_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("daemon thread should observe EOF and report back");
    assert!(
        line.is_empty(),
        "expected no RequestShutdown line to be sent, got {line:?}"
    );

    daemon.join().unwrap();
}

#[test]
fn v3_peer_sends_queue_append_wire_command() {
    let existing = vec![make_media_item("1")];
    let (remote, _event_rx, cmd_rx) = RemotePlayer::stub_with_command_rx(existing, 0);

    assert!(remote.send_command(PlayerCommand::QueueAppend {
        items: vec![make_media_item("2")]
    }));

    match cmd_rx.recv().unwrap() {
        CtrlCmd::PlayerCmd(WireCommand::QueueAppend { items }) => {
            assert_eq!(
                items
                    .iter()
                    .map(|item| item.id.as_str())
                    .collect::<Vec<_>>(),
                ["2"]
            );
        }
        _ => panic!("expected QueueAppend"),
    }
}
