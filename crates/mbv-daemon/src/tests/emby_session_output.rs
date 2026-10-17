// emby-session-owner-output task 3.2: `SelectPlaybackOutput(EmbySession)` —
// the hook and owner-context gates, the device lookup on the output's worker,
// the swap and broadcast, and the no-op for the device that is already the
// output. Everything runs against the recording `SessionApi` fake.

use super::cast_output::{connect_output_client_with_caps, select_output};
use super::cast_playback::drained_events;
use super::r#loop::{TestLoop, test_loop_with_role};
use super::*;
use mbv_ctrl::{OutputConnection, OutputSelection, OutputState};

use crate::cast::PlaybackOutput;
use crate::emby_session::fake::{FakeSessionApi, SessionCall, session};
use crate::emby_session::worker::spawn_emby_session_worker;

/// The selection the TUI sends for the fake's Session: the device id keys the
/// lookup, the name is the label the ctrl state carries.
fn session_selection() -> OutputSelection {
    OutputSelection::EmbySession {
        device_id: "device-1".to_string(),
        name: "Living room TV".to_string(),
    }
}

/// The `OutputState` a successful selection projects.
fn session_state() -> OutputState {
    OutputState::EmbySession {
        device_id: "device-1".to_string(),
        name: "Living room TV".to_string(),
        connection: OutputConnection::Connected,
        audio_only: false,
        now_playing: None,
    }
}

/// A loop that hosts the Emby Session output: the hook the Local daemon sets,
/// the Emby owner context a selection needs, and a worker whose `SessionApi`
/// is `fake`, so no test reaches the network.
fn loop_with_session(fake: FakeSessionApi) -> TestLoop {
    let mut t = test_loop_with_role(DaemonRole::Local);
    t.event_loop.emby_session_output = true;
    t.event_loop.emby_runtime = Some(crate::EmbyOwnerContext::from_client(
        t.event_loop.client.lock().unwrap().clone(),
        1,
    ));
    t.event_loop.emby_session_worker = Some(Box::new(move || {
        let fake = fake.clone();
        spawn_emby_session_worker(move || fake)
    }));
    t
}

/// Pumps the one `DaemonEvent::EmbySession` the selection's lookup produced
/// into the loop.
fn pump_session_event(t: &mut TestLoop) {
    let event = t.merged_rx.recv().expect("the worker returns the lookup");
    assert!(
        matches!(&event, DaemonEvent::EmbySession(_)),
        "the lookup result is the returned event"
    );
    t.event_loop.handle_event(event);
}

/// Contract: row 3.2 / design D2 — a runtime without the hook (mbvd, or a
/// Local daemon started without it) rejects the selection with a reason and
/// keeps local playback.
#[test]
fn a_session_selection_with_the_hook_off_is_rejected_and_keeps_local() {
    let mut t = test_loop_with_role(DaemonRole::Local);
    t.event_loop.emby_runtime = Some(crate::EmbyOwnerContext::from_client(
        t.event_loop.client.lock().unwrap().clone(),
        1,
    ));
    let (client_id, reply_tx, rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);

    select_output(&mut t, client_id, &reply_tx, session_selection());

    assert!(
        matches!(recv_event(&rx), CtrlEvent::CommandRejected(reason) if !reason.is_empty()),
        "the rejection carries a reason"
    );
    assert!(matches!(t.event_loop.owner.output, PlaybackOutput::Local));
    assert_eq!(
        *t.event_loop.shared_queue.playback_output.lock().unwrap(),
        OutputState::Local
    );
}

/// Contract: row 3.2 / design D4 — a device that the unfiltered Session list
/// does not contain keeps the current output and reports the reason to the
/// selecting Client.
#[test]
fn a_session_that_is_not_listed_is_rejected_and_keeps_local() {
    let fake = FakeSessionApi::with_sessions(vec![session("device-other", "session-1")]);
    let calls = fake.calls();
    let mut t = loop_with_session(fake);
    let (client_id, reply_tx, rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);

    select_output(&mut t, client_id, &reply_tx, session_selection());
    pump_session_event(&mut t);

    assert!(
        matches!(recv_event(&rx), CtrlEvent::CommandRejected(reason) if reason.contains("device-1")),
        "the rejection names the device that is not listed"
    );
    assert!(matches!(t.event_loop.owner.output, PlaybackOutput::Local));
    assert_eq!(
        *t.event_loop.shared_queue.playback_output.lock().unwrap(),
        OutputState::Local
    );
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        &[SessionCall::GetSessions],
        "the lookup ran on the output's worker"
    );
}

/// Contract: row 3.2 / design D3 — a successful selection swaps the output,
/// records the resolved session id, and broadcasts the state to peers that
/// negotiated `emby-session-output`, while a peer without it receives `Local`.
#[test]
fn a_successful_selection_broadcasts_the_session_output_to_capable_peers() {
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let mut t = loop_with_session(fake);
    let (capable_id, capable_tx, capable_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let (_plain_id, _plain_tx, plain_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, false);

    select_output(&mut t, capable_id, &capable_tx, session_selection());
    pump_session_event(&mut t);

    assert_eq!(
        format!("{:?}", recv_event(&capable_rx)),
        format!("{:?}", CtrlEvent::PlaybackOutput(session_state())),
        "a capable peer receives the Session output"
    );
    assert!(
        matches!(
            recv_event(&plain_rx),
            CtrlEvent::PlaybackOutput(OutputState::Local)
        ),
        "a peer without the capability receives Local in its place"
    );
    let PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is the output");
    };
    assert_eq!(output.device_id, "device-1");
    assert_eq!(output.session_id.as_deref(), Some("session-1"));
    assert_eq!(
        *t.event_loop.shared_queue.playback_output.lock().unwrap(),
        session_state()
    );
}

/// Contract: row 3.2 / design D11 — selecting the device that already is the
/// output does nothing: no second lookup, no rejection, no broadcast.
#[test]
fn selecting_the_device_that_is_already_the_output_does_nothing() {
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let calls = fake.calls();
    let mut t = loop_with_session(fake);
    let (client_id, reply_tx, rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);

    select_output(&mut t, client_id, &reply_tx, session_selection());
    pump_session_event(&mut t);
    let _ = recv_event(&rx);
    select_output(&mut t, client_id, &reply_tx, session_selection());

    assert!(
        drained_events(&rx).is_empty(),
        "the repeated selection neither reports nor broadcasts"
    );
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        &[SessionCall::GetSessions],
        "no second device lookup runs"
    );
}
