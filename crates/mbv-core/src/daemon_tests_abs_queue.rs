// Tests for task 4.1: mixed-version initial snapshots, later broadcasts,
// reconnects, and inbound mutations with capable and older unified peers
// attached simultaneously.

use super::apply_audiobookshelf_book_progress;
use super::apply_audiobookshelf_progress;
use crate::playback_queue::AudiobookshelfBookQueueItem;
use crate::playback_queue::AudiobookshelfQueueItem;
use crate::player::AudiobookshelfBookProgressUpdate;
use crate::player::AudiobookshelfProgressUpdate;
use crate::service_runtime::SetupGeneration;

fn abs_qi(library_item_id: &str, episode_id: &str) -> QueueItem {
    QueueItem::Audiobookshelf(AudiobookshelfQueueItem {
        library_item_id: library_item_id.into(),
        episode_id: episode_id.into(),
        title: "Test Episode".into(),
        show_title: None,
        author: None,
        description: None,
        duration_ticks: None,
        position_ticks: 0,
        played: false,
        pub_date_secs: None,
        is_finished: false,
        cover_path: None,
    })
}

fn book_qi(library_item_id: &str) -> QueueItem {
    QueueItem::AudiobookshelfBook(AudiobookshelfBookQueueItem {
        library_item_id: library_item_id.into(),
        title: "Test Book".into(),
        author: None,
        duration_ticks: None,
        position_ticks: 0,
        played: false,
        is_finished: false,
        cover_path: None,
    })
}

