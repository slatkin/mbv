// emby-session-owner-output task 3.2: `SelectPlaybackOutput(EmbySession)` —
// the hook and owner-context gates, the device lookup on the output's worker,
// the swap and broadcast, and the no-op for the device that is already the
// output. Everything runs against the recording `SessionApi` fake.

use super::cast_output::{connect_output_client_with_caps, select_output};
use super::cast_playback::drained_events;
use super::r#loop::{TestLoop, test_loop_with_queue, test_loop_with_role};
use super::*;
use mbv_ctrl::{OutputConnection, OutputSelection, OutputState};

use crate::cast::PlaybackOutput;
use crate::emby_session::EmbySessionEvent;
use crate::emby_session::fake::{FakeSessionApi, SessionCall, session};
use crate::emby_session::poll::{EMBY_SESSION_DISCONNECT_MISSES, EMBY_SESSION_POLL_INTERVAL};
use crate::emby_session::worker::spawn_emby_session_worker;

/// The selection the TUI sends for the fake's Session: the device id keys the
/// lookup, the name is the label the ctrl state carries.
pub(super) fn session_selection() -> OutputSelection {
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
    host_session_output(&mut t, fake);
    t
}

/// Gives `t` the runtime hosting an Emby Session output (design D2, D4): the
/// hook, the Emby owner context a selection needs, and a worker whose
/// `SessionApi` is `fake`. A loop built with [`test_loop_with_queue`] calls
/// this to keep a scripted queue while the Session is the output.
pub(super) fn host_session_output(t: &mut TestLoop, fake: FakeSessionApi) {
    t.event_loop.emby_session_output = true;
    t.event_loop.emby_runtime = Some(crate::EmbyOwnerContext::from_client(
        t.event_loop.client.lock().unwrap().clone(),
        1,
    ));
    t.event_loop.emby_session_worker = Some(Box::new(move || {
        let fake = fake.clone();
        spawn_emby_session_worker(move || fake)
    }));
}

