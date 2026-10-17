// Unit tests for the extracted `DaemonLoop` (change split-daemon-event-loop,
// §3). These build a loop with a recording store so the per-pass persistence
// can be asserted without touching real state files, and drive one event at a
// time through `handle_event` so the process never exits.

use super::*;
use mbv_ids::ItemId;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;
use tracing_subscriber::prelude::*;

pub(super) type Persisted = Rc<RefCell<Vec<RecordedSnapshot>>>;

/// Cloneable projection of a persisted snapshot (`StayAliveQueueState` is not
/// `Clone`), holding the fields these tests assert on.
pub(super) struct RecordedSnapshot {
    item_ids: Vec<String>,
    source: QueueSource,
    cursor: usize,
}

impl RecordedSnapshot {
    fn of(state: &StayAliveQueueState) -> Self {
        Self {
            item_ids: state
                .queue
                .items
                .iter()
                .map(|i| i.id().to_string())
                .collect(),
            source: state.queue.source.clone(),
            cursor: state.queue.cursor,
        }
    }
}

pub(super) struct TestLoop {
    pub(super) event_loop: DaemonLoop,
    pub(super) persisted: Persisted,
    pub(super) settings: Arc<Mutex<OwnerSettings>>,
    pub(super) merged_rx: mpsc::Receiver<DaemonEvent>,
}

pub(super) fn test_loop_with_role(role: crate::DaemonRole) -> TestLoop {
    test_loop_with_queue(role, Vec::new(), 0)
}

/// A `TrayState` whose hook never produces a Tray: for loops whose events
/// never reach the tray path.
fn null_tray() -> TrayState {
    let (tray_tx, _tray_rx) = mpsc::sync_channel(1);
    TrayState::new(Box::new(|_| None), tray_tx)
}

#[test]
fn audiobookshelf_acknowledged_progress_is_followed_by_queue_broadcast() {
    let mut fixture =
        test_loop_with_queue(crate::DaemonRole::Local, vec![abs_qi("li_1", "ep_1")], 0);
    let generation = SetupGeneration::new(1);
    fixture.event_loop.audiobookshelf_runtime = Some(AudiobookshelfOwnerContext {
        setup: mbv_config::AudiobookshelfSetup::default(),
        device_id: "test-device".into(),
        generation,
    });
    let (_client_id, client_rx) =
        connect_client(&mut fixture.event_loop.ctrl_clients.lock().unwrap());

    fixture
        .event_loop
        .handle_event(DaemonEvent::AudiobookshelfProgress(
            AudiobookshelfProgressUpdate {
                generation,
                library_item_id: "li_1".into(),
                episode_id: "ep_1".into(),
                current_time_seconds: 30.0,
                duration_seconds: 100.0,
                is_finished: false,
            },
        ));

    assert!(matches!(
        recv_event(&client_rx),
        CtrlEvent::AudiobookshelfProgress(_)
    ));
    match recv_event(&client_rx) {
        CtrlEvent::UnifiedQueueState(state) => {
            assert_eq!(
                state.slots[0]
                    .item
                    .as_audiobookshelf()
                    .unwrap()
                    .position_ticks,
                30 * mbv_emby_model::TICKS_PER_SECOND
            );
        }
        _ => panic!("expected updated owner queue broadcast"),
    }
}

