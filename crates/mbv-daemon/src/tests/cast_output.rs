// cast-owner-output tasks 3.1 and 3.2: the Owner's playback-output state —
// the Hello capability, the cold seed, and `SelectPlaybackOutput` handling
// against a fake Cast connector.

use super::*;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

use mbv_ctrl::{CastConnection, OutputSelection, OutputState};

use super::r#loop::{TestLoop, test_loop_with_role};
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