/// Pumps the one `DaemonEvent::EmbySession` the selection's lookup produced
/// into the loop.
pub(super) fn pump_session_event(t: &mut TestLoop) {
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

/// Selects the fake's Session as the output and pumps its lookup, returning
/// the loop and the fake's call log. The fake stays scriptable through a
/// clone the test keeps.
pub(super) fn session_output(fake: FakeSessionApi) -> (TestLoop, Arc<Mutex<Vec<SessionCall>>>) {
    let calls = fake.calls();
    let mut t = loop_with_session(fake);
    let (client_id, reply_tx, _rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    select_output(&mut t, client_id, &reply_tx, session_selection());
    pump_session_event(&mut t);
    (t, calls)
}

/// Waits until every job queued on the Session worker has run, so a test can
/// assert on the fake's call log without a sleep.
pub(super) fn wait_for_session_jobs(output: &PlaybackOutput) {
    let PlaybackOutput::EmbySession(output) = output else {
        panic!("the Session is the output");
    };
    let (done_tx, done_rx) = mpsc::channel();
    output
        .worker
        .send(Box::new(move |_api| {
            let _ = done_tx.send(());
        }))
        .expect("the Session worker accepts the sentinel");
    done_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the queued jobs ran before the sentinel");
}

/// Drives one poll: the tick submits it, the fake answers, and the result
/// re-enters the loop. Returns the poll id the worker stamped.
pub(super) fn poll_once(t: &mut TestLoop, at: std::time::Instant) -> u64 {
    t.event_loop.tick(at);
    let event = t.merged_rx.recv().expect("the poll reports a result");
    let DaemonEvent::EmbySession(EmbySessionEvent::Sessions { poll_id, .. }) = &event else {
        panic!("the tick's poll result is the returned event");
    };
    let poll_id = *poll_id;
    t.event_loop.handle_event(event);
    poll_id
}

/// The number of `/Sessions` calls the fake has seen, including the
/// selection's lookup.
fn sessions_calls(calls: &Arc<Mutex<Vec<SessionCall>>>) -> usize {
    calls
        .lock()
        .unwrap()
        .iter()
        .filter(|call| matches!(call, SessionCall::GetSessions))
        .count()
}

/// The output state the disconnected test asserts was broadcast.
fn disconnected_state() -> OutputState {
    OutputState::EmbySession {
        device_id: "device-1".to_string(),
        name: "Living room TV".to_string(),
        connection: OutputConnection::Disconnected,
        audio_only: false,
        now_playing: None,
    }
}

/// Contract: row 3.3 / design D5 and spec "The Emby Session output follows the
/// device" — three observations in a row that do not list the device broadcast
/// the output as disconnected and leave the queue revision unchanged.
#[test]
fn three_missing_polls_disconnect_the_output_and_leave_the_queue_unchanged() {
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let script = fake.clone();
    let (mut t, _calls) = session_output(fake);
    let (_monitor_id, _monitor_tx, monitor_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let revision_before = t.event_loop.owner.core.queue.revision();
    script.set_sessions(Vec::new());

    let t0 = std::time::Instant::now();
    poll_once(&mut t, t0);
    poll_once(&mut t, t0 + EMBY_SESSION_POLL_INTERVAL);
    assert!(
        monitor_rx.try_recv().is_err(),
        "two misses do not disconnect the output"
    );
    poll_once(&mut t, t0 + 2 * EMBY_SESSION_POLL_INTERVAL);

    assert_eq!(
        format!("{:?}", recv_event(&monitor_rx)),
        format!("{:?}", CtrlEvent::PlaybackOutput(disconnected_state())),
        "the third miss broadcasts the disconnected output"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.revision(),
        revision_before,
        "the misses change no queue revision"
    );
    assert!(t.event_loop.owner.core.queue.slots().is_empty());
    let PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session stays the output");
    };
    assert_eq!(output.connection, OutputConnection::Disconnected);
    assert_eq!(output.misses, EMBY_SESSION_DISCONNECT_MISSES);
}

/// Contract: row 3.3 / design D5 — the device reappearing under a new session
/// id reconnects the output, adopts that id, and the next command addresses
/// it (the command paths of task 4.4 read the adopted id).
#[test]
fn a_reattached_device_under_a_new_session_id_reconnects_and_commands_use_it() {
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let script = fake.clone();
    let (mut t, calls) = session_output(fake);
    let (_monitor_id, _monitor_tx, monitor_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);

    script.set_sessions(Vec::new());
    let t0 = std::time::Instant::now();
    poll_once(&mut t, t0);
    poll_once(&mut t, t0 + EMBY_SESSION_POLL_INTERVAL);
    poll_once(&mut t, t0 + 2 * EMBY_SESSION_POLL_INTERVAL);
    assert!(matches!(
        recv_event(&monitor_rx),
        CtrlEvent::PlaybackOutput(OutputState::EmbySession {
            connection: OutputConnection::Disconnected,
            ..
        })
    ));

    script.set_sessions(vec![session("device-1", "session-2")]);
    poll_once(&mut t, t0 + 3 * EMBY_SESSION_POLL_INTERVAL);

    assert_eq!(
        format!("{:?}", recv_event(&monitor_rx)),
        format!("{:?}", CtrlEvent::PlaybackOutput(session_state())),
        "the reappearing device broadcasts the connected output again"
    );
    let PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is the output");
    };
    assert_eq!(output.connection, OutputConnection::Connected);
    assert_eq!(output.misses, 0);
    assert_eq!(
        output.session_id.as_deref(),
        Some("session-2"),
        "the observation adopts the new session id"
    );

    // The next command goes to the adopted id: every command path addresses
    // the Session through this id, so a transport submitted with it reaches
    // the device under the new session.
    let session_id = output
        .session_id
        .clone()
        .expect("the observation adopted a session id");
    output
        .worker
        .send(Box::new(move |api| {
            let _ = api.transport(&session_id, "Pause");
        }))
        .expect("the Session worker accepts the command");
    wait_for_session_jobs(&t.event_loop.owner.output);
    assert_eq!(
        calls.lock().unwrap().last(),
        Some(&SessionCall::Transport {
            session_id: "session-2".to_string(),
            command: "Pause".to_string(),
        }),
        "the command goes to the new session id"
    );
}

/// Contract: row 3.3 / design D5 — a late result carrying an old poll id is
/// ignored: it adds no miss, leaves the new poll in flight, and changes no
/// state that the current poll's own result then applies.
#[test]
fn a_late_poll_result_with_an_old_poll_id_is_ignored() {
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let (mut t, _calls) = session_output(fake);
    let (_monitor_id, _monitor_tx, monitor_rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let t0 = std::time::Instant::now();

    t.event_loop.tick(t0);
    let first = t.merged_rx.recv().expect("the first poll reports");
    let DaemonEvent::EmbySession(EmbySessionEvent::Sessions {
        poll_id: first_id, ..
    }) = &first
    else {
        panic!("the first poll result is the returned event");
    };
    let first_id = *first_id;
    t.event_loop.handle_event(first);

    t.event_loop.tick(t0 + EMBY_SESSION_POLL_INTERVAL);
    t.event_loop
        .handle_event(DaemonEvent::EmbySession(EmbySessionEvent::Sessions {
            poll_id: first_id,
            result: Ok(Vec::new()),
        }));

    let PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is the output");
    };
    assert_eq!(
        output.poll_sent.map(|(id, _)| id),
        Some(first_id + 1),
        "the stale result leaves the current poll in flight"
    );
    assert_eq!(output.misses, 0, "the stale result adds no miss");
    assert_eq!(output.session_id.as_deref(), Some("session-1"));
    assert!(
        monitor_rx.try_recv().is_err(),
        "the stale result broadcasts nothing"
    );

    let second = t.merged_rx.recv().expect("the current poll reports");
    let DaemonEvent::EmbySession(EmbySessionEvent::Sessions {
        poll_id: second_id, ..
    }) = &second
    else {
        panic!("the current poll result is the returned event");
    };
    assert_eq!(*second_id, first_id + 1, "the poll id is monotonic");
    t.event_loop.handle_event(second);
    let PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the Session is the output");
    };
    assert!(output.poll_sent.is_none(), "the current poll settles");
    assert_eq!(output.misses, 0);
}

