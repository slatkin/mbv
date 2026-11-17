//! The state file a restart reloads (design D6 of `gui-named-queues`): a change
//! of the current queue writes every Named queue when the persisted content
//! changes, a change that only clears away writes nothing, and the restore
//! rebuilds the current queue and the resting queues with their saved slot
//! identities and start slots.

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

/// Contract: design D6 — the event loop writes the Named-queue file only when
/// the persisted content changes. The file holds no Service position, so a
/// stop that only records a new position clears away to the same state, and a
/// rewrite would clone and serialize every resting queue for nothing.
#[test]
fn a_stop_that_only_records_a_position_writes_the_named_queues_once() {
    let mut t = crate::tests::r#loop::test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![
            emby_qi("a", "Video", "Movie"),
            emby_qi("b", "Video", "Movie"),
        ],
        1,
    );
    let second = t.event_loop.owner.core.queue.slots()[1].slot_id;
    let run_identity = t
        .event_loop
        .player
        .status
        .lock()
        .unwrap()
        .sequence_generation;
    let stopped = |position_ticks: i64| {
        DaemonEvent::Player(PlayerEvent::Stopped {
            slot_id: Some(second),
            run_identity,
            position_ticks,
            played: false,
            consume: false,
            error: None,
        })
    };

    t.event_loop.handle_event(stopped(600));
    assert_eq!(
        t.persisted_named.borrow().len(),
        1,
        "the first change writes the file"
    );

    t.event_loop.handle_event(stopped(900));
    assert_eq!(
        t.persisted_named.borrow().len(),
        1,
        "a second stop that records only a position writes nothing: the file holds no positions"
    );
}

/// Contract: design D6 — an unreadable Named-queue file degrades to the earlier
/// single-queue owner file, so one corrupt file cannot reset every queue it
/// held. The restore reads only; it never rewrites either file.
#[test]
fn a_corrupt_named_file_restores_the_current_queue_from_the_single_queue_file() {
    let scratch = mbv_config::TestTempDir::new();
    let named_path = scratch.join("named_queues_state.json");
    let owner_path = scratch.join("stay_alive_queue_state.json");
    std::fs::write(&named_path, "{ not a named-queue state").unwrap();
    let queue = PlaybackQueue::from_queue_items(
        vec![
            emby_qi("a", "Video", "Movie"),
            emby_qi("b", "Video", "Movie"),
            emby_qi("c", "Video", "Movie"),
        ],
        Some(1),
        revision_mint(),
    );
    mbv_config::save_stay_alive_queue_state_at(
        &owner_path,
        &StayAliveQueueState {
            queue: crate::project_queue_state(
                &queue,
                &QueueSource::Album,
                &PlayerStatus::default(),
            ),
            lineage: mbv_queue::QueueLineage(4),
        },
    )
    .unwrap();
    let written = std::fs::read(&named_path).unwrap();

    let state = restored_state_at(&named_path, &owner_path)
        .expect("the earlier single-queue file restores the current queue");

    assert_eq!(state.queues.len(), 1);
    let restored = state.queues.first().expect("one queue restores");
    assert_eq!(restored.id, state.current);
    assert_eq!(restored.name, "Default");
    assert_eq!(restored.source, QueueSource::Album);
    assert_eq!(
        restored
            .slots
            .iter()
            .map(|slot| slot.item.id())
            .collect::<Vec<_>>(),
        vec!["a", "b", "c"]
    );
    assert_eq!(
        restored.start_slot,
        Some(mbv_queue::QueueSlotId::from_raw(2)),
        "the position the earlier Owner saved becomes the start slot"
    );
    assert_eq!(
        std::fs::read(&named_path).unwrap(),
        written,
        "the corrupt file is left as it was"
    );
}
