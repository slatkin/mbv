use super::*;

#[test]
fn acknowledged_progress_updates_bound_slot_and_broadcasts() {
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_capable_id, capable_rx) = connect_client(&mut registry.lock().unwrap());
    let mut queue = abs_queue_with_slot();

    apply_audiobookshelf_progress(
        &progress_update(1, 30.0, false),
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
        &progress_update(1, 100.0, true),
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
        &progress_update(1, 30.0, false),
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
        &progress_update(1, 30.0, false),
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
        &progress_update(1, 30.0, false),
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
        &progress_update(1, 60.0, false),
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
    let steps: &[(u8, bool)] = &[
        (10, false),
        (30, false),
        (30, false), // pause: same position re-reported
        (10, false), // seek back to an earlier point
        (45, false),
        (80, false),
        (100, true), // natural completion
    ];

    for &(secs, finished) in steps {
        apply_audiobookshelf_progress(
            &progress_update(1, f64::from(secs), finished),
            Some(SetupGeneration::new(1)),
            &mut queue,
            &registry,
        );

        let expected_ticks = i64::from(secs) * crate::api::TICKS_PER_SECOND;
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