/// Contract: row 3.3 / design D5 — the tick polls at most once a second and
/// never with a poll already in flight; the poll id is monotonic.
#[test]
fn the_tick_polls_once_a_second_with_one_poll_in_flight() {
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let (mut t, calls) = session_output(fake);
    let base = sessions_calls(&calls);
    let t0 = std::time::Instant::now();

    t.event_loop.tick(t0);
    wait_for_session_jobs(&t.event_loop.owner.output);
    assert_eq!(sessions_calls(&calls), base + 1, "the first tick polls");

    t.event_loop.tick(t0 + Duration::from_millis(500));
    wait_for_session_jobs(&t.event_loop.owner.output);
    assert_eq!(
        sessions_calls(&calls),
        base + 1,
        "a poll in flight excludes a second one"
    );

    let first = t.merged_rx.recv().expect("the first poll reports");
    let DaemonEvent::EmbySession(EmbySessionEvent::Sessions {
        poll_id: first_id, ..
    }) = &first
    else {
        panic!("the first poll result is the returned event");
    };
    let first_id = *first_id;
    t.event_loop.handle_event(first);

    t.event_loop.tick(t0 + Duration::from_millis(900));
    wait_for_session_jobs(&t.event_loop.owner.output);
    assert_eq!(
        sessions_calls(&calls),
        base + 1,
        "the cadence is one second, not one tick"
    );

    t.event_loop.tick(t0 + EMBY_SESSION_POLL_INTERVAL);
    wait_for_session_jobs(&t.event_loop.owner.output);
    let second = t.merged_rx.recv().expect("the second poll reports");
    let DaemonEvent::EmbySession(EmbySessionEvent::Sessions {
        poll_id: second_id, ..
    }) = &second
    else {
        panic!("the second poll result is the returned event");
    };
    assert_eq!(*second_id, first_id + 1, "the poll id is monotonic");
}

/// A worker whose jobs are dropped unrun, so no lookup outcome races the ones
/// the test injects: the selection bookkeeping is driven exactly.
fn silent_session_worker() -> mpsc::Sender<crate::emby_session::worker::EmbySessionJob> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || for _job in rx {});
    tx
}

