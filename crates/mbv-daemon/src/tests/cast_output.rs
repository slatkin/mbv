// cast-owner-output tasks 3.1 and 3.2: the Owner's playback-output state —
// the Hello capability, the cold seed, and `SelectPlaybackOutput` handling
// against a fake Cast connector.

use super::*;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

use mbv_cast::ConnectMode;
use mbv_ctrl::{CastConnection, OutputSelection, OutputState};

use super::r#loop::{TestLoop, test_loop_with_queue, test_loop_with_role};
use crate::cast::PlaybackOutput;

/// A Cast connector whose connect always succeeds, backed by a fake
/// transport; returns the connector and the transport's call log.
fn fake_connector() -> (CastConnector, Arc<Mutex<Vec<String>>>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&calls);
    let connector: CastConnector = Box::new(move |request: CastConnectRequest| {
        let transport = mbv_cast::FakeCastTransport {
            calls: Arc::clone(&log),
            status: None,
        };
        let (worker, _calls) = mbv_cast::spawn_fake_cast_worker(transport);
        let _ = request.reply.send(CastConnectOutcome::Connected(worker));
    });
    (connector, calls)
}

/// A Cast connector whose connect always succeeds, backed by a fake
/// transport whose status poll answers `status`; records each connect's
/// `ConnectMode` so a test can tell a first select from a reconnect.
type RecordedConnector = (
    CastConnector,
    Arc<Mutex<Vec<String>>>,
    Arc<Mutex<Vec<ConnectMode>>>,
);

fn recording_connector(status: Option<mbv_cast::CastStatus>) -> RecordedConnector {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let modes = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&calls);
    let mode_log = Arc::clone(&modes);
    let connector: CastConnector = Box::new(move |request: CastConnectRequest| {
        mode_log.lock().unwrap().push(request.mode);
        let transport = mbv_cast::FakeCastTransport {
            calls: Arc::clone(&log),
            status: status.clone(),
        };
        let (worker, _calls) = mbv_cast::spawn_fake_cast_worker(transport);
        let _ = request.reply.send(CastConnectOutcome::Connected(worker));
    });
    (connector, calls, modes)
}

/// A Cast connector whose connect always fails with `reason`.
fn failing_connector(reason: &'static str) -> CastConnector {
    Box::new(move |request: CastConnectRequest| {
        let _ = request
            .reply
            .send(CastConnectOutcome::Failed(reason.to_string()));
    })
}

/// A ctrl Client that negotiated `playback-output` at Hello.
fn connect_output_client(
    clients: &ClientRegistry,
    supports_playback_output: bool,
) -> (u64, CtrlSender, mpsc::Receiver<CtrlOutbound>) {
    let (tx, rx) = mpsc::channel();
    let id = clients.lock().unwrap().connect_with_role(
        tx.clone(),
        CtrlTransport::Local,
        mbv_ctrl::CtrlAudiobookshelfCapabilities::default(),
        false,
        supports_playback_output,
        None,
        crate::ctrl::CtrlConnectionRole::Client,
    );
    (id, tx, rx)
}

fn cast_selection() -> OutputSelection {
    cast_selection_for("rcv-1", "Living room")
}

fn cast_selection_for(receiver_id: &str, name: &str) -> OutputSelection {
    OutputSelection::Cast {
        receiver_id: receiver_id.to_string(),
        name: name.to_string(),
        host: "192.0.2.10".to_string(),
        port: 8009,
    }
}

/// Pumps the one `DaemonEvent::Cast` the connector's outcome produced into
/// the loop.
fn pump_cast_event(t: &mut TestLoop) {
    let event = t
        .merged_rx
        .recv()
        .expect("the connector reports one outcome");
    t.event_loop.handle_event(event);
}

fn select_output(
    t: &mut TestLoop,
    client_id: u64,
    reply_tx: &CtrlSender,
    selection: OutputSelection,
) {
    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::SelectPlaybackOutput(selection),
        client_id,
        reply_tx.clone(),
    ));
}

