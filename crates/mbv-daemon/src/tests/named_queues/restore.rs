//! The state file a restart reloads (design D6 of `gui-named-queues`): a change of
//! the current queue writes every Named queue, and the restore rebuilds the
//! current queue and the resting queues with their saved slot identities and
//! start slots.

use super::*;

/// Contract: spec `local-daemon-thin-client` "Restart restores every named
/// queue" — a change of the current queue writes the whole state file, and a
/// restart reloads every queue with its name, source, slot identities, and start
/// slot, keeping `Default` current (design D6).
#[test]
fn a_restart_restores_every_named_queue() {
    let mut t = crate::tests::r#loop::test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![
            emby_qi("a", "Video", "Movie"),
            emby_qi("b", "Video", "Movie"),
            emby_qi("c", "Video", "Movie"),
        ],
        2,
    );
    let third = t.event_loop.owner.core.queue.slots()[2].slot_id;
    t.event_loop.shared_queue.note_start_slot(
        &t.event_loop.owner.core.queue,
        mbv_queue::StartSlotObservation::Active(third),
    );
    t.event_loop.owner.core.source = QueueSource::Playlist {
        id: None,
        name: "Morning".to_string(),
    };
    let current_slot_ids = slot_ids(&t.event_loop.owner.core.queue);
    let kids_id = add_resting_queue(&mut t.event_loop.owner, "Kids");
    let kids_start = {
        let kids = t
            .event_loop
            .owner
            .named_queues
            .resting
            .first_mut()
            .expect("Kids rests");
        let start = kids.queue.append(emby_qi("k", "Video", "Movie"));
        kids.source = QueueSource::Album;
        kids.start_slot = Some(start);
        start
    };
    let run_identity = t
        .event_loop
        .player
        .status
        .lock()
        .unwrap()
        .sequence_generation;

    // A stop is a change of the current queue, so it writes the whole file.
    t.event_loop
        .handle_event(DaemonEvent::Player(PlayerEvent::Stopped {
            slot_id: Some(third),
            run_identity,
            position_ticks: 600,
            played: false,
            consume: false,
            error: None,
        }));
    let written = t
        .persisted_named
        .borrow()
        .last()
        .cloned()
        .expect("a change of the current queue writes the Named queues");
    let state: mbv_config::NamedQueuesState =
        serde_json::from_value(written).expect("the written state reloads");

    let startup = StartupQueues::restored(&state, &revision_mint()).expect("two queues restore");

    assert_eq!(state.queues.len(), 2, "the file holds every queue");
    assert_eq!(startup.store.current, state.current);
    assert_eq!(startup.store.current_name, "Default");
    assert_eq!(
        startup.store.next_queue_id,
        mbv_queue::NamedQueueId::from_raw(3)
    );
    assert_eq!(
        startup.source,
        QueueSource::Playlist {
            id: None,
            name: "Morning".to_string()
        }
    );
    assert_eq!(slot_ids(&startup.queue), current_slot_ids);
    assert_eq!(
        startup.queue.active_slot_id(),
        Some(third),
        "the saved start slot survives the restart with its slot identity"
    );

    let kids = startup
        .store
        .resting
        .first()
        .expect("Kids rests again after the restart");
    assert_eq!(kids.id, kids_id);
    assert_eq!(kids.name, "Kids");
    assert_eq!(kids.source, QueueSource::Album);
    assert_eq!(kids.start_slot, Some(kids_start));
    assert_eq!(
        kids.queue.slots()[0].item.id(),
        "k",
        "the resting queue keeps its slot"
    );
}

/// The slot identities of one queue, in queue order.
fn slot_ids(queue: &PlaybackQueue) -> Vec<mbv_queue::QueueSlotId> {
    queue.slots().iter().map(|slot| slot.slot_id).collect()
}
