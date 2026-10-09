// Unit tests for the extracted `DaemonLoop` (change split-daemon-event-loop,
// §3). These build a loop with a recording store so the per-pass persistence
// can be asserted without touching real state files, and drive one event at a
// time through `handle_event` so the process never exits.

use super::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;
use tracing_subscriber::prelude::*;

type Persisted = Rc<RefCell<Vec<RecordedSnapshot>>>;

/// Cloneable projection of a persisted snapshot (`StayAliveQueueState` is not
/// `Clone`), holding the fields these tests assert on.
struct RecordedSnapshot {
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

struct TestLoop {
    event_loop: DaemonLoop,
    persisted: Persisted,
    settings: Arc<Mutex<OwnerSettings>>,
    merged_rx: mpsc::Receiver<DaemonEvent>,
}

fn test_loop_with_role(role: crate::DaemonRole) -> TestLoop {
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

fn test_loop_with_queue(role: crate::DaemonRole, items: Vec<QueueItem>, active: usize) -> TestLoop {
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
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
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
fn track_completed_stale_run_leaves_queue_and_persists_nothing() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![emby_qi("a", "Video", "Movie")],
        0,
    );
    t.event_loop
        .player
        .status
        .lock()
        .unwrap()
        .sequence_generation = 5;
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    let original_position = t.event_loop.owner.core.queue.slots()[0]
        .item
        .playback_position_ticks();

    let flow = t
        .event_loop
        .handle_event(DaemonEvent::Player(PlayerEvent::TrackCompleted {
            slot_id: slot,
            run_identity: 4,
            position_ticks: 900,
            played: true,
            consume: true,
        }));

    assert_eq!(flow, LoopFlow::Continue);
    assert_eq!(t.event_loop.owner.core.queue.slots().len(), 1);
    assert_eq!(
        t.event_loop.owner.core.queue.slots()[0]
            .item
            .playback_position_ticks(),
        original_position
    );
    assert!(t.persisted.borrow().is_empty());
}

#[test]
fn stopped_current_run_persists_once() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![emby_qi("a", "Video", "Movie")],
        0,
    );
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;

    let flow = t
        .event_loop
        .handle_event(DaemonEvent::Player(PlayerEvent::Stopped {
            slot_id: Some(slot),
            run_identity: current_run(&t.event_loop),
            position_ticks: 900,
            played: false,
            consume: false,
            error: None,
        }));

    assert_eq!(flow, LoopFlow::Continue);
    assert_eq!(
        t.event_loop
            .owner
            .core
            .queue
            .slot(slot)
            .unwrap()
            .item
            .playback_position_ticks(),
        900
    );
    assert_eq!(t.persisted.borrow().len(), 1);
}

#[test]
fn stopped_stale_run_persists_nothing() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![emby_qi("a", "Video", "Movie")],
        0,
    );
    t.event_loop
        .player
        .status
        .lock()
        .unwrap()
        .sequence_generation = 5;
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;

    let flow = t
        .event_loop
        .handle_event(DaemonEvent::Player(PlayerEvent::Stopped {
            slot_id: Some(slot),
            run_identity: 4,
            position_ticks: 900,
            played: true,
            consume: false,
            error: None,
        }));

    assert_eq!(flow, LoopFlow::Continue);
    assert!(
        !t.event_loop
            .owner
            .core
            .queue
            .slot(slot)
            .unwrap()
            .item
            .played()
    );
    assert!(t.persisted.borrow().is_empty());
}

#[test]
fn stopped_matching_pending_idle_load_commits_and_persists_once() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![emby_qi("old", "Video", "Movie")],
        0,
    );
    let old_slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    let run = current_run(&t.event_loop);
    let (reply_tx, reply_rx) = mpsc::channel();
    t.event_loop.owner.pending_idle_load = Some(PendingIdleQueueLoad {
        client_id: 0,
        request_id: 7,
        slots: vec![(old_slot, emby_qi("new", "Video", "Movie"))],
        cursor: 0,
        source: QueueSource::Album,
        reply_tx,
        stopped_run: run,
        started_at: Instant::now(),
    });

    let flow = t
        .event_loop
        .handle_event(DaemonEvent::Player(PlayerEvent::Stopped {
            slot_id: Some(old_slot),
            run_identity: run,
            position_ticks: 900,
            played: false,
            consume: false,
            error: None,
        }));

    assert_eq!(flow, LoopFlow::Continue);
    assert!(t.event_loop.owner.pending_idle_load.is_none());
    assert_eq!(t.event_loop.owner.core.queue.slots()[0].item.id(), "new");
    assert_eq!(t.event_loop.owner.core.source, QueueSource::Album);
    assert_eq!(t.persisted.borrow().len(), 1);
    assert!(matches!(
        recv_event(&reply_rx),
        CtrlEvent::UnifiedQueueLoadResult {
            request_id: 7,
            result: mbv_ctrl::QueueLoadResult::Accepted,
        }
    ));
}

