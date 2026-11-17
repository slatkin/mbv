//! The swap of design D2 of `gui-named-queues`: a play or replace that names a
//! resting queue parks the queue that was current, installs the resting queue
//! as the current queue, and starts it.

use super::*;

/// Adds one resting queue named `name` holding the item ids `items`, and returns
/// its identity with its slot ids in queue order.
fn resting_queue_with(
    owner: &mut DaemonPlayerOwner,
    name: &str,
    items: &[&str],
) -> (mbv_queue::NamedQueueId, Vec<mbv_queue::QueueSlotId>) {
    let id = add_resting_queue(owner, name);
    let queue = owner
        .named_queues
        .resting
        .last_mut()
        .expect("the queue the store just added rests");
    let slot_ids = items
        .iter()
        .map(|name| queue.queue.append(emby_qi(name, "Video", "Movie")))
        .collect();
    (id, slot_ids)
}

/// The item ids of every slot of the Owner's current queue.
fn current_item_ids(owner: &DaemonPlayerOwner) -> Vec<String> {
    owner
        .core
        .queue
        .slots()
        .iter()
        .map(|slot| slot.item.id().to_string())
        .collect()
}

/// The next whole-queue snapshot a connected Client receives, skipping the
/// Named-queue list a swap also broadcasts.
fn next_queue_snapshot(rx: &mpsc::Receiver<CtrlOutbound>) -> mbv_ctrl::UnifiedQueueStateData {
    loop {
        match recv_event(rx) {
            CtrlEvent::UnifiedQueueState(data) => return data,
            CtrlEvent::NamedQueueList { .. } => {}
            other => panic!("expected a queue snapshot, got {other:?}"),
        }
    }
}

/// The state every swap test starts from: a Local Owner, one connected command
/// Client, and a store it owns.
struct SwapFixture {
    player: Player,
    client: Arc<Mutex<EmbyClient>>,
    registry: Arc<Mutex<CtrlClients>>,
    client_id: u64,
    reply_tx: mpsc::Sender<CtrlOutbound>,
    reply_rx: mpsc::Receiver<CtrlOutbound>,
    shared_queue: SharedQueueState,
}

fn swap_fixture() -> SwapFixture {
    let player = cold_player();
    let client = queue_client();
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    SwapFixture {
        player,
        client,
        registry,
        client_id,
        reply_tx,
        reply_rx,
        shared_queue,
    }
}

/// Contract: spec `named-queues` "Play starts a queue at its start slot" — a
/// Play of a resting queue makes it current and starts its own start slot, and
/// the queue that was current is parked with its slots and start slot kept
/// (design D2).
#[test]
fn play_a_resting_queue_starts_its_start_slot() {
    let fixture = swap_fixture();
    let mut owner = owner_with(vec![emby_qi("default", "Video", "Movie")], 0);
    owner.named_queues.current_name = "Default".to_string();
    let default_id = owner.named_queues.current;
    let default_slot = owner.core.queue.slots()[0].slot_id;
    fixture.shared_queue.note_start_slot(
        &owner.core.queue,
        mbv_queue::StartSlotObservation::Active(default_slot),
    );
    let (kids_id, kids_slots) = resting_queue_with(&mut owner, "Kids", &["k1", "k2"]);
    if let Some(kids) = owner.named_queues.resting.last_mut() {
        kids.note_start_slot(mbv_queue::StartSlotObservation::Active(kids_slots[1]));
    }

    let merged_rx = run_named_command(
        CtrlCmd::NamedQueuePlay {
            queue: kids_id,
            slot: None,
            op: mbv_ctrl::QueueOpId(76),
        },
        fixture.client_id,
        &fixture.reply_tx,
        &fixture.client,
        &fixture.player,
        &mut owner,
        &fixture.shared_queue,
        &fixture.registry,
    );

    assert_eq!(owner.named_queues.current, kids_id);
    assert_eq!(owner.named_queues.current_name, "Kids");
    assert_eq!(
        owner
            .core
            .queue
            .slots()
            .iter()
            .map(|slot| slot.slot_id)
            .collect::<Vec<_>>(),
        kids_slots,
        "the installed queue keeps its slot identities"
    );
    assert_eq!(owner.core.queue.active_slot_id(), Some(kids_slots[1]));
    assert_eq!(fixture.shared_queue.start_slot(), Some(kids_slots[1]));
    let parked = owner
        .named_queues
        .resting
        .first()
        .expect("the queue that was current is parked");
    assert_eq!(parked.id, default_id);
    assert_eq!(parked.start_slot, Some(default_slot));
    assert_eq!(parked.queue.slots()[0].slot_id, default_slot);
    let status = fixture.player.status.lock().unwrap();
    assert_eq!(
        status.current_idx, 1,
        "Kids' start slot is the one that plays"
    );
    assert_eq!(status.queue_len, 2);
    assert!(status.active);
    drop(status);
    match recv_event(&fixture.reply_rx) {
        CtrlEvent::QueueOpResult {
            op: mbv_ctrl::QueueOpId(76),
            outcome: mbv_ctrl::QueueOpOutcome::Applied(data),
        } => {
            assert_eq!(data.queue, None, "the answer describes the current queue");
            assert_eq!(
                data.slots
                    .iter()
                    .map(|slot| slot.item.id().to_string())
                    .collect::<Vec<_>>(),
                vec!["k1", "k2"]
            );
        }
        _ => panic!("expected the play to be answered"),
    }
    assert_eq!(
        persisted_named_queues(&merged_rx, &owner, &fixture.shared_queue).current,
        kids_id
    );
}

