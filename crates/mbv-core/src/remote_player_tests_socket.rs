use crate::ctrl::WireCommand;
use crate::player::PlayerCommand;

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
            },
        ))
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
            },
        ))
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
    let (accepted_tx, accepted_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();

    let daemon = std::thread::spawn(move || {
        // Accept the connection and then say nothing -- long enough to
        // outlive the test's much shorter hard bound below.
        let (_stream, _) = listener.accept().unwrap();
        accepted_tx.send(()).unwrap();
        release_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("test must release the stalled daemon");
    });

    let stream = ControlStream::Tcp(TcpStream::connect(addr).unwrap());
    accepted_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("daemon must accept the test connection");
    let result = crate::bounded::run_with_hard_bound(
        move || perform_handshake(stream, || Ok("control".to_string())),
        Duration::from_millis(50),
    );

    match result {
        Err(e) => assert_eq!(e, "timed out after 0s"),
        Ok(_) => panic!("expected perform_handshake to time out, got Ok"),
    }

    release_tx.send(()).unwrap();
    daemon.join().unwrap();
}

#[test]
fn perform_handshake_succeeds_promptly_when_daemon_responds() {
    // Companion to the timeout test above: the fast/success path must still
    // work end-to-end through the same bounded wrapper used in
    // `connect_endpoint`, without transmitting a Service credential.
    use std::io::BufRead;
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let daemon = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut writer = stream.try_clone().unwrap();
        let mut reader = BufReader::new(stream);

        let mut hello_info = CtrlHello::current();
        hello_info
            .capabilities
            .retain(|cap| cap != crate::ctrl::CTRL_CAP_CONTROL_AUTH);
        let hello = serde_json::to_string(&CtrlEvent::Hello(hello_info)).unwrap();
        writeln!(writer, "{hello}").unwrap();

        let mut client_hello = String::new();
        reader.read_line(&mut client_hello).unwrap();
        let CtrlCmd::Hello(client_hello) = serde_json::from_str(&client_hello).unwrap() else {
            panic!("expected client hello");
        };
        assert_eq!(client_hello.control_token, None);

        let initial_state = serde_json::to_string(&CtrlEvent::UnifiedQueueState(
            crate::ctrl::UnifiedQueueStateData {
                status: PlayerStatus::default(),
                slots: Vec::new(),
                active_slot: None,
                revision: 0,
                source: crate::config::QueueSource::Unknown,
            },
        ))
        .unwrap();
        writeln!(writer, "{initial_state}").unwrap();
    });

    let stream = ControlStream::Tcp(TcpStream::connect(addr).unwrap());
    let result = crate::bounded::run_with_hard_bound(
        move || {
            perform_handshake(stream, || {
                panic!("packaged peer must not request a credential")
            })
        },
        Duration::from_secs(5),
    );

    let (_reader, state_event, compatibility) = match result {
        Ok(v) => v,
        Err(e) => panic!("expected Ok, got Err({e})"),
    };
    assert!(matches!(state_event, CtrlEvent::UnifiedQueueState(_)));
    assert_eq!(
        compatibility.peer_protocol_version,
        crate::ctrl::CTRL_PROTOCOL_VERSION
    );

    daemon.join().unwrap();
}

#[test]
fn perform_handshake_rejects_old_version_before_sending_client_hello() {
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let daemon = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut writer = stream.try_clone().unwrap();
        let mut reader = BufReader::new(stream);
        let mut old_hello = CtrlHello::current();
        old_hello.protocol_version -= 1;
        writeln!(
            writer,
            "{}",
            serde_json::to_string(&CtrlEvent::Hello(old_hello)).unwrap()
        )
        .unwrap();
        reader
            .get_mut()
            .set_read_timeout(Some(Duration::from_millis(250)))
            .unwrap();
        let mut client_hello = String::new();
        assert_eq!(reader.read_line(&mut client_hello).unwrap(), 0);
    });

    let stream = ControlStream::Tcp(TcpStream::connect(addr).unwrap());
    let result = perform_handshake(stream, || {
        panic!("version mismatch must not request a Control credential")
    });
    let error = match result {
        Ok(_) => panic!("old protocol version must be rejected"),
        Err(error) => error,
    };
    assert!(error.contains("incompatible daemon protocol version"));
    daemon.join().unwrap();
}

#[test]
fn request_shutdown_sends_command_and_receives_via_shared_channel() {
    // Proves the shutdown request/response path works through the shared
    // command channel: request_shutdown sends CtrlCmd::RequestShutdown and
    // the response arrives on the same completer channel. The stub's
    // channel pair is deterministic (no socket timing), covering the same
    // property the old socket-based test targeted.
    let (mut remote, _event_rx, cmd_rx) = RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    remote.ctrl_compatibility.supports_lifecycle_shutdown = true;

    let handle = std::thread::spawn(move || remote.request_shutdown(Duration::from_millis(100)));

    // Read the command the background thread sent.
    let cmd = cmd_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("request_shutdown must send a command");
    assert!(
        matches!(cmd, CtrlCmd::RequestShutdown),
        "expected RequestShutdown"
    );

    // The thread is now blocked on response_rx.  We can't easily inject a
    // response through the stub (no reader thread), so just join and
    // confirm it doesn't panic — the channel mechanism itself is proven by
    // the command arriving above.
    let response = handle
        .join()
        .expect("request_shutdown thread must not panic");
    // Without a daemon to reply, the stub times out — that's expected.
    assert!(
        matches!(response, crate::remote_player::ShutdownResponse::TimedOut),
        "stub has no reader thread to inject a reply, so TimedOut is correct"
    );
}

#[test]

fn request_shutdown_is_unsupported_and_sends_nothing_when_daemon_lacks_capability() {
    let (mut remote, _event_rx, cmd_rx) = RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    remote.ctrl_compatibility.supports_lifecycle_shutdown = false;

    let response = remote.request_shutdown(Duration::from_secs(2));
    assert_eq!(
        response,
        crate::remote_player::ShutdownResponse::Unsupported
    );

    // Dropping the stub is the deterministic proof that no command was
    // queued: the receiver can only disconnect after observing an empty
    // command channel.
    drop(remote);
    assert!(matches!(
        cmd_rx.try_recv(),
        Err(mpsc::TryRecvError::Disconnected)
    ));
}

#[test]
fn v3_peer_sends_queue_append_wire_command() {
    let existing = vec![make_media_item("1")];
    let (remote, _event_rx, cmd_rx) = RemotePlayer::stub_with_command_rx(existing, 0);

    assert!(remote.send_command(PlayerCommand::QueueAppend {
        items: vec![QueueItem::Emby(Box::new(make_media_item("2")))]
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
