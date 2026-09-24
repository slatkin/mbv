// Unit tests for the extracted `DaemonLoop` (change split-daemon-event-loop,
// §3). These build a loop with a recording store so the per-pass persistence
// can be asserted without touching real state files, and drive one event at a
// time through `handle_event` so the process never exits.

use super::{DaemonLoop, LoopFlow, PendingIdleQueueLoad};
use crate::config::StayAliveQueueState;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

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
            item_ids: state.queue.items.iter().map(|i| i.id().to_string()).collect(),
            source: state.queue.source.clone(),
            cursor: state.queue.cursor,
        }
    }
}

struct TestLoop {
    event_loop: DaemonLoop,
    persisted: Persisted,
    /// Isolates the idle-load install path's incidental direct write; kept
    /// alive for the test body. `None` when the thread already has a guard.
    _state_dir: Option<crate::config::TestStateDirGuard>,
}

fn test_loop_with_role(role: crate::daemon::DaemonRole) -> TestLoop {
    test_loop_with_queue(role, Vec::new(), 0)
}

fn test_loop_with_queue(
    role: crate::daemon::DaemonRole,
    items: Vec<QueueItem>,
    active: usize,
) -> TestLoop {
    let persisted: Persisted = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&persisted);
    let (merged_tx, _merged_rx) = mpsc::channel::<DaemonEvent>();
    let event_loop = DaemonLoop {
        owner: owner_with(items, active),
        player: cold_player(),
        shared_queue: shared_queue_state(),
        ctrl_clients: Arc::new(Mutex::new(CtrlClients::default())),
        client: Arc::new(Mutex::new(EmbyClient::new(Config::default()))),
        emby_runtime: None,
        audiobookshelf_runtime: None,
        merged_tx,
        ws_send_tx: None,
        direct_commands: Vec::new(),
        stay_alive: false,
        role,
        audio_only: false,
        last_keepalive: Instant::now(),
        last_capabilities: Instant::now(),
        store: Box::new(move |state| {
            recorded.borrow_mut().push(RecordedSnapshot::of(state));
            Ok(())
        }),
    };
    TestLoop {
        event_loop,
        persisted,
        _state_dir: crate::config::TestStateDirGuard::new_if_unset(),
    }
}

fn current_run(event_loop: &DaemonLoop) -> (crate::ctrl::PlaybackRequestId, crate::ctrl::PlaybackGeneration) {
    (0, event_loop.player.status.lock().unwrap().sequence_generation)
}

#[test]
fn track_completed_current_run_consumes_slot_and_persists_once() {
    let mut t = test_loop_with_queue(
        crate::daemon::DaemonRole::Local,
        vec![
            emby_qi("track", "Audio", "Audio"),
            emby_qi("next", "Audio", "Audio"),
        ],
        0,
    );
    t.event_loop.client.lock().unwrap().config.consume_audio = true;
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;

    let flow = t.event_loop.handle_event(DaemonEvent::Player(
        PlayerEvent::TrackCompleted {
            slot_id: slot,
            run_identity: current_run(&t.event_loop),
            position_ticks: 0,
            played: true,
            consume: true,
            progress_report_accepted: false,
        },
    ));

    assert_eq!(flow, LoopFlow::Continue);
    assert_eq!(t.event_loop.owner.core.queue.slots().len(), 1);
    assert_eq!(t.event_loop.owner.core.queue.slots()[0].item.id(), "next");
    assert_eq!(t.persisted.borrow().len(), 1);
    assert_eq!(t.persisted.borrow()[0].item_ids, vec!["next".to_string()]);
    assert_eq!(t.persisted.borrow()[0].cursor, 0);
}

#[test]
fn track_completed_stale_run_leaves_queue_and_persists_nothing() {
    let mut t = test_loop_with_queue(
        crate::daemon::DaemonRole::Local,
        vec![emby_qi("a", "Video", "Movie")],
        0,
    );
    t.event_loop.player.status.lock().unwrap().sequence_generation = 5;
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    let original_position = t.event_loop.owner.core.queue.slots()[0]
        .item
        .playback_position_ticks();

    let flow = t.event_loop.handle_event(DaemonEvent::Player(
        PlayerEvent::TrackCompleted {
            slot_id: slot,
            run_identity: (0, 4),
            position_ticks: 900,
            played: true,
            consume: true,
            progress_report_accepted: false,
        },
    ));

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
        crate::daemon::DaemonRole::Local,
        vec![emby_qi("a", "Video", "Movie")],
        0,
    );
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;

    let flow = t.event_loop.handle_event(DaemonEvent::Player(PlayerEvent::Stopped {
        slot_id: Some(slot),
        run_identity: current_run(&t.event_loop),
        position_ticks: 900,
        played: false,
        consume: false,
        progress_report_accepted: false,
        error: None,
    }));

    assert_eq!(flow, LoopFlow::Continue);
    assert_eq!(
        t.event_loop.owner.core.queue.slot(slot).unwrap()
            .item
            .playback_position_ticks(),
        900
    );
    assert_eq!(t.persisted.borrow().len(), 1);
}