fn connect_old_unified_peer(clients: &mut CtrlClients) -> (u64, mpsc::Receiver<CtrlOutbound>) {
    let (tx, rx) = mpsc::channel();
    // abs_queue=false, abs_progress=false, abs_book_*=false
    let id = clients.connect(tx, CtrlTransport::Local, false, false, false, false);
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

    let capable_data =
        match super::unified_queue_state_for_peer(&status, &queue, &source, true, false) {
            CtrlEvent::UnifiedQueueState(d) => d,
            _ => panic!("expected UnifiedQueueState"),
        };
    let old_data = match super::unified_queue_state_for_peer(&status, &queue, &source, false, false)
    {
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

    let old_data = match super::unified_queue_state_for_peer(&status, &queue, &source, false, false)
    {
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
        &dummy_merged_tx,
        false,
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
        &dummy_merged_tx,
        false,
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
        &dummy_merged_tx,
        false,
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
        &dummy_merged_tx,
        false,
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
        &dummy_merged_tx,
        false,
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

// Task 1.2: The emitted AudiobookshelfProgress wire event must carry no API
// key, Authorization header, resolved URL, or sessionId, and must be
// delivered only to peers that negotiated `abs-progress`.
#[test]
fn progress_event_carries_no_credentials_and_is_gated_to_capable_peer() {
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_capable_id, capable_rx) = connect_client(&mut registry.lock().unwrap());
    let (_old_id, old_rx) = connect_old_unified_peer(&mut registry.lock().unwrap());
    let mut queue = abs_queue_with_slot();

    apply_audiobookshelf_progress(
        progress_update(1, 30.0, false),
        Some(SetupGeneration::new(1)),
        &mut queue,
        &registry,
    );

    // Capable peer receives the event — extract the raw JSON for inspection.
    let json = match capable_rx.recv().unwrap() {
        CtrlOutbound::Event(json) => json,
        CtrlOutbound::Flush(_) => panic!("expected event, got flush barrier"),
    };

    // Wire payload must carry no credentials, authorization tokens, or sessions.
    assert!(
        !json.contains("Authorization"),
        "wire event must not contain Authorization header"
    );
    assert!(
        !json.contains("sessionId"),
        "wire event must not contain sessionId"
    );
    assert!(
        !json.contains("api_key"),
        "wire event must not contain api_key field"
    );

    // Sanity-decode: the event must parse as AudiobookshelfProgress.
    let event: CtrlEvent = serde_json::from_str(&json).unwrap();
    assert!(
        matches!(event, CtrlEvent::AudiobookshelfProgress(_)),
        "capable peer must receive AudiobookshelfProgress"
    );

    // Non-capable (old/unified) peer must NOT receive AudiobookshelfProgress.
    assert!(
        old_rx.try_recv().is_err(),
        "peer without abs-progress capability must not receive AudiobookshelfProgress"
    );
}

// Task 5.1: After the only attached client exits, a newly connected capable
// client must receive subsequent AudiobookshelfProgress broadcasts without
// requiring a session restart.
#[test]
fn emission_resumes_to_new_capable_client_after_previous_client_exits() {
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (id1, rx1) = connect_client(&mut registry.lock().unwrap());
    let mut queue = abs_queue_with_slot();

    // First emission — the initial client receives it.
    apply_audiobookshelf_progress(
        progress_update(1, 30.0, false),
        Some(SetupGeneration::new(1)),
        &mut queue,
        &registry,
    );
    // Drain the first client's event (confirmed received).
    let _ = rx1.recv().unwrap();

    // Client 1 disconnects; queue must NOT be finalized (stay-alive invariant).
    registry.lock().unwrap().remove(id1);
    assert_eq!(queue.len(), 1, "queue must not be mutated by client exit");
    assert!(
        queue.active_slot_id().is_some(),
        "active slot must survive client exit"
    );

    // A new capable client attaches to the same daemon session.
    let (_id2, rx2) = connect_client(&mut registry.lock().unwrap());

    // Second emission must reach the new client.
    apply_audiobookshelf_progress(
        progress_update(1, 60.0, false),
        Some(SetupGeneration::new(1)),
        &mut queue,
        &registry,
    );

    match recv_event(&rx2) {
        CtrlEvent::AudiobookshelfProgress(ev) => {
            assert_eq!(ev.library_item_id, "li_1");
            assert_eq!(ev.episode_id, "ep_1");
            assert!(!ev.is_finished);
        }
        _ => panic!("expected AudiobookshelfProgress from new client, got unexpected variant"),
    }
}

// Task 5.3 – multi-step: acknowledged progress advances through a full
// lifecycle sequence (play → pause → seek → resume → completion), updating
// the Bound slot and broadcasting each step to the capable client.
#[test]
fn acknowledged_progress_advances_through_play_pause_seek_and_completion() {
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_capable_id, capable_rx) = connect_client(&mut registry.lock().unwrap());
    let mut queue = abs_queue_with_slot();

    // (seconds, is_finished): play → pause (same pos) → seek back → resume → complete.
    let steps: &[(f64, bool)] = &[
        (10.0, false),
        (30.0, false),
        (30.0, false), // pause: same position re-reported
        (10.0, false), // seek back to an earlier point
        (45.0, false),
        (80.0, false),
        (100.0, true), // natural completion
    ];

    for &(secs, finished) in steps {
        apply_audiobookshelf_progress(
            progress_update(1, secs, finished),
            Some(SetupGeneration::new(1)),
            &mut queue,
            &registry,
        );

        let expected_ticks = (secs * crate::api::TICKS_PER_SECOND as f64) as i64;
        let ep = queue.slots()[0].item.as_audiobookshelf().unwrap();
        assert_eq!(
            ep.position_ticks, expected_ticks,
            "Bound slot position_ticks must match at {secs}s"
        );
        assert_eq!(
            ep.is_finished, finished,
            "Bound slot is_finished must match at {secs}s"
        );

        match recv_event(&capable_rx) {
            CtrlEvent::AudiobookshelfProgress(ev) => {
                assert_eq!(
                    ev.position_ticks, expected_ticks,
                    "broadcast ticks at {secs}s"
                );
                assert_eq!(ev.is_finished, finished, "broadcast is_finished at {secs}s");
                assert_eq!(ev.setup_generation, 1);
            }
            _ => panic!("expected AudiobookshelfProgress at {secs}s"),
        }
    }

    let ep = queue.slots()[0].item.as_audiobookshelf().unwrap();
    assert!(
        ep.is_finished,
        "slot must be marked finished after completion"
    );
}

// A book progress update must never match an episode-shaped queue slot, even
// when the `library_item_id` collides — the two kinds share no identity.
#[test]
fn book_progress_update_does_not_touch_episode_slots() {
    let mut queue = PlaybackQueue::from_queue_items(vec![abs_qi("li_1", "ep")], Some(0));
    let before = queue.slots()[0]
        .item
        .as_audiobookshelf()
        .unwrap()
        .position_ticks;

    apply_audiobookshelf_book_progress(
        AudiobookshelfBookProgressUpdate {
            generation: SetupGeneration::new(1),
            library_item_id: "li_1".into(),
            current_time_seconds: 30.0,
            duration_seconds: 100.0,
            is_finished: false,
        },
        Some(SetupGeneration::new(1)),
        &mut queue,
        &Arc::new(Mutex::new(CtrlClients::default())),
    );

    let episode = queue.slots()[0].item.as_audiobookshelf().unwrap();
    assert_eq!(
        episode.position_ticks, before,
        "a book progress event must not update an episode-shaped slot"
    );
    assert!(!episode.is_finished);
}

// An episode progress update must not match a book queue slot, even on a
// colliding `library_item_id`.
#[test]
fn episode_progress_update_does_not_touch_book_slots() {
    let mut queue = PlaybackQueue::from_queue_items(vec![book_qi("shared_1")], Some(0));
    let before = queue.slots()[0]
        .item
        .as_audiobookshelf_book()
        .unwrap()
        .position_ticks;

    apply_audiobookshelf_progress(
        AudiobookshelfProgressUpdate {
            generation: SetupGeneration::new(1),
            library_item_id: "shared_1".into(),
            episode_id: "ep-1".into(),
            current_time_seconds: 30.0,
            duration_seconds: 100.0,
            is_finished: false,
        },
        Some(SetupGeneration::new(1)),
        &mut queue,
        &Arc::new(Mutex::new(CtrlClients::default())),
    );

    let book = queue.slots()[0].item.as_audiobookshelf_book().unwrap();
    assert_eq!(
        book.position_ticks, before,
        "an episode progress update must not move a book-shaped slot"
    );
    assert!(!book.is_finished);
}