pub(super) fn test_loop_with_queue(
    role: crate::DaemonRole,
    items: Vec<QueueItem>,
    active: usize,
) -> TestLoop {
    let persisted: Persisted = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&persisted);
    let (merged_tx, merged_rx) = mpsc::channel::<DaemonEvent>();
    let settings = Arc::new(Mutex::new(OwnerSettings::default()));
    let current_settings = Arc::clone(&settings);
    let pin_swap = PinSwapState::new(
        Box::new(|_direction| Err("no swap command in tests".to_string())),
        Box::new(|_message| ()),
        merged_tx.clone(),
        std::sync::Arc::new(crate::PendingSwapToken::default()),
    );
    let event_loop = DaemonLoop {
        owner: owner_with(items, active),
        player: cold_player(),
        shared_queue: shared_queue_state(),
        ctrl_clients: Arc::new(Mutex::new(CtrlClients::new(
            merged_tx.clone(),
            Arc::new(mbv_ctrl::AttachedClientsCell::default()),
            Arc::new(crate::PendingSwapToken::default()),
        ))),
        client: Arc::new(Mutex::new(EmbyClient::new(Config::default()))),
        emby_runtime: None,
        audiobookshelf_runtime: None,
        merged_tx,
        ws_send_tx: None,
        direct_commands: Vec::new(),
        owner_settings: Arc::new(move || *current_settings.lock().unwrap()),
        role,
        audio_only: false,
        last_keepalive: Instant::now(),
        last_capabilities: Instant::now(),
        store: Box::new(move |state| {
            recorded.borrow_mut().push(RecordedSnapshot::of(state));
            Ok(())
        }),
        queue_persist_tx: None,
        tray: null_tray(),
        pin_swap,
        cast_connector: None,
        emby_session_output: false,
        emby_session_worker: None,
    };
    TestLoop {
        event_loop,
        persisted,
        settings,
        merged_rx,
    }
}

fn current_run(event_loop: &DaemonLoop) -> mbv_ctrl::PlaybackGeneration {
    event_loop.player.status.lock().unwrap().sequence_generation
}

/// Runs `run` under the shared logfmt capture layer (structured-logging
/// design D5) and returns the rendered lines.
fn capture_log_lines(run: impl FnOnce()) -> Vec<String> {
    let lines = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = std::sync::Arc::clone(&lines);
    let subscriber =
        tracing_subscriber::registry().with(mbv_core::applog::test_support::capture_layer(
            move |line| sink.lock().unwrap().push(line.to_owned()),
        ));
    tracing::subscriber::with_default(subscriber, run);
    lines.lock().unwrap().clone()
}

#[test]
fn transport_next_dispatches_owner_resolved_canonical_jump() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![
            emby_qi("current", "Video", "Movie"),
            emby_qi("next", "Video", "Movie"),
        ],
        0,
    );
    let commands = t.event_loop.player.spy_on_commands();
    let next = t.event_loop.owner.core.queue.slots()[1].slot_id;
    let resume = mbv_player::resume_ticks_for_slot(&t.event_loop.owner.core.queue, next);

    t.event_loop
        .handle_event(DaemonEvent::Transport(mbv_ctrl::TransportCommand::Step(
            mbv_ctrl::Direction::Next,
        )));

    let command = commands.try_recv();
    assert!(
        matches!(
            command,
            Ok(mbv_ctrl::player::PlayerCommand::JumpTo { slot_id, resume_ticks, .. })
                if slot_id == next && resume_ticks == resume
        ),
        "unexpected command: {command:?}"
    );
}

#[test]
fn transport_stop_stops_the_player_and_clears_slot_jumps() {
    // A transport stop (MPRIS/tray) mirrors `CtrlCmd::Stop`: a stop makes
    // every awaited transition identity unobservable, so an in-flight jump
    // must not survive it and settle against a later run.
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![
            emby_qi("current", "Video", "Movie"),
            emby_qi("next", "Video", "Movie"),
        ],
        0,
    );
    let commands = t.event_loop.player.spy_on_commands();
    t.event_loop
        .handle_event(DaemonEvent::Transport(mbv_ctrl::TransportCommand::Step(
            mbv_ctrl::Direction::Next,
        )));
    let dispatched = commands.try_recv();
    assert!(
        matches!(
            dispatched,
            Ok(mbv_ctrl::player::PlayerCommand::JumpTo { .. })
        ),
        "unexpected dispatch: {dispatched:?}"
    );
    assert!(t.event_loop.owner.core.transitions.in_flight().is_some());

    t.event_loop
        .handle_event(DaemonEvent::Transport(mbv_ctrl::TransportCommand::Stop));

    assert!(t.event_loop.owner.core.transitions.in_flight().is_none());
    assert!(t.event_loop.owner.queued_transition_origin.is_none());
}