#[test]
fn stopped_different_run_cancels_pending_idle_load_and_persists_nothing() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![emby_qi("old", "Video", "Movie")],
        0,
    );
    t.event_loop
        .player
        .status
        .lock()
        .unwrap()
        .sequence_generation = 5;
    let old_slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    let (reply_tx, reply_rx) = mpsc::channel();
    t.event_loop.owner.pending_idle_load = Some(PendingIdleQueueLoad {
        client_id: 0,
        request_id: 8,
        slots: vec![(old_slot, emby_qi("new", "Video", "Movie"))],
        cursor: 0,
        source: QueueSource::Album,
        reply_tx,
        stopped_run: 5,
        started_at: Instant::now(),
    });

    let flow = t
        .event_loop
        .handle_event(DaemonEvent::Player(PlayerEvent::Stopped {
            slot_id: Some(old_slot),
            run_identity: 4,
            position_ticks: 900,
            played: false,
            consume: false,
            error: None,
        }));

    assert_eq!(flow, LoopFlow::Continue);
    assert!(t.event_loop.owner.pending_idle_load.is_none());
    assert_eq!(t.event_loop.owner.core.queue.slots()[0].item.id(), "old");
    assert!(t.persisted.borrow().is_empty());
    assert!(matches!(
        recv_event(&reply_rx),
        CtrlEvent::UnifiedQueueLoadResult {
            request_id: 8,
            result: mbv_ctrl::QueueLoadResult::Rejected { .. },
        }
    ));
}

#[test]
fn ws_matching_generation_persists_once() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![emby_qi("a", "Video", "Movie")],
        0,
    );
    let runtime = EmbyOwnerContext::from_client(EmbyClient::new(Config::default()), 1);
    let generation = runtime.generation;
    t.event_loop.emby_runtime = Some(runtime);

    let flow = t.event_loop.handle_event(DaemonEvent::Ws {
        generation,
        event: WsEvent::Stop,
    });

    assert_eq!(flow, LoopFlow::Continue);
    assert_eq!(t.persisted.borrow().len(), 1);
}

#[test]
fn ws_mismatched_generation_persists_nothing() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![emby_qi("a", "Video", "Movie")],
        0,
    );
    t.event_loop.emby_runtime = Some(EmbyOwnerContext::from_client(
        EmbyClient::new(Config::default()),
        1,
    ));

    let flow = t.event_loop.handle_event(DaemonEvent::Ws {
        generation: mbv_core::service_runtime::SetupGeneration::new(99),
        event: WsEvent::Stop,
    });

    assert_eq!(flow, LoopFlow::Continue);
    assert!(t.persisted.borrow().is_empty());
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

#[test]
fn role_gate_non_local_dirty_event_persists_nothing() {
    let mut t = test_loop_with_role(crate::DaemonRole::Packaged);
    t.event_loop.client.lock().unwrap().config.consume_audio = true;

    let flow = t
        .event_loop
        .handle_event(DaemonEvent::Player(PlayerEvent::TrackCompleted {
            slot_id: mbv_queue::QueueSlotId::from_raw(1),
            run_identity: current_run(&t.event_loop),
            position_ticks: 0,
            played: true,
            consume: true,
        }));

    assert_eq!(flow, LoopFlow::Continue);
    assert!(t.persisted.borrow().is_empty());
}

#[test]
fn idle_queue_load_install_persists_once_through_injected_store() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![emby_qi("old", "Video", "Movie")],
        0,
    );
    // A real state dir would receive the install path's removed production
    // write; point it at a tempdir and assert that file never appears.
    let _guard = mbv_config::TestStateDirGuard::new();
    t.event_loop.client.lock().unwrap().token = "test-token".to_string();
    let (client_id, _rx) = connect_client(&mut t.event_loop.ctrl_clients.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();

    let flow = t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::UnifiedQueueLoadIdle {
            request_id: 5,
            slots: vec![mbv_ctrl::UnifiedQueueSlot {
                slot_id: 1,
                item: emby_qi("new", "Video", "Movie"),
            }],
            cursor: 0,
            source: QueueSource::Album,
        },
        client_id,
        reply_tx,
    ));

    assert_eq!(flow, LoopFlow::Continue);
    assert_eq!(t.event_loop.owner.core.queue.slots()[0].item.id(), "new");
    assert_eq!(t.event_loop.owner.core.source, QueueSource::Album);
    // The install path has no inline production write: the loop-pass store is
    // the single writer, so a committed load records exactly one snapshot.
    assert_eq!(t.persisted.borrow().len(), 1);
    assert_eq!(t.persisted.borrow()[0].item_ids, vec!["new".to_string()]);
    assert!(
        !mbv_config::stay_alive_queue_state_path().exists(),
        "install path must not write the production store directly"
    );
    assert!(matches!(
        recv_event(&reply_rx),
        CtrlEvent::UnifiedQueueLoadResult {
            request_id: 5,
            result: mbv_ctrl::QueueLoadResult::Accepted,
        }
    ));
}
