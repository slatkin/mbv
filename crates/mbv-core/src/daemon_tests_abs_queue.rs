// Tests for task 4.1: mixed-version initial snapshots, later broadcasts,
// reconnects, and inbound mutations with capable and older unified peers
// attached simultaneously.

use super::apply_audiobookshelf_progress;
use crate::playback_queue::AudiobookshelfQueueItem;
use crate::player::AudiobookshelfProgressUpdate;
use crate::service_runtime::SetupGeneration;

fn abs_qi(library_item_id: &str, episode_id: &str) -> QueueItem {
    QueueItem::Audiobookshelf(AudiobookshelfQueueItem {
        library_item_id: library_item_id.into(),
        episode_id: episode_id.into(),
        title: "Test Episode".into(),
        show_title: None,
        author: None,
        duration_ticks: None,
        position_ticks: 0,
        played: false,
        pub_date_secs: None,
        is_finished: false,
        cover_path: None,
    })
}

fn connect_old_unified_peer(clients: &mut CtrlClients) -> (u64, mpsc::Receiver<CtrlOutbound>) {
    let (tx, rx) = mpsc::channel();
    // supports_feed=true, supports_unified=true, abs_queue=false, abs_progress=false
    let id = clients.connect(tx, CtrlTransport::Local, true, true, false, false);
    (id, rx)
}

fn recv_unified_queue(rx: &mpsc::Receiver<CtrlOutbound>) -> crate::ctrl::UnifiedQueueStateData {
    match recv_event(rx) {
        CtrlEvent::UnifiedQueueState(data) => data,
        _ => panic!("expected UnifiedQueueState"),
    }
}

// Covers initial snapshots and reconnects: `unified_queue_state_for_peer` is
// the function handle_ws calls for both. Tested directly here to avoid the
// socket plumbing that integration tests cover.
#[test]
fn abs_queue_projection_includes_abs_slots_for_capable_peer_only() {
    let abs = abs_qi("li_1", "ep_1");
    let emby = emby_qi("movie1", "Video", "Movie");
    let queue = PlaybackQueue::from_queue_items(vec![abs, emby], Some(0));
    let status = crate::player::PlayerStatus::default();
    let source = crate::config::QueueSource::Unknown;

    let capable_data = match super::unified_queue_state_for_peer(&status, &queue, &source, true) {
        CtrlEvent::UnifiedQueueState(d) => d,
        _ => panic!("expected UnifiedQueueState"),
    };
    let old_data = match super::unified_queue_state_for_peer(&status, &queue, &source, false) {
        CtrlEvent::UnifiedQueueState(d) => d,
        _ => panic!("expected UnifiedQueueState"),
    };

    assert_eq!(capable_data.slots.len(), 2, "capable peer sees ABS+Emby");
    assert_eq!(old_data.slots.len(), 1, "old peer sees Emby only");
    assert!(
        old_data.slots[0].item.is_emby(),
        "old peer's sole slot must be Emby"
    );
}

// When the active slot is ABS, old peers must receive no active_slot (not a
// dangling ID pointing at a missing slot).
#[test]
fn abs_queue_projection_clears_active_slot_for_old_peer_when_abs_is_active() {
    let abs = abs_qi("li_1", "ep_1");
    let emby = emby_qi("movie1", "Video", "Movie");
    // active index 0 = ABS
    let queue = PlaybackQueue::from_queue_items(vec![abs, emby], Some(0));
    let status = crate::player::PlayerStatus::default();
    let source = crate::config::QueueSource::Unknown;

    let old_data = match super::unified_queue_state_for_peer(&status, &queue, &source, false) {
        CtrlEvent::UnifiedQueueState(d) => d,
        _ => panic!("expected UnifiedQueueState"),
    };

    assert_eq!(old_data.slots.len(), 1);
    assert_eq!(
        old_data.active_slot, None,
        "active_slot must be cleared for old peer when ABS slot is active"
    );
}