/// Contract: spec `named-queues` "Play starts a queue at its start slot" — a
/// Play of a row of a resting queue while another queue plays stops the playing
/// queue with its playing slot as its start slot, and starts that row (design
/// D2).
#[test]
fn play_a_row_of_a_resting_queue_while_another_queue_plays() {
    let fixture = swap_fixture();
    fixture.player.status.lock().unwrap().active = true;
    let mut owner = owner_with(
        vec![
            emby_qi("a", "Video", "Movie"),
            emby_qi("b", "Video", "Movie"),
            emby_qi("c", "Video", "Movie"),
            emby_qi("d", "Video", "Movie"),
        ],
        3,
    );
    let default_id = owner.named_queues.current;
    let fourth = owner.core.queue.slots()[3].slot_id;
    owner.core.note_observed_active_slot(Some(fourth));
    fixture.shared_queue.publish_observed(&owner.core);
    fixture.shared_queue.note_start_slot(
        &owner.core.queue,
        mbv_queue::StartSlotObservation::Active(fourth),
    );
    let (kids_id, kids_slots) = resting_queue_with(&mut owner, "Kids", &["k1", "k2"]);

    let merged_rx = run_named_command(
        CtrlCmd::UnifiedQueuePlaySlot {
            slot_id: kids_slots[0].raw(),
            queue: Some(kids_id),
            op: Some(mbv_ctrl::QueueOpId(77)),
        },
        fixture.client_id,
        &fixture.reply_tx,
        &fixture.client,
        &fixture.player,
        &mut owner,
        &fixture.shared_queue,
        &fixture.registry,
    );

    assert_eq!(owner.named_queues.current, kids_id);
    assert_eq!(owner.core.queue.active_slot_id(), Some(kids_slots[0]));
    assert_eq!(fixture.shared_queue.start_slot(), Some(kids_slots[0]));
    let parked = owner
        .named_queues
        .resting
        .first()
        .expect("the queue that was playing is parked");
    assert_eq!(parked.id, default_id);
    assert_eq!(
        parked.start_slot,
        Some(fourth),
        "the stop keeps the slot that was playing"
    );
    assert_eq!(parked.queue.len(), 4, "the parked queue keeps every slot");
    let status = fixture.player.status.lock().unwrap();
    assert_eq!(status.current_idx, 0, "the named row is the one that plays");
    assert_eq!(status.queue_len, 2);
    drop(status);
    assert_eq!(
        persisted_named_queues(&merged_rx, &owner, &fixture.shared_queue).current,
        kids_id
    );
}

/// Contract: design D2 — a replace that names a resting queue swaps that queue
/// in and installs the command's own items in place of its slots, which is what
/// a Library Play onto a resting queue sends.
#[test]
fn replace_of_a_resting_queue_installs_the_new_items() {
    let fixture = swap_fixture();
    let mut owner = owner_holding(&["Default"]);
    let default_id = owner.named_queues.current;
    let (kids_id, _kids_slots) = resting_queue_with(&mut owner, "Kids", &["k1", "k2"]);

    let merged_rx = run_named_command(
        CtrlCmd::UnifiedQueueReplace {
            items: vec![emby_qi("library", "Video", "Movie")],
            slots: Vec::new(),
            start_idx: Some(0),
            source: QueueSource::Unknown,
            start_position_secs: None,
            op: Some(mbv_ctrl::QueueOpId(78)),
            queue: Some(kids_id),
        },
        fixture.client_id,
        &fixture.reply_tx,
        &fixture.client,
        &fixture.player,
        &mut owner,
        &fixture.shared_queue,
        &fixture.registry,
    );

    assert_eq!(owner.named_queues.current, kids_id);
    assert_eq!(current_item_ids(&owner), vec!["library"]);
    let parked = owner
        .named_queues
        .resting
        .first()
        .expect("the queue that was current is parked");
    assert_eq!(parked.id, default_id);
    let status = fixture.player.status.lock().unwrap();
    assert_eq!(status.current_idx, 0);
    assert_eq!(status.queue_len, 1);
    drop(status);
    assert_eq!(
        persisted_named_queues(&merged_rx, &owner, &fixture.shared_queue).current,
        kids_id
    );
}

/// Contract: spec `named-queues` "Clients without named queues see the current
/// queue" — a swap reaches a Client that negotiated no Named-queue capability as
/// one whole-queue snapshot of the new current queue.
#[test]
fn a_client_without_named_queues_receives_the_new_current_queue() {
    let fixture = swap_fixture();
    let (_observer, observer_rx) = connect_client(&mut fixture.registry.lock().unwrap());
    let mut owner = owner_holding(&["Default"]);
    let (kids_id, kids_slots) = resting_queue_with(&mut owner, "Kids", &["k1", "k2"]);

    run_named_command(
        CtrlCmd::NamedQueuePlay {
            queue: kids_id,
            slot: Some(kids_slots[0].raw()),
            op: mbv_ctrl::QueueOpId(79),
        },
        fixture.client_id,
        &fixture.reply_tx,
        &fixture.client,
        &fixture.player,
        &mut owner,
        &fixture.shared_queue,
        &fixture.registry,
    );

    let data = next_queue_snapshot(&observer_rx);
    assert_eq!(
        data.queue, None,
        "a Client without named queues sees the current queue"
    );
    assert_eq!(
        data.slots
            .iter()
            .map(|slot| slot.item.id().to_string())
            .collect::<Vec<_>>(),
        vec!["k1", "k2"]
    );
    assert_eq!(data.active_slot, Some(kids_slots[0].raw()));
    assert_eq!(data.start_slot, Some(kids_slots[0].raw()));
}