#[test]
fn websocket_next_dispatches_owner_resolved_jump_not_run_step() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![
            emby_qi("current", "Video", "Movie"),
            emby_qi("next", "Video", "Movie"),
        ],
        0,
    );
    let commands = t.event_loop.player.spy_on_commands();
    let generation = mbv_core::service_runtime::SetupGeneration::default();
    t.event_loop.emby_runtime = Some(crate::EmbyOwnerContext::from_client(
        t.event_loop.client.lock().unwrap().clone(),
        1,
    ));
    let next = t.event_loop.owner.core.queue.slots()[1].slot_id;
    let resume = mbv_player::resume_ticks_for_slot(&t.event_loop.owner.core.queue, next);

    t.event_loop.handle_ws_event(generation, WsEvent::NextTrack);

    assert!(matches!(
        commands.try_recv(),
        Ok(mbv_ctrl::player::PlayerCommand::JumpTo { slot_id, resume_ticks, .. })
            if slot_id == next && resume_ticks == resume
    ));
}

#[test]
fn daemon_reads_consume_audio_turned_on_during_the_session() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![
            emby_qi("track", "Audio", "Audio"),
            emby_qi("next", "Audio", "Audio"),
        ],
        0,
    );
    t.settings.lock().unwrap().consume.audio = true;
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;

    let flow = t
        .event_loop
        .handle_event(DaemonEvent::Player(PlayerEvent::TrackCompleted {
            slot_id: slot,
            run_identity: current_run(&t.event_loop),
            position_ticks: 0,
            played: true,
            consume: true,
        }));

    assert_eq!(flow, LoopFlow::Continue);
    assert_eq!(t.event_loop.owner.core.queue.slots().len(), 1);
    assert_eq!(t.event_loop.owner.core.queue.slots()[0].item.id(), "next");
    assert_eq!(t.persisted.borrow().len(), 1);
    assert_eq!(t.persisted.borrow()[0].item_ids, vec!["next".to_string()]);
    assert_eq!(t.persisted.borrow()[0].cursor, 0);
}

/// A shutdown request is rejected while stay-alive is on, and the decision
/// reads the settings closure at request time (spawn config stays off).
#[test]
fn daemon_reads_stay_alive_at_shutdown_decision_time_and_rejects_while_on() {
    let mut t = test_loop_with_role(crate::DaemonRole::Local);
    *t.settings.lock().unwrap() = OwnerSettings {
        stay_alive: true,
        ..Default::default()
    };
    let (client_id, _client_rx) = connect_client(&mut t.event_loop.ctrl_clients.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();

    let flow = t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::RequestShutdown,
        client_id,
        reply_tx,
    ));

    assert_eq!(flow, LoopFlow::Continue);
    assert!(matches!(
        recv_event(&reply_rx),
        CtrlEvent::ShutdownRejected { reason } if reason == "daemon is in stay-alive mode"
    ));
    assert!(matches!(
        t.merged_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
}

#[test]
fn ordinary_disconnect_is_not_shutdown_local_role_persists_and_shuts_down_when_stay_alive_is_off() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![emby_qi("persist-on-disconnect", "Audio", "Audio")],
        0,
    );
    let (client_id, _client_rx) = connect_client(&mut t.event_loop.ctrl_clients.lock().unwrap());
    t.event_loop.ctrl_clients.lock().unwrap().remove(client_id);
    let event = t.merged_rx.recv().unwrap();

    assert!(matches!(event, DaemonEvent::LastClientGone));
    assert_eq!(t.event_loop.handle_event(event), LoopFlow::Shutdown);
    assert_eq!(t.persisted.borrow().len(), 1);
}

