//! The current queue's start slot (gui-named-queues design D4): the slot a Play
//! of the queue starts, tracked from run observations at one write site
//! (`SharedQueueState::note_start_slot`) and carried to Clients in
//! `UnifiedQueueStateData::start_slot`.

use super::*;

/// The `start_slot` a peer receives for the Owner's current queue, projected
/// exactly as the daemon's broadcast projects it.
fn projected_start_slot(owner: &DaemonPlayerOwner, shared_queue: &SharedQueueState) -> Option<u64> {
    let CtrlEvent::UnifiedQueueState(data) = unified_queue_state_for_peer(
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