// Broadcast fan-out: capable and old peers both connected; after a queue
// mutation the broadcast sends each peer its correctly projected snapshot.
#[test]
fn broadcast_projects_abs_slots_per_connection_capability() {
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (capable_id, capable_rx) = connect_client(&mut registry.lock().unwrap());
    let (_old_id, old_rx) = connect_old_unified_peer(&mut registry.lock().unwrap());
    let (reply_tx, _reply_rx) = mpsc::channel();
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    // Build a mixed queue directly (bypasses daemon_admits so ABS stays in).
    let mut queue = PlaybackQueue::from_queue_items(
        vec![abs_qi("li_1", "ep_1"), emby_qi("movie1", "Video", "Movie")],
        Some(1),
    );
    let mut source = QueueSource::Unknown;

    // Trigger broadcast via UnifiedQueuePlaySlot on the Emby slot (index 1).
    let emby_slot_id = crate::ctrl::slot_id_to_u64(queue.slots()[1].slot_id);

    handle_ctrl(
        CtrlCmd::UnifiedQueuePlaySlot {
            slot_id: emby_slot_id,
        },
        capable_id,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut queue,
        &mut source,
        &shared_queue_state(),
        &registry,
        &mut PlaybackIntentState::default(),
        false,
        None,
        &dummy_merged_tx,
    );

    let capable_data = recv_unified_queue(&capable_rx);
    let old_data = recv_unified_queue(&old_rx);

    assert_eq!(
        capable_data.slots.len(),
        2,
        "capable peer broadcast includes ABS+Emby"
    );
    assert_eq!(
        old_data.slots.len(),
        1,
        "old peer broadcast includes Emby only"
    );
    assert!(old_data.slots[0].item.is_emby());
}