#[test]
fn ordinary_disconnect_is_not_shutdown_when_reader_says_stay_alive() {
    let mut t = test_loop_with_role(crate::DaemonRole::Local);
    *t.settings.lock().unwrap() = OwnerSettings {
        stay_alive: true,
        ..Default::default()
    };
    let (client_id, _client_rx) = connect_client(&mut t.event_loop.ctrl_clients.lock().unwrap());
    t.event_loop.ctrl_clients.lock().unwrap().remove(client_id);
    let event = t.merged_rx.recv().unwrap();

    assert_eq!(t.event_loop.handle_event(event), LoopFlow::Continue);
    assert!(t.persisted.borrow().is_empty());
}

#[test]
fn ordinary_disconnect_is_not_shutdown_for_packaged_role() {
    let mut t = test_loop_with_role(crate::DaemonRole::Packaged);
    let (client_id, _client_rx) = connect_client(&mut t.event_loop.ctrl_clients.lock().unwrap());
    t.event_loop.ctrl_clients.lock().unwrap().remove(client_id);
    let event = t.merged_rx.recv().unwrap();

    assert_eq!(t.event_loop.handle_event(event), LoopFlow::Continue);
    assert!(t.persisted.borrow().is_empty());
}

#[test]
fn packaged_daemon_reads_stay_alive_as_true_regardless_of_spawn_config() {
    let config = Config {
        stay_alive: false,
        ..Config::default()
    };
    let settings = crate::owner_settings::reader(crate::DaemonRole::Packaged, &config);

    assert!((settings)().stay_alive);
}

#[test]
fn playback_resolved_current_request_replaces_queue_and_persists_once() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![emby_qi("old", "Video", "Movie")],
        0,
    );
    // Keep the player on its submit fast path (status already active) so the
    // replacement seeds state without spawning a real mpv thread.
    t.event_loop.player.status.lock().unwrap().active = true;
    let (client_id, _rx) = connect_client(&mut t.event_loop.ctrl_clients.lock().unwrap());
    let (request_id, generation) = (11, 3);
    t.event_loop.owner.intents.accept(
        client_id,
        PlaybackIntent {
            request_id,
            generation,
            action: PlaybackIntentAction::Play {
                item_ids: vec!["new".into()],
                start_idx: 0,
                start_ticks: 0,
                source: QueueSource::Album,
            },
        },
        false,
    );

    let flow = t.event_loop.handle_event(DaemonEvent::PlaybackResolved {
        start_idx: 0,
        start_ticks: 0,
        source: QueueSource::Album,
        client_id,
        request_id,
        generation,
        fetched: Ok(vec![item("new", "Video", "Movie")]),
    });

    assert_eq!(flow, LoopFlow::Continue);
    assert_eq!(t.event_loop.owner.core.queue.slots()[0].item.id(), "new");
    assert_eq!(t.event_loop.owner.core.source, QueueSource::Album);
    assert_eq!(t.persisted.borrow().len(), 1);
    assert_eq!(t.persisted.borrow()[0].item_ids, vec!["new".to_string()]);
    assert_eq!(t.persisted.borrow()[0].source, QueueSource::Album);
}

#[test]
fn playback_resolved_stale_request_persists_nothing() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![emby_qi("old", "Video", "Movie")],
        0,
    );
    t.event_loop.player.status.lock().unwrap().active = true;
    let (client_id, _rx) = connect_client(&mut t.event_loop.ctrl_clients.lock().unwrap());
    t.event_loop.owner.intents.accept(
        client_id,
        PlaybackIntent {
            request_id: 11,
            generation: 3,
            action: PlaybackIntentAction::Play {
                item_ids: vec!["new".into()],
                start_idx: 0,
                start_ticks: 0,
                source: QueueSource::Album,
            },
        },
        false,
    );

    let flow = t.event_loop.handle_event(DaemonEvent::PlaybackResolved {
        start_idx: 0,
        start_ticks: 0,
        source: QueueSource::Album,
        client_id,
        request_id: 12,
        generation: 3,
        fetched: Ok(vec![item("new", "Video", "Movie")]),
    });

    assert_eq!(flow, LoopFlow::Continue);
    assert_eq!(t.event_loop.owner.core.queue.slots()[0].item.id(), "old");
    assert_eq!(t.event_loop.owner.core.source, QueueSource::Unknown);
    assert!(t.persisted.borrow().is_empty());
}

