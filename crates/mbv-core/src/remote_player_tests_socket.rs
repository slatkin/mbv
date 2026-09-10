
#[test]
fn adopt_queue_returns_false_when_ctrl_socket_is_dead() {
    // #119 task 5: `adopt_queue`'s return value is the only signal that
    // the daemon never actually received the adoption — the call site
    // must not discard it and silently carry on with optimistic state.
    let (remote, _event_rx, cmd_rx) = RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    drop(cmd_rx);

    let adopted = remote.adopt_queue(
        vec![QueueItem::Emby(Box::new(make_media_item("1")))],
        0,
        QueueSource::Unknown,
    );

    assert!(!adopted);
}

#[test]
fn control_stream_shutdown_unblocks_a_concurrent_blocking_read() {
    // #233: shutdown() must affect the *shared underlying socket*, not
    // just the fd this particular SocketStream clone holds -- that's
    // the whole point of using shutdown() instead of Drop. Prove it by
    // shutting down one clone and confirming a DIFFERENT clone's
    // blocking read unblocks (returns Ok(0), i.e. EOF) as a result.
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let accept_thread = std::thread::spawn(move || listener.accept().unwrap().0);

    let client_stream = SocketStream::Tcp(TcpStream::connect(addr).unwrap());
    let _server_stream = accept_thread.join().unwrap();

    let reader_clone = client_stream.try_clone().unwrap();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let read_thread = std::thread::spawn(move || {
        let mut reader_clone = reader_clone;
        let mut buf = [0u8; 8];
        ready_tx.send(()).unwrap();
        reader_clone.read(&mut buf)
    });

    // Synchronize with the reader immediately before its blocking read
    // instead of racing an arbitrary sleep against thread scheduling.
    ready_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("read thread must start before the socket is shut down");
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
    // See `announced_daemon_shutdown_...`: the handshake credential lookup
    // runs on the connect worker thread.
    let _scratch = crate::config::TestTempDir::new().as_xdg_home();
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
    let (release_tx, release_rx) = std::sync::mpsc::channel();

    let daemon = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut writer = stream.try_clone().unwrap();
        let mut reader = BufReader::new(stream);

        let hello = serde_json::to_string(&CtrlEvent::Hello(CtrlHello::current())).unwrap();
        writeln!(writer, "{hello}").unwrap();
        let mut client_hello = String::new();
        reader.read_line(&mut client_hello).unwrap();

        let initial_state = serde_json::to_string(&CtrlEvent::UnifiedQueueState(
            crate::ctrl::UnifiedQueueStateData {
                status: PlayerStatus::default(),
                slots: Vec::new(),
                active_slot: None,
                revision: 0,
                source: crate::config::QueueSource::Unknown,
                in_flight_transition: None,
                queued_latest_transition: None,
            },
        ))
        .unwrap();
        writeln!(writer, "{initial_state}").unwrap();

        // Keep the daemon-side handle open until the client has observed
        // the shutdown. This is deterministic and does not leak a sleeping
        // test thread past the assertion.
        release_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("test must release the daemon after observing shutdown");
    });

    let (remote, _event_rx) = RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr)).unwrap();
    assert!(!remote.is_disconnected());

    remote.disconnect();

    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    while !remote.is_disconnected() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        remote.is_disconnected(),
        "reader thread must observe the shutdown and exit, flipping is_disconnected()"
    );

    release_tx.send(()).unwrap();
    daemon.join().unwrap();
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

        let initial_state = serde_json::to_string(&CtrlEvent::UnifiedQueueState(
            crate::ctrl::UnifiedQueueStateData {
                status: PlayerStatus::default(),
                slots: Vec::new(),
                active_slot: None,
                revision: 0,
                source: crate::config::QueueSource::Unknown,
                in_flight_transition: None,
                queued_latest_transition: None,
            },
        ))
        .unwrap();
        writeln!(writer, "{initial_state}").unwrap();

        after_state(&mut writer);
    })
}