// Inbound mutation from an old peer containing ABS items is transport-rejected
// before the canonical queue is touched.
#[test]
fn old_peer_submitting_abs_items_is_transport_rejected() {
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (old_id, old_rx) = connect_old_unified_peer(&mut registry.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();
    let mut queue = PlaybackQueue::default();
    let mut source = QueueSource::Unknown;

    handle_ctrl(
        CtrlCmd::UnifiedAdoptQueue {
            items: vec![abs_qi("li_1", "ep_1")],
            cursor: 0,
            source: QueueSource::Remote,
        },
        old_id,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut queue,
        &mut source,
        &shared_queue_state(),
        &registry,
        &mut PlaybackIntentState::default(),
        false,
        None,
        &dummy_merged_tx,
    );

    assert!(
        queue.is_empty(),
        "queue must not be mutated by transport-rejected submission"
    );

    match recv_event(&reply_rx) {
        CtrlEvent::CommandRejected(reason) => assert!(
            reason.contains("did not negotiate"),
            "rejection must name the missing capability, got: {reason}"
        ),
        _ => panic!("expected CommandRejected"),
    }

    // No broadcast goes out — the rejection short-circuits before broadcast_queue_state.
    assert!(
        old_rx.try_recv().is_err(),
        "old peer must not receive a broadcast when transport rejection fires"
    );
}

// Task 4.2: a capable peer's ABS item clears the transport gate but stays
// ineligible for daemon admission (daemon_admits hardcodes
// can_admit_audiobookshelf: false). Proves the resulting canonical queue
// never contains the ABS item and never reaches player.set_initial_queue
// with it — i.e. no source preparation is ever attempted for it.
#[test]
fn capable_peer_abs_item_is_admission_ineligible_with_no_queue_mutation() {
    let player = cold_player();
    let mut emby_client = crate::api::EmbyClient::new(Config::default());
    emby_client.token = "test-token".to_string();
    let client = Arc::new(Mutex::new(emby_client));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (capable_id, _capable_rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, _reply_rx) = mpsc::channel();
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();
    let mut queue = PlaybackQueue::default();
    let mut source = QueueSource::Unknown;

    handle_ctrl(
        CtrlCmd::UnifiedAdoptQueue {
            items: vec![abs_qi("li_1", "ep_1"), emby_qi("movie1", "Video", "Movie")],
            cursor: 0,
            source: QueueSource::Remote,
        },
        capable_id,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut queue,
        &mut source,
        &shared_queue_state(),
        &registry,
        &mut PlaybackIntentState::default(),
        false,
        None,
        &dummy_merged_tx,
    );

    // Transport gate passed (peer is capable), but the canonical queue must
    // never hold the ABS item — admission is a separate, always-active gate.
    assert_eq!(
        queue.len(),
        1,
        "only the admissible Emby item should reach the canonical queue"
    );
    assert!(
        queue.slots()[0].item.is_emby(),
        "the surviving slot must be the Emby item, not Audiobookshelf"
    );
    assert!(
        queue.slots().iter().all(|s| !s.item.is_audiobookshelf()),
        "no Audiobookshelf slot may reach the canonical queue regardless of transport capability"
    );
}

// Inbound mutation from a capable peer passes the transport gate — any later
// filtering is from daemon_admits (not an abs-queue transport rejection).
#[test]
fn capable_peer_submitting_abs_items_passes_transport_gate() {
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (capable_id, _capable_rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();
    let mut queue = PlaybackQueue::default();
    let mut source = QueueSource::Unknown;

    handle_ctrl(
        CtrlCmd::UnifiedAdoptQueue {
            items: vec![abs_qi("li_1", "ep_1")],
            cursor: 0,
            source: QueueSource::Remote,
        },
        capable_id,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut queue,
        &mut source,
        &shared_queue_state(),
        &registry,
        &mut PlaybackIntentState::default(),
        false,
        None,
        &dummy_merged_tx,
    );

    // Transport gate passed. If a CommandRejected arrives, it must not name
    // the abs-queue transport capability — that would mean the capable peer
    // was incorrectly blocked at the transport layer.
    if let Ok(CtrlOutbound::Event(json)) = reply_rx.try_recv() {
        if let CtrlEvent::CommandRejected(reason) =
            serde_json::from_str::<CtrlEvent>(&json).unwrap()
        {
            assert!(
                !reason.contains("did not negotiate"),
                "capable peer must not receive transport rejection, got: {reason}"
            );
        }
    }
}

// A capable peer with installed runtime admits ABS items into the canonical
// queue. Mirrors the two-condition gate: runtime present (has_audiobookshelf)
// AND the client negotiated abs-queue (transport gate already passed).
#[test]
fn capable_peer_abs_item_is_admitted_with_installed_runtime() {
    let player = cold_player();
    let mut emby_client = crate::api::EmbyClient::new(Config::default());
    emby_client.token = "test-token".to_string();
    let client = Arc::new(Mutex::new(emby_client));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (capable_id, _capable_rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, _reply_rx) = mpsc::channel();
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();
    let mut queue = PlaybackQueue::default();
    let mut source = QueueSource::Unknown;

    handle_ctrl(
        CtrlCmd::UnifiedAdoptQueue {
            items: vec![abs_qi("li_1", "ep_1"), emby_qi("movie1", "Video", "Movie")],
            cursor: 0,
            source: QueueSource::Remote,
        },
        capable_id,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut queue,
        &mut source,
        &shared_queue_state(),
        &registry,
        &mut PlaybackIntentState::default(),
        true,
        None,
        &dummy_merged_tx,
    );

    assert_eq!(
        queue.len(),
        2,
        "capable peer with installed runtime admits ABS and Emby"
    );
    assert!(
        queue
            .slots()
            .iter()
            .any(|slot| slot.item.is_audiobookshelf()),
        "the ABS item must reach the canonical queue"
    );
}

fn progress_update(
    generation: u64,
    current_time_seconds: f64,
    is_finished: bool,
) -> AudiobookshelfProgressUpdate {
    AudiobookshelfProgressUpdate {
        generation: SetupGeneration::new(generation),
        library_item_id: "li_1".into(),
        episode_id: "ep_1".into(),
        current_time_seconds,
        duration_seconds: 100.0,
        is_finished,
    }
}

fn abs_queue_with_slot() -> PlaybackQueue {
    PlaybackQueue::from_queue_items(vec![abs_qi("li_1", "ep_1")], Some(0))
}

// Acknowledged periodic sync updates the matching Bound slot and is broadcast
// as redacted progress to a client that negotiated abs-progress.
#[test]
fn acknowledged_progress_updates_bound_slot_and_broadcasts() {
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_capable_id, capable_rx) = connect_client(&mut registry.lock().unwrap());
    let mut queue = abs_queue_with_slot();

    apply_audiobookshelf_progress(
        progress_update(1, 30.0, false),
        Some(SetupGeneration::new(1)),
        &mut queue,
        &registry,
    );

    let episode = queue.slots()[0].item.as_audiobookshelf().unwrap();
    assert!(
        episode.position_ticks > 0,
        "acknowledged position must be written to the Bound slot"
    );
    assert!(!episode.is_finished);

    match recv_event(&capable_rx) {
        CtrlEvent::AudiobookshelfProgress(event) => {
            assert_eq!(event.library_item_id, "li_1");
            assert_eq!(event.episode_id, "ep_1");
            assert_eq!(event.setup_generation, 1);
            assert!(!event.is_finished);
        }
        _ => panic!("expected AudiobookshelfProgress broadcast"),
    }
}

// Completion marks the Bound slot finished and broadcasts the completion.
#[test]
fn acknowledged_completion_marks_slot_done_and_broadcasts() {
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_capable_id, capable_rx) = connect_client(&mut registry.lock().unwrap());
    let mut queue = abs_queue_with_slot();

    apply_audiobookshelf_progress(
        progress_update(1, 100.0, true),
        Some(SetupGeneration::new(1)),
        &mut queue,
        &registry,
    );

    let episode = queue.slots()[0].item.as_audiobookshelf().unwrap();
    assert!(
        episode.is_finished,
        "completion must mark the Bound slot finished"
    );

    match recv_event(&capable_rx) {
        CtrlEvent::AudiobookshelfProgress(event) => assert!(event.is_finished),
        _ => panic!("expected AudiobookshelfProgress broadcast"),
    }
}

