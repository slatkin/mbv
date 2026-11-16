//! The current queue's start slot (gui-named-queues design D4): the slot a Play
//! of the queue starts, tracked from run observations at one write site
//! (`SharedQueueState::note_start_slot`) and carried to Clients in
//! `UnifiedQueueStateData::start_slot`.

use super::*;

/// The `start_slot` a peer receives for the Owner's current queue, projected
/// exactly as the daemon's broadcast projects it.
fn projected_start_slot(owner: &DaemonPlayerOwner, shared_queue: &SharedQueueState) -> Option<u64> {
    let CtrlEvent::UnifiedQueueState(data) = unified_queue_state_for_peer(
        None,
        &mbv_ctrl::player::PlayerStatus::default(),
        &owner.core.queue,
        &owner.core.source,
        mbv_queue::QueueLineage::default(),
        *shared_queue.observed_active_slot.lock().unwrap(),
        shared_queue.start_slot(),
        None,
        None,
        QueueTransport {
            abs_episode: true,
            abs_book: true,
            twitch: true,
        },
    ) else {
        panic!("expected UnifiedQueueState");
    };
    data.start_slot
}

/// A four-slot queue playing its third slot, with the start slot the run's
/// observation left on it.
fn owner_playing_its_third_slot() -> (DaemonPlayerOwner, mbv_queue::QueueSlotId) {
    let mut owner = owner_with(
        vec![
            emby_qi("a", "Video", "Movie"),
            emby_qi("b", "Video", "Movie"),
            emby_qi("c", "Video", "Movie"),
            emby_qi("d", "Video", "Movie"),
        ],
        2,
    );
    let third = owner.core.queue.slots()[2].slot_id;
    owner.core.note_observed_active_slot(Some(third));
    (owner, third)
}

/// Contract: spec `named-queues` "Each queue has a start slot" — a stop in the
/// middle of an item keeps that item's slot as the start slot, and peers receive
/// it, so a Play of the queue resumes there.
#[test]
fn a_stop_keeps_the_start_slot() {
    let player = cold_player();
    player.status.lock().unwrap().sequence_generation = 7;
    let shared_queue = shared_queue_state();
    let (mut owner, third) = owner_playing_its_third_slot();
    shared_queue.publish_observed(&owner.core);
    shared_queue.note_start_slot(
        &owner.core.queue,
        mbv_queue::StartSlotObservation::Active(third),
    );

    assert_eq!(
        apply_stopped_observation(
            &mut owner,
            &shared_queue,
            &player,
            7,
            Some(third),
            900,
            false
        ),
        Some(true)
    );

    assert_eq!(shared_queue.start_slot(), Some(third));
    assert_eq!(
        projected_start_slot(&owner, &shared_queue),
        Some(third.raw()),
        "the stopped slot reaches peers as the queue's start slot"
    );
}

/// Contract: spec `named-queues` "Each queue has a start slot" — a natural
/// completion makes the slot that followed the completed one the start slot.
#[test]
fn a_completion_moves_the_start_slot() {
    let shared_queue = shared_queue_state();
    let (mut owner, third) = owner_playing_its_third_slot();
    shared_queue.note_start_slot(
        &owner.core.queue,
        mbv_queue::StartSlotObservation::Active(third),
    );

    assert!(record_completed_slot(
        &mut owner,
        &shared_queue,
        third,
        0,
        true,
        false,
        ConsumePolicy {
            videos: true,
            audio: true,
        },
    ));

    let fourth = owner.core.queue.slots()[3].slot_id;
    assert_eq!(shared_queue.start_slot(), Some(fourth));
    assert_eq!(
        projected_start_slot(&owner, &shared_queue),
        Some(fourth.raw())
    );
}