#[test]
fn playback_resolved_failure_line_carries_intent_client_and_request() {
    // Rejoin-correlation contract (structured-logging): a failed
    // `PlaybackResolved` handled on the event loop logs a line carrying the
    // intent's `client` and `request`, rebuilt from the event's ids.
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![emby_qi("old", "Video", "Movie")],
        0,
    );
    let (client_id, _rx) = connect_client(&mut t.event_loop.ctrl_clients.lock().unwrap());
    let (request_id, generation) = (11, 3);
    t.event_loop.owner.intents.accept(
        client_id,
        PlaybackIntent {
            request_id,
            generation,
            action: PlaybackIntentAction::Play {
                item_ids: vec!["new".into()],
                start_idx: 0,
                start_ticks: 0,
                source: QueueSource::Album,
            },
        },
        false,
    );

    let lines = capture_log_lines(|| {
        t.event_loop.handle_event(DaemonEvent::PlaybackResolved {
            start_idx: 0,
            start_ticks: 0,
            source: QueueSource::Album,
            client_id,
            request_id,
            generation,
            fetched: Err(crate::DaemonLibError::owner_context("lookup failed")),
        });
    });

    assert!(
        lines.iter().any(|line| {
            line.contains("event=ctrl.intent.failed")
                && line.contains(&format!("client={client_id}"))
                && line.contains(&format!("request={request_id}"))
        }),
        "no failure line carrying the intent ids: {lines:?}"
    );
}

/// In Shuffle, the Next-Up threshold card names the slot the core picked, not
/// the queue neighbor (queue-shuffle-play-order task 4.2; queue-play-order
/// spec: one pick drives every forward advance).
#[test]
fn next_up_threshold_in_shuffle_names_the_picked_slot() {
    let mut fixture = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![
            emby_qi("a", "Video", "Movie"),
            emby_qi("b", "Video", "Movie"),
            emby_qi("c", "Video", "Movie"),
        ],
        0,
    );
    let cmd_rx = fixture.event_loop.player.spy_on_commands();
    let slots: Vec<_> = fixture
        .event_loop
        .owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| slot.slot_id)
        .collect();
    fixture
        .event_loop
        .owner
        .core
        .note_observed_active_slot(Some(slots[0]));
    fixture
        .event_loop
        .owner
        .core
        .set_play_order(mbv_queue::PlayOrderMode::Shuffle);
    let mbv_queue::PlayOrder::Shuffle {
        upcoming: Some(picked),
    } = fixture.event_loop.owner.core.play_order()
    else {
        panic!("shuffle over two other slots must pick one");
    };
    // The pick can land on the queue neighbor by chance; moving it to the
    // tail keeps the card's source unambiguous for either outcome.
    fixture.event_loop.owner.core.queue.move_slot(picked, 2);
    let picked_item_id = fixture
        .event_loop
        .owner
        .core
        .queue
        .slot(picked)
        .map(|slot| slot.item.id().to_string())
        .unwrap();

    fixture
        .event_loop
        .handle_event(DaemonEvent::Player(PlayerEvent::NextUpThreshold {
            series_id: ItemId::new("series"),
            season: 1,
            episode: 1,
        }));

    match cmd_rx.recv().unwrap() {
        PlayerCommand::NextUpShow { item_id, .. } => assert_eq!(item_id, picked_item_id),
        other => panic!("expected NextUpShow, got {other:?}"),
    }
}

mod persistence;