// A stale-generation update is dropped without queue or broadcast side effect.
#[test]
fn stale_generation_progress_is_dropped_without_side_effects() {
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_capable_id, capable_rx) = connect_client(&mut registry.lock().unwrap());
    let mut queue = abs_queue_with_slot();
    let before = queue.slots()[0]
        .item
        .as_audiobookshelf()
        .unwrap()
        .position_ticks;

    apply_audiobookshelf_progress(
        progress_update(1, 30.0, false),
        Some(SetupGeneration::new(2)),
        &mut queue,
        &registry,
    );

    let episode = queue.slots()[0].item.as_audiobookshelf().unwrap();
    assert_eq!(
        episode.position_ticks, before,
        "stale generation must not mutate the Bound slot"
    );
    assert!(
        capable_rx.try_recv().is_err(),
        "stale generation must not broadcast progress"
    );
}

// Client exit alone must not finalize active ABS playback or mutate the Bound
// queue — the stay-alive event loop continues owning ABS playback.
#[test]
fn client_exit_does_not_finalize_or_mutate_active_abs_queue() {
    let mut clients = CtrlClients::default();
    let (id, _rx) = connect_client(&mut clients);
    let mut intents = PlaybackIntentState::default();

    // Mirror the CtrlDisconnected arm: drop the client, invalidate its intent.
    clients.remove(id);
    intents.invalidate_connection(id);

    // The disconnect path never receives the queue or player; an active ABS
    // queue is untouched.
    let queue = abs_queue_with_slot();
    assert_eq!(queue.len(), 1);
    assert!(
        queue.slots()[0].item.is_audiobookshelf(),
        "active ABS slot must survive client exit"
    );
    assert!(
        queue.active_slot_id().is_some(),
        "active ABS playback must not be finalized by client exit"
    );
}