/// Contract: spec `named-queues` "Play starts a queue at its start slot" — a
/// Play that names no items starts the current queue at its start slot, so a
/// Client with nothing playing resumes the queue where it left off.
#[test]
fn a_play_with_no_items_starts_the_start_slot() {
    let player = cold_player();
    let commands = player.spy_on_commands();
    let shared_queue = shared_queue_state();
    let (mut owner, third) = owner_playing_its_third_slot();
    shared_queue.note_start_slot(
        &owner.core.queue,
        mbv_queue::StartSlotObservation::Active(third),
    );
    let client = Arc::new(Mutex::new(EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, _reply_rx) = mpsc::channel();
    let (merged_tx, _merged_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl_for_role(
        CtrlCmd::PlaybackIntent(PlaybackIntent {
            request_id: 1,
            generation: 1,
            action: PlaybackIntentAction::Play {
                item_ids: Vec::new(),
                start_idx: 0,
                start_ticks: 0,
                source: QueueSource::Unknown,
            },
        }),
        CtrlContext {
            reply_tx: &reply_tx,
            client_id,
            client: &client,
            player: &player,
            audio_only: false,
            owner: &mut owner,
            shared_queue: &shared_queue,
            ctrl_clients: &registry,
            audiobookshelf: None,
            merged_tx: &merged_tx,
            owner_settings: crate::owner_settings::fixed_reader(false),
            role: crate::DaemonRole::Local,
            op: std::cell::Cell::new(None),
        },
    );

    match commands.recv().unwrap() {
        PlayerCommand::JumpTo { slot_id, .. } => assert_eq!(slot_id, third),
        other => panic!("a Play with no items jumps to the start slot, got {other:?}"),
    }
}

/// An Emby-configured client, so an Emby item is admissible.
fn queue_client() -> Arc<Mutex<EmbyClient>> {
    let mut client = EmbyClient::new(Config::default());
    client.token = "test-token".to_string();
    Arc::new(Mutex::new(client))
}

/// An Owner holding the Named queues `names` in queue order: the first is the
/// current queue and the rest rest. Every queue is empty.
fn owner_holding(names: &[&str]) -> DaemonPlayerOwner {
    let mut owner = DaemonPlayerOwner::default();
    let (current, resting) = names.split_first().expect("a table row names a queue");
    owner.named_queues.current_name = (*current).to_string();
    for name in resting {
        let index = owner.named_queues.push_new_queue(revision_mint());
        if let Some(queue) = owner.named_queues.resting.get_mut(index) {
            queue.name = (*name).to_string();
        }
    }
    owner
}

/// Adds one empty resting queue to `owner` and returns its identity.
fn add_resting_queue(owner: &mut DaemonPlayerOwner, name: &str) -> mbv_queue::NamedQueueId {
    let index = owner.named_queues.push_new_queue(revision_mint());
    let queue = owner
        .named_queues
        .resting
        .get_mut(index)
        .expect("the queue the store just added rests");
    queue.name = name.to_string();
    queue.id
}

/// Runs one ctrl command on a Local Owner with its own shared queue state, and
/// hands back the merged-event receiver so a test can read the persistence
/// request the command sent.
fn run_named_command(
    cmd: CtrlCmd,
    client_id: u64,
    reply_tx: &mpsc::Sender<CtrlOutbound>,
    client: &Arc<Mutex<EmbyClient>>,
    player: &Player,
    owner: &mut DaemonPlayerOwner,
    shared_queue: &SharedQueueState,
    registry: &Arc<Mutex<CtrlClients>>,
) -> mpsc::Receiver<DaemonEvent> {
    let (merged_tx, merged_rx) = mpsc::channel::<DaemonEvent>();
    handle_ctrl_for_role(
        cmd,
        CtrlContext {
            reply_tx,
            client_id,
            client,
            player,
            audio_only: false,
            owner,
            shared_queue,
            ctrl_clients: registry,
            audiobookshelf: None,
            merged_tx: &merged_tx,
            owner_settings: crate::owner_settings::fixed_reader(false),
            role: crate::DaemonRole::Local,
            op: std::cell::Cell::new(None),
        },
    );
    merged_rx
}

/// The item ids of a resting queue of `owner`.
fn resting_item_ids(owner: &DaemonPlayerOwner, index: usize) -> Vec<String> {
    owner
        .named_queues
        .resting
        .get(index)
        .map(|queue| {
            queue
                .queue
                .slots()
                .iter()
                .map(|slot| slot.item.id().to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// The pending persistence request a Named-queue command sends (design D6).
fn persisted_named_queues(merged_rx: &mpsc::Receiver<DaemonEvent>) -> mbv_config::NamedQueuesState {
    match merged_rx.recv().expect("the command persisted the store") {
        DaemonEvent::SaveNamedQueues(state) => *state,
        _ => panic!("expected the Named-queue persistence request"),
    }
}

/// Contract: spec `named-queues` "Queue edits name their queue" — an append that
/// names a resting queue lands in that queue, playback does not change, and the
/// edit is answered with the queue's own snapshot (design D5).
#[test]
fn append_to_a_resting_queue_edits_the_store() {
    let player = cold_player();
    let cmd_rx = player.spy_on_commands();
    let client = queue_client();
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, reply_tx, reply_rx) = {
        let (id, _rx) = connect_client(&mut registry.lock().unwrap());
        let (tx, reply_rx) = mpsc::channel();
        (id, tx, reply_rx)
    };
    let shared_queue = shared_queue_state();
    let mut owner = owner_with(vec![emby_qi("a", "Video", "Movie")], 0);
    let resting_id = add_resting_queue(&mut owner, "Kids");
    if let Some(kids) = owner.named_queues.resting.first_mut() {
        kids.queue.append(emby_qi("k", "Video", "Movie"));
    }

    let merged_rx = run_named_command(
        CtrlCmd::UnifiedQueueAppend {
            queue: Some(resting_id),
            items: vec![emby_qi("b", "Video", "Movie")],
            before: None,
            op: Some(mbv_ctrl::QueueOpId(70)),
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &shared_queue,
        &registry,
    );

    assert_eq!(resting_item_ids(&owner, 0), vec!["k", "b"]);
    assert_eq!(
        owner.core.queue.len(),
        1,
        "the current queue keeps its slots"
    );
    assert!(
        cmd_rx.try_recv().is_err(),
        "a resting queue's edit never reaches the player"
    );
    match recv_event(&reply_rx) {
        CtrlEvent::QueueOpResult {
            op: mbv_ctrl::QueueOpId(70),
            outcome: mbv_ctrl::QueueOpOutcome::Applied(data),
        } => {
            assert_eq!(data.queue, Some(resting_id));
            assert_eq!(
                data.slots
                    .iter()
                    .map(|slot| slot.item.id().to_string())
                    .collect::<Vec<_>>(),
                vec!["k", "b"]
            );
        }
        _ => panic!("expected the resting queue's snapshot as the op answer"),
    }
    let persisted = persisted_named_queues(&merged_rx);
    assert_eq!(persisted.queues.len(), 2, "the whole store is persisted");
}

/// Contract: spec `named-queues` "Queue edits name their queue" — an edit that
/// names a queue the Owner no longer holds is rejected and no queue changes.
#[test]
fn an_edit_that_names_a_closed_queue_is_rejected() {
    let player = cold_player();
    let cmd_rx = player.spy_on_commands();
    let client = queue_client();
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    let mut owner = owner_with(vec![emby_qi("a", "Video", "Movie")], 0);
    add_resting_queue(&mut owner, "Kids");
    let slot_id = owner
        .named_queues
        .resting
        .first_mut()
        .map(|kids| kids.queue.append(emby_qi("k", "Video", "Movie")))
        .expect("the resting queue holds the removed slot");
    // The identity a Client still holds after another Client closed that queue.
    let closed = mbv_queue::NamedQueueId::from_raw(99);

    let merged_rx = run_named_command(
        CtrlCmd::UnifiedQueueRemoveSlot {
            queue: Some(closed),
            slot_id: slot_id.raw(),
            op: Some(mbv_ctrl::QueueOpId(71)),
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &shared_queue,
        &registry,
    );

    assert_eq!(resting_item_ids(&owner, 0), vec!["k"], "no queue changed");
    assert_eq!(owner.core.queue.len(), 1);
    assert!(
        cmd_rx.try_recv().is_err(),
        "no player command on a rejection"
    );
    match recv_event(&reply_rx) {
        CtrlEvent::QueueOpResult {
            op: mbv_ctrl::QueueOpId(71),
            outcome: mbv_ctrl::QueueOpOutcome::Rejected(reason),
        } => assert_eq!(reason, "named queue not found; edit skipped"),
        _ => panic!("expected the edit to be rejected"),
    }
    assert!(
        merged_rx.try_recv().is_err(),
        "a rejected edit persists nothing"
    );
}

/// Contract: spec `named-queues` "A Client creates and renames queues" — a new
/// queue is named `Queue N` for the lowest N from 2 up that no queue uses.
#[rstest]
#[case::a_new_queue_is_made(&["Default", "Queue 2"], "Queue 3")]
#[case::the_first_new_queue(&["Default"], "Queue 2")]
#[case::a_freed_number_is_used_again(&["Default", "Queue 3"], "Queue 2")]
fn a_new_queue_takes_the_lowest_free_number(#[case] existing: &[&str], #[case] expected: &str) {
    let player = cold_player();
    let client = queue_client();
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, _reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    let mut owner = owner_holding(existing);

    let merged_rx = run_named_command(
        CtrlCmd::NamedQueueCreate {
            op: mbv_ctrl::QueueOpId(72),
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &shared_queue,
        &registry,
    );

    let new_queue = owner
        .named_queues
        .resting
        .last()
        .expect("the Owner holds the new queue");
    assert_eq!(new_queue.name, expected);
    assert!(new_queue.queue.is_empty(), "a new queue is empty");
    assert_ne!(
        owner.named_queues.current, new_queue.id,
        "a new queue rests; the current queue still holds the playhead"
    );
    assert_eq!(
        persisted_named_queues(&merged_rx).queues.len(),
        existing.len() + 1
    );
}

/// Contract: spec `named-queues` "A Client creates and renames queues" — a
/// rename to a name of only spaces is refused and the name does not change.
#[test]
fn an_empty_name_is_refused() {
    let player = cold_player();
    let client = queue_client();
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    let mut owner = owner_holding(&["Default"]);
    let current = owner.named_queues.current;

    let merged_rx = run_named_command(
        CtrlCmd::NamedQueueRename {
            queue: current,
            name: "   ".to_string(),
            op: mbv_ctrl::QueueOpId(73),
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &shared_queue,
        &registry,
    );

    assert_eq!(owner.named_queues.current_name, "Default");
    match recv_event(&reply_rx) {
        CtrlEvent::QueueOpResult {
            op: mbv_ctrl::QueueOpId(73),
            outcome: mbv_ctrl::QueueOpOutcome::Rejected(reason),
        } => assert_eq!(reason, "queue name is empty; rename skipped"),
        _ => panic!("expected the rename to be rejected"),
    }
    assert!(
        merged_rx.try_recv().is_err(),
        "a refused rename persists nothing"
    );
}

/// Contract: spec `named-queues` "A Client closes queues" — closing the playing
/// current queue stops playback, deletes it, and makes the first remaining
/// queue in queue order current without playing.
#[test]
fn close_the_playing_queue_stops_and_installs_the_first_remaining_queue() {
    let player = cold_player();
    player.status.lock().unwrap().active = true;
    let cmd_rx = player.spy_on_commands();
    let client = queue_client();
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    let mut owner = owner_with(vec![emby_qi("kids", "Video", "Movie")], 0);
    owner.named_queues.current_name = "Kids".to_string();
    let default_id = add_resting_queue(&mut owner, "Default");
    let default_slot = {
        let default = owner
            .named_queues
            .resting
            .first_mut()
            .expect("the resting queue exists");
        let slot = default.queue.append(emby_qi("default", "Video", "Movie"));
        default.note_start_slot(mbv_queue::StartSlotObservation::Active(slot));
        slot
    };
    let closed = owner.named_queues.current;

    let merged_rx = run_named_command(
        CtrlCmd::NamedQueueClose {
            queue: closed,
            op: mbv_ctrl::QueueOpId(74),
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &shared_queue,
        &registry,
    );

    assert_eq!(
        owner.named_queues.current, default_id,
        "Default is current now"
    );
    assert_eq!(owner.named_queues.current_name, "Default");
    assert_eq!(owner.named_queues.resting.len(), 0, "Kids is gone");
    assert_eq!(
        owner
            .core
            .queue
            .slots()
            .iter()
            .map(|slot| slot.item.id().to_string())
            .collect::<Vec<_>>(),
        vec!["default"],
        "the installed queue's slots keep their identities"
    );
    assert_eq!(owner.core.queue.slots()[0].slot_id, default_slot);
    assert_eq!(
        shared_queue.start_slot(),
        Some(default_slot),
        "the installed queue's own start slot becomes the current one"
    );
    match cmd_rx.recv().expect("the run's queue is emptied") {
        PlayerCommand::SubmitQueue { items, .. } => assert!(items.is_empty()),
        _ => panic!("expected the run's queue to be emptied"),
    }
    match recv_event(&reply_rx) {
        CtrlEvent::QueueOpResult {
            op: mbv_ctrl::QueueOpId(74),
            outcome: mbv_ctrl::QueueOpOutcome::Applied(data),
        } => {
            assert_eq!(data.queue, None, "the answer describes the current queue");
            assert_eq!(data.slots[0].item.id(), "default");
        }
        _ => panic!("expected the close to be answered"),
    }
    let persisted = persisted_named_queues(&merged_rx);
    assert_eq!(persisted.current, default_id);
    assert_eq!(persisted.queues.len(), 1);
}

/// Contract: spec `named-queues` "A Client closes queues" — the last queue does
/// not close.
#[test]
fn the_last_queue_does_not_close() {
    let player = cold_player();
    let client = queue_client();
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, _rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, reply_rx) = mpsc::channel();
    let shared_queue = shared_queue_state();
    let mut owner = owner_holding(&["Default"]);
    let current = owner.named_queues.current;

    let merged_rx = run_named_command(
        CtrlCmd::NamedQueueClose {
            queue: current,
            op: mbv_ctrl::QueueOpId(75),
        },
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &shared_queue,
        &registry,
    );

    assert_eq!(owner.named_queues.current, current);
    assert_eq!(owner.named_queues.current_name, "Default");
    match recv_event(&reply_rx) {
        CtrlEvent::QueueOpResult {
            op: mbv_ctrl::QueueOpId(75),
            outcome: mbv_ctrl::QueueOpOutcome::Rejected(reason),
        } => assert_eq!(reason, "the last named queue does not close"),
        _ => panic!("expected the close to be rejected"),
    }
    assert!(
        merged_rx.try_recv().is_err(),
        "a refused close persists nothing"
    );
}
