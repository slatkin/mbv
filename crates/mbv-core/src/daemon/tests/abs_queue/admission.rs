use super::*;

#[test]
fn abs_queue_projection_includes_abs_slots_for_capable_peer_only() {
    let abs = abs_qi("li_1", "ep_1");
    let emby = emby_qi("movie1", "Video", "Movie");
    let queue = PlaybackQueue::from_queue_items(vec![abs, emby], Some(0));
    let status = crate::player::PlayerStatus::default();
    let source = crate::config::QueueSource::Unknown;

    let CtrlEvent::UnifiedQueueState(capable_data) = super::unified_queue_state_for_peer(
        &status,
        &queue,
        &source,
        crate::ctrl::QueueLineage::default(),
        None,
        None,
        None,
        true,
        false,
    ) else {
        panic!("expected UnifiedQueueState");
    };
    let CtrlEvent::UnifiedQueueState(old_data) = super::unified_queue_state_for_peer(
        &status,
        &queue,
        &source,
        crate::ctrl::QueueLineage::default(),
        None,
        None,
        None,
        false,
        false,
    ) else {
        panic!("expected UnifiedQueueState");
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

    let CtrlEvent::UnifiedQueueState(old_data) = super::unified_queue_state_for_peer(
        &status,
        &queue,
        &source,
        crate::ctrl::QueueLineage::default(),
        None,
        None,
        None,
        false,
        false,
    ) else {
        panic!("expected UnifiedQueueState");
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
    let queue = PlaybackQueue::from_queue_items(
        vec![abs_qi("li_1", "ep_1"), emby_qi("movie1", "Video", "Movie")],
        Some(1),
    );
    let source = QueueSource::Unknown;

    // Trigger broadcast via UnifiedQueuePlaySlot on the Emby slot (index 1).
    let emby_slot_id = crate::ctrl::slot_id_to_u64(queue.slots()[1].slot_id);

    let mut owner = DaemonPlayerOwner {
        core: PlayerOwnerState::new(queue, source),
        ..Default::default()
    };
    handle_ctrl_for_role(
        CtrlCmd::UnifiedQueuePlaySlot {
            slot_id: emby_slot_id,
        },
        CtrlContext {
            reply_tx: &reply_tx,
            client_id: capable_id,
            client: &client,
            player: &player,
            audio_only: false,
            owner: &mut owner,
            shared_queue: &shared_queue_state(),
            ctrl_clients: &registry,
            has_audiobookshelf: false,
            merged_tx: &dummy_merged_tx,
            stay_alive: false,
            role: crate::daemon::DaemonRole::Local,
        },
    );
    let _queue = owner.core.queue;

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

    // The broadcast follows transition dispatch, so the requested slot is
    // published as in-flight (not left invisible until it settles).
    assert_eq!(
        capable_data.in_flight_transition.map(|t| t.target_slot),
        Some(emby_slot_id),
        "PlaySlot broadcast must carry the pending slot as in-flight"
    );
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
    let queue = PlaybackQueue::default();
    let source = QueueSource::Unknown;

    let mut owner = DaemonPlayerOwner {
        core: PlayerOwnerState::new(queue, source),
        ..Default::default()
    };
    handle_ctrl_for_role(
        CtrlCmd::UnifiedAdoptQueue {
            items: vec![abs_qi("li_1", "ep_1")],
            cursor: 0,
            source: QueueSource::Remote,
        },
        CtrlContext {
            reply_tx: &reply_tx,
            client_id: old_id,
            client: &client,
            player: &player,
            audio_only: false,
            owner: &mut owner,
            shared_queue: &shared_queue_state(),
            ctrl_clients: &registry,
            has_audiobookshelf: false,
            merged_tx: &dummy_merged_tx,
            stay_alive: false,
            role: crate::daemon::DaemonRole::Packaged,
        },
    );
    let queue = owner.core.queue;

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
    let queue = PlaybackQueue::default();
    let source = QueueSource::Unknown;

    let mut owner = DaemonPlayerOwner {
        core: PlayerOwnerState::new(queue, source),
        ..Default::default()
    };
    handle_ctrl_for_role(
        CtrlCmd::UnifiedAdoptQueue {
            items: vec![abs_qi("li_1", "ep_1"), emby_qi("movie1", "Video", "Movie")],
            cursor: 0,
            source: QueueSource::Remote,
        },
        CtrlContext {
            reply_tx: &reply_tx,
            client_id: capable_id,
            client: &client,
            player: &player,
            audio_only: false,
            owner: &mut owner,
            shared_queue: &shared_queue_state(),
            ctrl_clients: &registry,
            has_audiobookshelf: false,
            merged_tx: &dummy_merged_tx,
            stay_alive: false,
            role: crate::daemon::DaemonRole::Packaged,
        },
    );
    let queue = owner.core.queue;

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
    let queue = PlaybackQueue::default();
    let source = QueueSource::Unknown;

    let mut owner = DaemonPlayerOwner {
        core: PlayerOwnerState::new(queue, source),
        ..Default::default()
    };
    handle_ctrl_for_role(
        CtrlCmd::UnifiedAdoptQueue {
            items: vec![abs_qi("li_1", "ep_1")],
            cursor: 0,
            source: QueueSource::Remote,
        },
        CtrlContext {
            reply_tx: &reply_tx,
            client_id: capable_id,
            client: &client,
            player: &player,
            audio_only: false,
            owner: &mut owner,
            shared_queue: &shared_queue_state(),
            ctrl_clients: &registry,
            has_audiobookshelf: false,
            merged_tx: &dummy_merged_tx,
            stay_alive: false,
            role: crate::daemon::DaemonRole::Packaged,
        },
    );
    let _queue = owner.core.queue;

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
    let queue = PlaybackQueue::default();
    let source = QueueSource::Unknown;

    let mut owner = DaemonPlayerOwner {
        core: PlayerOwnerState::new(queue, source),
        ..Default::default()
    };
    handle_ctrl_for_role(
        CtrlCmd::UnifiedAdoptQueue {
            items: vec![abs_qi("li_1", "ep_1"), emby_qi("movie1", "Video", "Movie")],
            cursor: 0,
            source: QueueSource::Remote,
        },
        CtrlContext {
            reply_tx: &reply_tx,
            client_id: capable_id,
            client: &client,
            player: &player,
            audio_only: false,
            owner: &mut owner,
            shared_queue: &shared_queue_state(),
            ctrl_clients: &registry,
            has_audiobookshelf: true,
            merged_tx: &dummy_merged_tx,
            stay_alive: false,
            role: crate::daemon::DaemonRole::Packaged,
        },
    );
    let queue = owner.core.queue;

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