/// Spawns a ctrl session over a socket pair whose daemon Hello advertises
/// `daemon_playback_output` and whose Client Hello negotiates
/// `client_playback_output`. Returns the client-side reader; the daemon Hello
/// is its first line.
fn spawn_cast_peer(
    daemon_playback_output: bool,
    client_playback_output: bool,
) -> BufReader<UnixStream> {
    let (client, peer) = UnixStream::pair().unwrap();
    client
        .set_read_timeout(Some(std::time::Duration::from_secs(1)))
        .unwrap();
    let (merged_tx, _merged_rx) = mpsc::channel();
    let clients = Arc::new(Mutex::new(CtrlClients::default()));
    let player = cold_player();
    crate::spawn_ctrl_client(
        SocketStream::Unix(peer),
        CtrlTransport::Local,
        merged_tx,
        clients,
        None,
        player.status,
        shared_queue_state(),
        crate::DaemonHelloCaps {
            audio_only: false,
            playback_output: daemon_playback_output,
        },
        DaemonRole::Local,
        crate::owner_settings::fixed_reader(false),
    );
    let reader = BufReader::new(client.try_clone().unwrap());
    let mut client_hello = CtrlHello::current();
    if client_playback_output {
        client_hello
            .capabilities
            .push(mbv_ctrl::CTRL_CAP_PLAYBACK_OUTPUT.to_string());
    }
    let mut writer = reader.get_ref().try_clone().unwrap();
    writeln!(
        writer,
        "{}",
        serde_json::to_string(&CtrlCmd::Hello(client_hello)).unwrap()
    )
    .unwrap();
    reader
}

fn read_event(reader: &mut BufReader<UnixStream>) -> CtrlEvent {
    let mut line = String::new();
    assert!(
        reader.read_line(&mut line).unwrap() > 0,
        "connection closed"
    );
    serde_json::from_str(line.trim_end()).unwrap()
}

/// Contract: cast-owner-output row 3.1 — the daemon Hello advertises
/// `playback-output` when the runtime supplies a Cast connector.
#[test]
fn the_daemon_hello_advertises_playback_output_with_a_connector() {
    let mut reader = spawn_cast_peer(true, false);
    let CtrlEvent::Hello(hello) = read_event(&mut reader) else {
        panic!("expected daemon hello");
    };
    assert!(hello.supports_playback_output());
}

/// Contract: cast-owner-output row 3.1 — the daemon Hello hides
/// `playback-output` when the runtime supplies no Cast connector (mbvd).
#[test]
fn the_daemon_hello_hides_playback_output_without_a_connector() {
    let mut reader = spawn_cast_peer(false, false);
    let CtrlEvent::Hello(hello) = read_event(&mut reader) else {
        panic!("expected daemon hello");
    };
    assert!(!hello.supports_playback_output());
}

/// Contract: cast-owner-output row 3.2 — a Client that negotiated
/// `playback-output` receives the owner's output on connect (spec "Client
/// connects to an owner that is casting"); the seed here is the Local start.
#[test]
fn a_capable_client_receives_the_playback_output_on_connect() {
    let mut reader = spawn_cast_peer(false, true);
    let CtrlEvent::Hello(_) = read_event(&mut reader) else {
        panic!("expected daemon hello");
    };
    assert!(matches!(
        read_event(&mut reader),
        CtrlEvent::UnifiedQueueState(_)
    ));
    assert!(
        matches!(
            read_event(&mut reader),
            CtrlEvent::PlaybackOutput(OutputState::Local)
        ),
        "a cold owner seeds Local playback output"
    );
}

/// Contract: cast-owner-output row 3.2 — without a connector, a Cast
/// selection is rejected with a reason and the output stays Local.
#[test]
fn a_cast_selection_without_a_connector_is_rejected() {
    let mut t = test_loop_with_role(DaemonRole::Local);
    let (client_id, reply_tx, rx) = connect_output_client(&t.event_loop.ctrl_clients, true);

    select_output(&mut t, client_id, &reply_tx, cast_selection());

    assert!(
        matches!(recv_event(&rx), CtrlEvent::CommandRejected(_)),
        "a Cast selection without a connector is rejected"
    );
    assert!(matches!(t.event_loop.owner.output, PlaybackOutput::Local));
    assert_eq!(
        *t.event_loop.shared_queue.playback_output.lock().unwrap(),
        OutputState::Local
    );
}