#[test]
fn announced_daemon_shutdown_sets_is_shutdown_announced_and_emits_no_stopped_event() {
    // The handshake's `load_or_create_control_credential()` resolves
    // `state_dir()` on the connect worker thread, so it needs the
    // process-wide scratch dir, not the thread-local guard.
    let _scratch = crate::config::TestTempDir::new().as_xdg_home();
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

    let (remote, event_rx) = RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr)).unwrap();

    // Wait for the DaemonShutdownAnnounced event with a hard deadline,
    // instead of racing a sleep against the reader thread's
    // `disconnected.store(true)` -> `shutdown_announced.store(true)` ->
    // `event_tx.send(DaemonShutdownAnnounced)` sequence. Receiving the
    // event proves the shutdown was announced, so the post-conditions
    // are stable to read.
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    let mut saw_shutdown = false;
    while std::time::Instant::now() < deadline {
        match event_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(PlayerEvent::DaemonShutdownAnnounced) => {
                saw_shutdown = true;
                break;
            }
            Ok(_) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    assert!(
        saw_shutdown,
        "expected a DaemonShutdownAnnounced event before the 2s deadline"
    );
    assert!(remote.is_disconnected());
    assert!(
        remote.is_shutdown_announced(),
        "an announced DaemonShutdown must mark is_shutdown_announced()"
    );

    // Drain anything that arrived after DaemonShutdownAnnounced and
    // assert the negative: an announced shutdown never emits a
    // synthetic Stopped event.
    let events: Vec<_> = event_rx.try_iter().collect();
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
    // See `announced_daemon_shutdown_...`: the handshake credential lookup
    // runs on the connect worker thread.
    let _scratch = crate::config::TestTempDir::new().as_xdg_home();
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

    let (remote, event_rx) = RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr)).unwrap();

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
    // See `announced_daemon_shutdown_...`: the handshake credential lookup
    // runs on the connect worker thread.
    let _scratch = crate::config::TestTempDir::new().as_xdg_home();
    // #175: a local `mbv` connected as the ctrl client of a remote
    // `mbvd` must mirror the daemon's active playback into
    // `RemotePlayer.status` -- that's the shared `Arc<Mutex<PlayerStatus>>`
    // MPRIS polls directly (see `src/mpris.rs::start`). This drives the
    // *real* TCP protocol path (hello exchange, initial `State`, then a
    // unified snapshot push) end-to-end, rather than calling
    // `apply_ctrl_event` directly, so it catches propagation bugs in the
    // reader thread / connect handshake that a unit-level test of
    // `apply_ctrl_event` alone would miss.
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (release_tx, release_rx) = std::sync::mpsc::channel();

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
        let initial_state = serde_json::to_string(&CtrlEvent::UnifiedQueueState(
            crate::ctrl::UnifiedQueueStateData {
                status: PlayerStatus::default(),
                slots: Vec::new(),
                active_slot: None,
                revision: 0,
                source: crate::config::QueueSource::Unknown,
                in_flight_transition: None,
                queued_latest_transition: None,
            },
        ))
        .unwrap();
        writeln!(writer, "{initial_state}").unwrap();

        // Now the daemon reports active playback in the same unified
        // snapshot used for reconnect and normal updates.
        let active_status = serde_json::to_string(&CtrlEvent::UnifiedQueueState(
            crate::ctrl::UnifiedQueueStateData {
                status: PlayerStatus {
                    active: true,
                    paused: false,
                    title: "Song".to_string(),
                    position_ticks: 5_000_000,
                    runtime_ticks: 100_000_000,
                    ..PlayerStatus::default()
                },
                slots: Vec::new(),
                active_slot: None,
                revision: 1,
                source: crate::config::QueueSource::Unknown,
                in_flight_transition: None,
                queued_latest_transition: None,
            },
        ))
        .unwrap();
        writeln!(writer, "{active_status}").unwrap();

        // Keep the connection open until the test has finished checking the
        // status, without relying on a timing sleep.
        release_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("test must release the daemon after checking status");
    });

    let (remote, _event_rx) = RemotePlayer::connect_endpoint(&DaemonEndpoint::Tcp(addr)).unwrap();

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

    release_tx.send(()).unwrap();
    daemon.join().unwrap();
}