/// A loop that hosts the Emby Session output with a worker that never answers,
/// so each lookup outcome arrives when the test injects it.
pub(super) fn loop_with_injected_lookups() -> TestLoop {
    let mut t = test_loop_with_role(DaemonRole::Local);
    t.event_loop.emby_session_output = true;
    t.event_loop.emby_runtime = Some(crate::EmbyOwnerContext::from_client(
        t.event_loop.client.lock().unwrap().clone(),
        1,
    ));
    t.event_loop.emby_session_worker = Some(Box::new(silent_session_worker));
    t
}

/// The selection the TUI sends for `device_id`.
fn session_selection_for(device_id: &str) -> OutputSelection {
    OutputSelection::EmbySession {
        device_id: device_id.to_string(),
        name: "Living room TV".to_string(),
    }
}

/// Injects one lookup outcome as the worker would have returned it.
pub(super) fn inject_lookup_outcome(t: &mut TestLoop, device_id: &str, session_id: &str) {
    t.event_loop
        .handle_event(DaemonEvent::EmbySession(EmbySessionEvent::Selected {
            device_id: device_id.to_string(),
            result: Ok(Box::new(session(device_id, session_id))),
        }));
}

/// Contract: row 3.2 review — a displaced selection's late lookup outcome must
/// not consume the live pending selection, so the newest selection still
/// installs when its own outcome arrives.
#[test]
fn a_stale_lookup_outcome_keeps_the_live_pending_selection() {
    let mut t = loop_with_injected_lookups();
    let (client_id, reply_tx, rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);

    select_output(
        &mut t,
        client_id,
        &reply_tx,
        session_selection_for("device-1"),
    );
    select_output(
        &mut t,
        client_id,
        &reply_tx,
        session_selection_for("device-2"),
    );
    assert!(
        matches!(
            drained_events(&rx).as_slice(),
            [CtrlEvent::CommandRejected(_)]
        ),
        "the displaced first selection is told it lost"
    );

    inject_lookup_outcome(&mut t, "device-1", "session-1");

    assert!(
        matches!(t.event_loop.owner.output, PlaybackOutput::Local),
        "the stale outcome installs nothing"
    );
    assert_eq!(
        t.event_loop
            .owner
            .pending_emby_session_select
            .as_ref()
            .map(|pending| pending.device_id.as_str()),
        Some("device-2"),
        "the live pending selection survives the stale outcome"
    );

    inject_lookup_outcome(&mut t, "device-2", "session-2");

    let PlaybackOutput::EmbySession(output) = &t.event_loop.owner.output else {
        panic!("the newest selection installs after its own outcome");
    };
    assert_eq!(output.device_id, "device-2");
    assert_eq!(output.session_id.as_deref(), Some("session-2"));
}

/// Contract: row 3.2 review / design D11 — a Local selection after an Emby
/// Session selection displaces the pending lookup across output kinds, so its
/// late outcome installs nothing.
#[test]
fn a_local_selection_discards_a_pending_emby_session_outcome() {
    let mut t = loop_with_injected_lookups();
    let (client_id, reply_tx, rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);

    select_output(&mut t, client_id, &reply_tx, session_selection());
    select_output(&mut t, client_id, &reply_tx, OutputSelection::Local);

    assert!(
        matches!(
            drained_events(&rx).as_slice(),
            [CtrlEvent::CommandRejected(_)]
        ),
        "the displaced Session selection is told it lost"
    );
    assert!(t.event_loop.owner.pending_emby_session_select.is_none());

    inject_lookup_outcome(&mut t, "device-1", "session-1");

    assert!(
        matches!(t.event_loop.owner.output, PlaybackOutput::Local),
        "the late outcome installs nothing after a Local selection"
    );
    assert!(
        drained_events(&rx).is_empty(),
        "the late outcome reports nothing"
    );
    assert_eq!(
        *t.event_loop.shared_queue.playback_output.lock().unwrap(),
        OutputState::Local
    );
}