/// Contract: cast-owner-output row 3.2 — a successful Cast selection swaps
/// the output and broadcasts it, only to peers that negotiated
/// `playback-output`.
#[test]
fn a_successful_cast_selection_swaps_the_output_and_broadcasts_to_capable_peers() {
    let mut t = test_loop_with_role(DaemonRole::Local);
    let (connector, _calls) = fake_connector();
    t.event_loop.cast_connector = Some(connector);
    let (capable_id, capable_tx, capable_rx) =
        connect_output_client(&t.event_loop.ctrl_clients, true);
    let (plain_id, plain_tx, plain_rx) = connect_output_client(&t.event_loop.ctrl_clients, false);

    select_output(&mut t, capable_id, &capable_tx, cast_selection());
    pump_cast_event(&mut t);

    let expected = CtrlEvent::PlaybackOutput(OutputState::Cast {
        receiver_id: "rcv-1".to_string(),
        name: "Living room".to_string(),
        connection: CastConnection::Connected,
    });
    assert_eq!(
        format!("{:?}", recv_event(&capable_rx)),
        format!("{expected:?}"),
        "a successful select broadcasts the new playback output"
    );
    assert!(matches!(t.event_loop.owner.output, PlaybackOutput::Cast(_)));
    assert_eq!(
        *t.event_loop.shared_queue.playback_output.lock().unwrap(),
        OutputState::Cast {
            receiver_id: "rcv-1".to_string(),
            name: "Living room".to_string(),
            connection: CastConnection::Connected,
        }
    );
    // The non-capable peer receives nothing from the broadcast: the next
    // event on its channel is a reply to a command of its own.
    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::PlayerCmd(mbv_ctrl::WireCommand::JumpTo(0)),
        plain_id,
        plain_tx,
    ));
    assert!(matches!(
        recv_event(&plain_rx),
        CtrlEvent::CommandRejected(_)
    ));
}

/// Contract: cast-owner-output row 3.2 — a failed connect keeps the current
/// output and reports the failure to the selecting Client.
#[test]
fn a_failed_cast_connect_keeps_the_local_output() {
    let mut t = test_loop_with_role(DaemonRole::Local);
    t.event_loop.cast_connector = Some(failing_connector("receiver unreachable"));
    let (client_id, reply_tx, rx) = connect_output_client(&t.event_loop.ctrl_clients, true);

    select_output(&mut t, client_id, &reply_tx, cast_selection());
    pump_cast_event(&mut t);

    let CtrlEvent::CommandRejected(reason) = recv_event(&rx) else {
        panic!("a failed connect reports to the selecting Client");
    };
    assert!(reason.contains("receiver unreachable"), "reason: {reason}");
    assert!(matches!(t.event_loop.owner.output, PlaybackOutput::Local));
    assert_eq!(
        *t.event_loop.shared_queue.playback_output.lock().unwrap(),
        OutputState::Local
    );
}

/// Contract: correction P2 — local playback that starts after the select but
/// before the connect outcome must not end in two live outputs: the late
/// success is discarded and reported, and the output stays Local.
#[test]
fn a_cast_success_after_local_playback_starts_keeps_the_local_output() {
    let mut t = test_loop_with_role(DaemonRole::Local);
    let (connector, _calls) = fake_connector();
    t.event_loop.cast_connector = Some(connector);
    let (client_id, reply_tx, rx) = connect_output_client(&t.event_loop.ctrl_clients, true);

    select_output(&mut t, client_id, &reply_tx, cast_selection());
    t.event_loop.player.status.lock().unwrap().active = true;
    pump_cast_event(&mut t);

    let CtrlEvent::CommandRejected(reason) = recv_event(&rx) else {
        panic!("a late success against active playback is reported");
    };
    assert!(
        reason.contains("started while connecting"),
        "reason: {reason}"
    );
    assert!(matches!(t.event_loop.owner.output, PlaybackOutput::Local));
    assert_eq!(
        *t.event_loop.shared_queue.playback_output.lock().unwrap(),
        OutputState::Local
    );
}