#[test]
fn stopped_stale_run_persists_nothing() {
    let mut t = test_loop_with_queue(
        crate::daemon::DaemonRole::Local,
        vec![emby_qi("a", "Video", "Movie")],
        0,
    );
    t.event_loop.player.status.lock().unwrap().sequence_generation = 5;
    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;

    let flow = t.event_loop.handle_event(DaemonEvent::Player(PlayerEvent::Stopped {
        slot_id: Some(slot),
        run_identity: (0, 4),
        position_ticks: 900,
        played: true,
        consume: false,
        progress_report_accepted: false,
        error: None,
    }));

    assert_eq!(flow, LoopFlow::Continue);
    assert!(!t.event_loop.owner.core.queue.slot(slot).unwrap().item.played());
    assert!(t.persisted.borrow().is_empty());
}

#[test]
fn stopped_matching_pending_idle_load_commits_and_persists_once() {
    let mut t = test_loop_with_queue(
        crate::daemon::DaemonRole::Local,
        vec![emby_qi("old", "Video", "Movie")],
        0,
    );
    let old_slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    let run = current_run(&t.event_loop);
    let (reply_tx, reply_rx) = mpsc::channel();
    t.event_loop.owner.pending_idle_load = Some(PendingIdleQueueLoad {
        request_id: 7,
        slots: vec![(old_slot, emby_qi("new", "Video", "Movie"))],
        cursor: 0,
        source: QueueSource::Album,
        reply_tx,
        stopped_run: run,
        started_at: Instant::now(),
    });

    let flow = t.event_loop.handle_event(DaemonEvent::Player(PlayerEvent::Stopped {
        slot_id: Some(old_slot),
        run_identity: run,
        position_ticks: 900,
        played: false,
        consume: false,
        progress_report_accepted: false,
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
            result: crate::ctrl::QueueLoadResult::Accepted,
        }
    ));
}

#[test]
fn stopped_different_run_cancels_pending_idle_load_and_persists_nothing() {
    let mut t = test_loop_with_queue(
        crate::daemon::DaemonRole::Local,
        vec![emby_qi("old", "Video", "Movie")],
        0,
    );
    t.event_loop.player.status.lock().unwrap().sequence_generation = 5;
    let old_slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    let (reply_tx, reply_rx) = mpsc::channel();
    t.event_loop.owner.pending_idle_load = Some(PendingIdleQueueLoad {
        request_id: 8,
        slots: vec![(old_slot, emby_qi("new", "Video", "Movie"))],
        cursor: 0,
        source: QueueSource::Album,
        reply_tx,
        stopped_run: (0, 5),
        started_at: Instant::now(),
    });

    let flow = t.event_loop.handle_event(DaemonEvent::Player(PlayerEvent::Stopped {
        slot_id: Some(old_slot),
        run_identity: (0, 4),
        position_ticks: 900,
        played: false,
        consume: false,
        progress_report_accepted: false,
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
            result: crate::ctrl::QueueLoadResult::Rejected { .. },
        }
    ));
}

#[test]
fn ws_matching_generation_persists_once() {
    let mut t = test_loop_with_queue(
        crate::daemon::DaemonRole::Local,
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
        crate::daemon::DaemonRole::Local,
        vec![emby_qi("a", "Video", "Movie")],
        0,
    );
    t.event_loop.emby_runtime = Some(EmbyOwnerContext::from_client(
        EmbyClient::new(Config::default()),
        1,
    ));

    let flow = t.event_loop.handle_event(DaemonEvent::Ws {
        generation: crate::service_runtime::SetupGeneration::new(99),
        event: WsEvent::Stop,
    });

    assert_eq!(flow, LoopFlow::Continue);
    assert!(t.persisted.borrow().is_empty());
}

#[test]
fn playback_resolved_current_request_replaces_queue_and_persists_once() {
    let mut t = test_loop_with_queue(
        crate::daemon::DaemonRole::Local,
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
        crate::daemon::DaemonRole::Local,
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
fn role_gate_non_local_dirty_event_persists_nothing() {
    let mut t = test_loop_with_role(crate::daemon::DaemonRole::Packaged);
    t.event_loop.client.lock().unwrap().config.consume_audio = true;

    let flow = t.event_loop.handle_event(DaemonEvent::Player(
        PlayerEvent::TrackCompleted {
            slot_id: crate::playback_queue::QueueSlotId::from_raw(1),
            run_identity: current_run(&t.event_loop),
            position_ticks: 0,
            played: true,
            consume: true,
            progress_report_accepted: false,
        },
    ));

    assert_eq!(flow, LoopFlow::Continue);
    assert!(t.persisted.borrow().is_empty());
}