/// Contract: row 9.2 / design D7 and D14 and spec "An Emby Session is the
/// output" — playing a several-hundred-slot queue gives the Session exactly
/// one `play` job, never the whole queue (#945), carrying the selected item
/// and the next admissible slot; the last slot has no slot after it, so the
/// device is given that one item alone.
#[rstest]
#[case::a_middle_slot(5, &["item-5", "item-6"])]
#[case::the_last_slot(299, &["item-299"])]
fn playing_a_300_slot_queue_gives_the_session_one_play_job(
    #[case] start_idx: usize,
    #[case] item_ids: &[&str],
) {
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let (mut t, calls) = session_output(fake);
    let (client_id, _reply_tx, _rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    let (request_id, generation) = (7, 7);
    t.event_loop.owner.intents.accept(
        client_id,
        PlaybackIntent {
            request_id,
            generation,
            action: PlaybackIntentAction::Play {
                item_ids: vec![format!("item-{start_idx}")],
                start_idx,
                start_ticks: 0,
                source: QueueSource::Remote,
            },
        },
        false,
    );
    let fetched: Vec<EmbyItem> = (0..300)
        .map(|index| item(&format!("item-{index}"), "Video", "Movie"))
        .collect();

    t.event_loop.handle_event(DaemonEvent::PlaybackResolved {
        start_idx,
        start_ticks: 0,
        source: QueueSource::Remote,
        client_id,
        request_id,
        generation,
        fetched: Ok(fetched),
    });
    wait_for_session_jobs(&t.event_loop.owner.output);

    assert_eq!(
        play_calls(&calls),
        vec![SessionCall::Play {
            session_id: "session-1".to_string(),
            item_ids: item_ids.iter().map(|id| (*id).to_string()).collect(),
            start_ticks: 0,
        }],
        "the Session is given exactly one play job, for the selected slot's item and the one after it"
    );
    assert_eq!(
        t.event_loop.owner.core.queue.len(),
        300,
        "the owner keeps the whole queue"
    );
    assert!(
        !t.event_loop.player.status.lock().unwrap().active,
        "local playback does not start while the Session is the output"
    );
}

/// Contract: row 4.2 / design D7 and spec "Explicit play of a slot the
/// Session cannot play" — playing an Audiobookshelf slot while the Session is
/// the output reports the item and the reason, sends no `play`, and expires
/// the in-flight transition the jump registered.
#[test]
fn playing_an_audiobookshelf_slot_on_the_session_loads_nothing_and_reports() {
    let fake = FakeSessionApi::with_sessions(vec![session("device-1", "session-1")]);
    let calls = fake.calls();
    let mut t = test_loop_with_queue(
        DaemonRole::Local,
        vec![book_qi("li_1"), emby_qi("movie", "Video", "Movie")],
        0,
    );
    host_session_output(&mut t, fake);
    let (client_id, reply_tx, rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    select_output(&mut t, client_id, &reply_tx, session_selection());
    pump_session_event(&mut t);
    let _ = drained_events(&rx);
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;

    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::UnifiedQueuePlaySlot {
            slot_id: mbv_ctrl::slot_id_to_u64(slot),
            op: None,
        },
        client_id,
        mpsc::channel().0,
    ));
    wait_for_session_jobs(&t.event_loop.owner.output);

    assert!(
        play_calls(&calls).is_empty(),
        "the Session is given no play job for an item it cannot play"
    );
    let rejection = drained_events(&rx)
        .into_iter()
        .find_map(|event| match event {
            CtrlEvent::CommandRejected(reason) => Some(reason),
            _ => None,
        })
        .expect("the unplayable item is reported");
    assert!(
        rejection.contains("Test Book") && rejection.contains("plays only Emby items"),
        "the report names the item and the reason: {rejection}"
    );
    assert!(
        t.event_loop.owner.core.transitions.in_flight().is_none(),
        "the rejected jump expires the in-flight transition"
    );
}

/// The `play` jobs the fake has recorded, in order, so a test can assert the
/// Session received exactly the item it should.
pub(super) fn play_calls(calls: &Arc<Mutex<Vec<SessionCall>>>) -> Vec<SessionCall> {
    calls
        .lock()
        .unwrap()
        .iter()
        .filter(|call| matches!(call, SessionCall::Play { .. }))
        .cloned()
        .collect()
}