/// Contract: correction P2 — a second Cast select supersedes the in-flight
/// connect: the first selector is told (it never hangs), and the last
/// selection wins.
#[test]
fn a_second_cast_select_supersedes_the_pending_connect() {
    let mut t = test_loop_with_role(DaemonRole::Local);
    let (connector, _calls) = fake_connector();
    t.event_loop.cast_connector = Some(connector);
    let (first_id, first_tx, first_rx) = connect_output_client(&t.event_loop.ctrl_clients, true);
    let (second_id, second_tx, _second_rx) =
        connect_output_client(&t.event_loop.ctrl_clients, true);

    select_output(
        &mut t,
        first_id,
        &first_tx,
        cast_selection_for("rcv-1", "Living room"),
    );
    select_output(
        &mut t,
        second_id,
        &second_tx,
        cast_selection_for("rcv-2", "Bedroom"),
    );
    // Both connects report; arrival order is thread timing, and each outcome
    // applies by receiver id, so both are pumped before asserting.
    pump_cast_event(&mut t);
    pump_cast_event(&mut t);

    let CtrlEvent::CommandRejected(reason) = recv_event(&first_rx) else {
        panic!("the displaced selector is told");
    };
    assert!(reason.contains("superseded"), "reason: {reason}");
    let PlaybackOutput::Cast(_) = &t.event_loop.owner.output else {
        panic!("the last selection wins");
    };
    assert_eq!(
        crate::cast::output_state(&t.event_loop.owner.output),
        OutputState::Cast {
            receiver_id: "rcv-2".to_string(),
            name: "Bedroom".to_string(),
            connection: CastConnection::Connected,
        }
    );
}

/// Contract: correction P2 — selecting Local while a Cast connect is in
/// flight cancels it: the selector is told, the late success is discarded,
/// and the output stays Local.
#[test]
fn a_local_select_cancels_a_pending_cast_connect() {
    let mut t = test_loop_with_role(DaemonRole::Local);
    let (connector, _calls) = fake_connector();
    t.event_loop.cast_connector = Some(connector);
    let (client_id, reply_tx, rx) = connect_output_client(&t.event_loop.ctrl_clients, true);

    select_output(&mut t, client_id, &reply_tx, cast_selection());
    select_output(&mut t, client_id, &reply_tx, OutputSelection::Local);
    pump_cast_event(&mut t);

    let CtrlEvent::CommandRejected(reason) = recv_event(&rx) else {
        panic!("the cancelled selector is told");
    };
    assert!(reason.contains("kept local playback"), "reason: {reason}");
    assert!(matches!(t.event_loop.owner.output, PlaybackOutput::Local));
    assert_eq!(
        *t.event_loop.shared_queue.playback_output.lock().unwrap(),
        OutputState::Local
    );
}

/// Contract: cast-owner-output row 3.3 — a status poll that fails marks the
/// output disconnected, broadcasts the change, and reconnects in reconnect
/// mode (design D8). The queue and its active slot are untouched: the
/// connection loss says nothing about the owner's playback intent.
#[test]
fn a_failed_status_poll_disconnects_broadcasts_and_reconnects() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![emby_qi("a", "Video", "Movie")],
        0,
    );
    let (connector, _calls, modes) = recording_connector(None);
    t.event_loop.cast_connector = Some(connector);
    let (client_id, reply_tx, rx) = connect_output_client(&t.event_loop.ctrl_clients, true);

    select_output(&mut t, client_id, &reply_tx, cast_selection());
    pump_cast_event(&mut t);
    assert!(
        matches!(
            recv_event(&rx),
            CtrlEvent::PlaybackOutput(OutputState::Cast {
                connection: CastConnection::Connected,
                ..
            })
        ),
        "the select is broadcast before the tick"
    );
    let revision_before = t.event_loop.owner.core.queue.revision();
    let active_before = t.event_loop.owner.core.queue.slots()[0].slot_id;

    let start = std::time::Instant::now();
    t.event_loop.tick(start);
    let event = t.merged_rx.recv().expect("the poll reports its result");
    t.event_loop.handle_event(event);
    let event = t
        .merged_rx
        .recv()
        .expect("the reconnect reports its outcome");
    t.event_loop.handle_event(event);

    assert_eq!(
        modes.lock().unwrap().clone(),
        vec![ConnectMode::FirstSelect, ConnectMode::Reconnect],
        "the loss reconnects in reconnect mode"
    );
    assert_eq!(
        crate::cast::output_state(&t.event_loop.owner.output),
        OutputState::Cast {
            receiver_id: "rcv-1".to_string(),
            name: "Living room".to_string(),
            connection: CastConnection::Connected,
        },
        "a successful reconnect restores the connection"
    );
    assert_eq!(t.event_loop.owner.core.queue.revision(), revision_before);
    assert_eq!(
        t.event_loop.owner.core.queue.slots()[0].slot_id,
        active_before
    );
    assert!(
        matches!(
            recv_event(&rx),
            CtrlEvent::PlaybackOutput(OutputState::Cast {
                connection: CastConnection::Disconnected,
                ..
            })
        ),
        "the loss is broadcast"
    );
    assert!(
        matches!(
            recv_event(&rx),
            CtrlEvent::PlaybackOutput(OutputState::Cast {
                connection: CastConnection::Connected,
                ..
            })
        ),
        "the restored connection is broadcast"
    );
}

/// Contract: cast-owner-output row 3.3 — a poll whose worker event never
/// arrives within the answer bound is a wedged worker (`rust_cast` has no
/// socket timeout): the tick marks the output disconnected and reconnects.
/// The status event the wedged poll eventually produced sits unpumped, as it
/// would on a wedged loop, and must not disturb the recovery.
#[test]
fn a_poll_with_no_worker_event_within_the_answer_bound_disconnects() {
    let mut t = test_loop_with_role(crate::DaemonRole::Local);
    let (connector, _calls, modes) = recording_connector(Some(mbv_cast::CastStatus::idle()));
    t.event_loop.cast_connector = Some(connector);
    let (client_id, reply_tx, rx) = connect_output_client(&t.event_loop.ctrl_clients, true);

    select_output(&mut t, client_id, &reply_tx, cast_selection());
    pump_cast_event(&mut t);
    assert!(
        matches!(
            recv_event(&rx),
            CtrlEvent::PlaybackOutput(OutputState::Cast {
                connection: CastConnection::Connected,
                ..
            })
        ),
        "the select is broadcast before the tick"
    );

    let start = std::time::Instant::now();
    // The poll is submitted; no worker event is pumped back.
    t.event_loop.tick(start);
    t.event_loop
        .tick(start + crate::cast::CAST_JOB_ANSWER_BOUND + std::time::Duration::from_secs(1));

    assert_eq!(
        crate::cast::output_state(&t.event_loop.owner.output),
        OutputState::Cast {
            receiver_id: "rcv-1".to_string(),
            name: "Living room".to_string(),
            connection: CastConnection::Disconnected,
        },
        "the wedged worker's poll is treated as a lost connection"
    );
    // The late status of the wedged poll is ignored, then the reconnect
    // outcome restores the connection.
    let event = t.merged_rx.recv().unwrap();
    t.event_loop.handle_event(event);
    let event = t.merged_rx.recv().unwrap();
    t.event_loop.handle_event(event);
    assert_eq!(
        modes.lock().unwrap().clone(),
        vec![ConnectMode::FirstSelect, ConnectMode::Reconnect]
    );
    assert!(
        matches!(
            crate::cast::output_state(&t.event_loop.owner.output),
            OutputState::Cast {
                connection: CastConnection::Connected,
                ..
            }
        ),
        "the reconnect restores the connection"
    );
    assert!(
        matches!(
            recv_event(&rx),
            CtrlEvent::PlaybackOutput(OutputState::Cast {
                connection: CastConnection::Disconnected,
                ..
            })
        ),
        "the loss is broadcast"
    );
}

/// Contract: cast-owner-output row 3.3 — a returned status disarms the
/// watchdog clock and re-arms the poll cadence: a later tick past the poll
/// interval polls again, and no reconnect fires.
#[test]
fn a_returned_status_rearms_the_poll_and_keeps_the_connection() {
    let mut t = test_loop_with_role(crate::DaemonRole::Local);
    let (connector, _calls, modes) = recording_connector(Some(mbv_cast::CastStatus::idle()));
    t.event_loop.cast_connector = Some(connector);
    let (client_id, reply_tx, _rx) = connect_output_client(&t.event_loop.ctrl_clients, true);

    select_output(&mut t, client_id, &reply_tx, cast_selection());
    pump_cast_event(&mut t);

    let start = std::time::Instant::now();
    t.event_loop.tick(start);
    let event = t
        .merged_rx
        .recv()
        .expect("the first poll reports its status");
    t.event_loop.handle_event(event);

    t.event_loop
        .tick(start + crate::cast::CAST_STATUS_POLL_INTERVAL + std::time::Duration::from_secs(1));
    let event = t
        .merged_rx
        .recv()
        .expect("the poll cadence re-arms and polls again");
    t.event_loop.handle_event(event);

    assert_eq!(
        modes.lock().unwrap().clone(),
        vec![ConnectMode::FirstSelect],
        "a responsive poll never reconnects"
    );
    assert!(
        matches!(t.event_loop.owner.output, PlaybackOutput::Cast(_)),
        "the connection stays"
    );
}

/// The owner's `OutputState` for the test receiver in `connection`.
fn cast_output_state(connection: CastConnection) -> OutputState {
    OutputState::Cast {
        receiver_id: "rcv-1".to_string(),
        name: "Living room".to_string(),
        connection,
    }
}

/// A hand-stamped late result of the old worker: the same shape the worker
/// sends, carrying a stale poll id.
fn stale_status(poll_id: u64) -> DaemonEvent {
    DaemonEvent::Cast(crate::cast::CastOwnerEvent::Status {
        receiver_id: "rcv-1".to_string(),
        poll_id,
        result: Err("late worker lost the connection".to_string()),
    })
}

/// Contract: review P2 on row 3.3 — a late result from the previous worker
/// (the watchdog reconnect race) carries a stale poll id and is dropped: it
/// causes no extra disconnect/reconnect flap and leaves the new poll's
/// watchdog armed.
#[test]
fn a_late_result_from_the_old_worker_leaves_the_new_poll_alone() {
    let mut t = test_loop_with_role(crate::DaemonRole::Local);
    let (connector, _calls, modes) = recording_connector(None);
    t.event_loop.cast_connector = Some(connector);
    let (client_id, reply_tx, rx) = connect_output_client(&t.event_loop.ctrl_clients, true);

    select_output(&mut t, client_id, &reply_tx, cast_selection());
    pump_cast_event(&mut t);
    assert!(matches!(
        recv_event(&rx),
        CtrlEvent::PlaybackOutput(state) if state == cast_output_state(CastConnection::Connected)
    ));

    let start = std::time::Instant::now();
    // Poll P1 goes to worker W1; its result sits unpumped.
    t.event_loop.tick(start);
    // W1's blocking status exceeds the answer bound: the watchdog disconnects
    // and reconnects.
    t.event_loop
        .tick(start + crate::cast::CAST_JOB_ANSWER_BOUND + std::time::Duration::from_secs(1));
    assert!(matches!(
        recv_event(&rx),
        CtrlEvent::PlaybackOutput(state) if state == cast_output_state(CastConnection::Disconnected)
    ));

    // W1's late result arrives while no poll is in flight: dropped silently.
    let event = t.merged_rx.recv().unwrap();
    let DaemonEvent::Cast(crate::cast::CastOwnerEvent::Status {
        poll_id: stale_poll_id,
        ..
    }) = event
    else {
        panic!("expected the wedged poll's late status result");
    };
    t.event_loop.handle_event(stale_status(stale_poll_id));
    assert!(rx.try_recv().is_err(), "the stale late result is silent");
    assert_eq!(
        crate::cast::output_state(&t.event_loop.owner.output),
        cast_output_state(CastConnection::Disconnected),
        "the stale late result causes no flap"
    );

    // The reconnect restores worker W2.
    let event = t
        .merged_rx
        .recv()
        .expect("the reconnect reports its outcome");
    t.event_loop.handle_event(event);
    assert!(matches!(
        recv_event(&rx),
        CtrlEvent::PlaybackOutput(state) if state == cast_output_state(CastConnection::Connected)
    ));

    // The next tick polls W2 (P2).
    t.event_loop
        .tick(start + std::time::Duration::from_secs(40));

    // A late result of the old worker arrives while P2 is in flight: dropped
    // by its stale id — no spurious disconnect.
    t.event_loop.handle_event(stale_status(stale_poll_id));
    assert!(rx.try_recv().is_err(), "the stale late result is silent");
    assert_eq!(
        crate::cast::output_state(&t.event_loop.owner.output),
        cast_output_state(CastConnection::Connected),
        "the stale late result does not disconnect the restored worker"
    );

    // P2's watchdog was not disarmed by the stale result: a tick past the
    // answer bound still fires it.
    t.event_loop
        .tick(start + std::time::Duration::from_secs(40) + crate::cast::CAST_JOB_ANSWER_BOUND);
    assert!(matches!(
        recv_event(&rx),
        CtrlEvent::PlaybackOutput(state) if state == cast_output_state(CastConnection::Disconnected)
    ));
    // The reconnect the fired watchdog spawns is the only one after the
    // recovery: the stale late results spawned none.
    assert_eq!(
        modes.lock().unwrap().clone(),
        vec![
            ConnectMode::FirstSelect,
            ConnectMode::Reconnect,
            ConnectMode::Reconnect
        ],
    );
}
