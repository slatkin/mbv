//! Remote-consume projection and ordering tests, split out of
//! `tests_remote_reconciliation.rs` to keep that file within the
//! repository's file-size limit.

use super::{attached_app, projection, tracker};
use crate::app::tests::{make_item, make_session};
use crate::app::*;
use mbv_core::remote_reconciliation::{
    ReconciliationTracker, RemoteIntent, RemoteObservation, SubmittedOccurrence, TrackingState,
};

#[test]
fn stale_consume_cannot_remove_a_replacement_slot() {
    let mut app = attached_app();
    app.player_tab.sync_queue_model_from_items_if_needed();
    let old_slot = app.player_tab.queue.slots()[0].slot_id;
    app.remote_consume_operations
        .push(crate::app::types_playback::RemoteConsumeOperation {
            operation_id: 8,
            mutation_id: 8,
            session_id: "session".into(),
            tracking_id: 0,
            epoch: 0,
            occurrence_id: 1,
            playlist_id: "playlist".into(),
            entry_id: "entry".into(),
            media_id: "a".into(),
            queue_slot_id: Some(old_slot),
            queue_lineage: app.remote_queue_lineage,
        });
    let mut replacement = make_item("replacement", "Movie");
    replacement.id = "replacement".into();
    app.replace_playback_queue(vec![replacement], 0);

    app.handle_session_event(SessionEvent::ConsumeOutcome {
        mutation_id: 8,
        operation_id: 8,
        tracking_id: 0,
        session_id: "session".into(),
        epoch: 0,
        occurrence_id: 1,
        playlist_id: "playlist".into(),
        entry_id: "entry".into(),
        media_id: "a".into(),
        result: Ok(()),
    });

    assert_eq!(app.player_tab.items.len(), 1);
    assert_eq!(app.player_tab.items[0].id, "replacement");
}

#[test]
fn already_absent_consume_projects_after_tracking_retirement() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = attached_app();
    app.player_tab.items[0].id = "a".into();
    app.player_tab.items[1].id = "b".into();
    app.player_tab.items[0].playlist_item_id = "entry-a".into();
    app.player_tab.sync_queue_model_from_items_if_needed();
    let mut tracking = tracker(&["a", "b"]);
    let tracking_id = tracking.tracking_id();
    tracking.observe(RemoteObservation::playing(1, "session", "a", 95, 100, 1));
    tracking.observe(RemoteObservation::playing(2, "session", "b", 1, 100, 2));
    assert!(tracking.mark_consumed(1));
    app.remote_tracker = Some(tracking);
    let slot_id = app.player_tab.queue.slots()[0].slot_id;
    app.remote_consume_operations
        .push(crate::app::types_playback::RemoteConsumeOperation {
            operation_id: 9,
            mutation_id: 9,
            session_id: "session".into(),
            tracking_id,
            epoch: 0,
            occurrence_id: 1,
            playlist_id: "playlist-1".into(),
            entry_id: "entry-a".into(),
            media_id: "a".into(),
            queue_slot_id: Some(slot_id),
            queue_lineage: app.remote_queue_lineage,
        });
    app.retire_remote_tracking(false);
    app.handle_session_event(SessionEvent::ConsumeValidated {
        mutation_id: 9,
        operation_id: 9,
        tracking_id,
        session_id: "session".into(),
        epoch: 0,
        occurrence_id: 1,
        playlist_id: "playlist-1".into(),
        entry_id: "entry-a".into(),
        media_id: "a".into(),
        result: Ok(false),
    });
    assert_eq!(app.player_tab.items.len(), 1);
    assert_eq!(app.player_tab.items[0].id, "b");
}

#[test]
fn mismatched_validation_event_cannot_remove_operation_by_id() {
    let mut app = attached_app();
    app.remote_consume_operations
        .push(crate::app::types_playback::RemoteConsumeOperation {
            operation_id: 10,
            mutation_id: 10,
            session_id: "session".into(),
            tracking_id: 1,
            epoch: 2,
            occurrence_id: 3,
            playlist_id: "playlist-a".into(),
            entry_id: "entry-a".into(),
            media_id: "a".into(),
            queue_slot_id: None,
            queue_lineage: app.remote_queue_lineage,
        });
    app.handle_session_event(SessionEvent::ConsumeValidated {
        mutation_id: 10,
        operation_id: 10,
        tracking_id: 1,
        session_id: "session".into(),
        epoch: 2,
        occurrence_id: 3,
        playlist_id: "playlist-b".into(),
        entry_id: "entry-a".into(),
        media_id: "a".into(),
        result: Err("mismatched".into()),
    });
    assert_eq!(app.remote_consume_operations.len(), 1);
}

#[test]
fn successful_remote_command_acknowledgment_unfreezes_tracking_after_failed_poll() {
    let mut app = attached_app();
    app.player_tab.items[0].id = "a".into();
    app.player_tab.items[1].id = "b".into();
    app.connected_session_state = Some({
        let mut state = make_session("Client", "Emby");
        state.id = "session".into();
        state.now_playing_item_id = Some("a".into());
        state.position_ticks = 1;
        state.runtime_ticks = 100;
        state
    });
    let mut tracking = tracker(&["a", "b"]);
    tracking.observe(RemoteObservation::playing(1, "session", "a", 1, 100, 1));
    tracking.issue_intent(RemoteIntent::Next { target: 2 }, 2);
    tracking.track_command_generation(5);
    let tracking_id = tracking.tracking_id();
    app.remote_tracker = Some(tracking);

    // The command succeeded, so a correlated acknowledgment arrives even
    // though the immediate post-command session poll failed (only Error
    // follows). Tracking must not freeze on the lost follow-up poll.
    app.handle_session_event(SessionEvent::CommandAcknowledged(ReconciliationCommand {
        session_id: "session".into(),
        tracking_id,
        tracker_epoch: 0,
        generation: 5,
    }));

    // A later ordinary poll issued after the acknowledgment boundary is
    // eligible to confirm/reconcile.
    let mut later = make_session("Client", "Emby");
    later.id = "session".into();
    later.now_playing_item_id = Some("b".into());
    later.position_ticks = 1;
    later.runtime_ticks = 100;
    app.handle_session_event(SessionEvent::Loaded {
        sessions: vec![later],
        generation: 6,
    });

    let tracker = app.remote_tracker.as_ref().unwrap();
    assert_eq!(tracker.state(), TrackingState::Tracking);
    assert_eq!(tracker.current_occurrence().unwrap().occurrence_id, 2);
}

#[test]
fn stale_acknowledgment_for_replaced_tracker_is_inert() {
    let mut app = attached_app();
    let mut original = tracker(&["a", "b"]);
    original.track_command_generation(4);
    let original_id = original.tracking_id();
    app.remote_tracker = Some(original);

    let replacement = tracker(&["a", "b"]);
    app.remote_tracker = Some(replacement);

    app.handle_session_event(SessionEvent::CommandAcknowledged(ReconciliationCommand {
        session_id: "session".into(),
        tracking_id: original_id,
        tracker_epoch: 0,
        generation: 4,
    }));

    assert!(app.remote_tracker.as_ref().unwrap().is_active());
}

#[test]
fn tracked_command_failure_retires_tracking_and_preserves_error_message() {
    let mut app = attached_app();
    let mut tracking = tracker(&["a", "b"]);
    tracking.track_command_generation(4);
    let tracking_id = tracking.tracking_id();
    app.remote_tracker = Some(tracking);

    app.handle_session_event(SessionEvent::CommandError {
        error: "playback command failed".into(),
        reconciliation: Some(ReconciliationCommand {
            session_id: "session".into(),
            tracking_id,
            tracker_epoch: 0,
            generation: 4,
        }),
    });

    assert!(app.remote_tracker.is_none());
    assert!(app
        .status
        .contains("Remote command failed: playback command failed"));
}

#[test]
fn older_tracked_command_failure_does_not_retire_a_newer_command_tracker() {
    let mut app = attached_app();
    let mut tracking = tracker(&["a", "b", "c"]);
    tracking.observe(RemoteObservation::playing(1, "session", "a", 1, 100, 1));
    tracking.issue_intent(RemoteIntent::Next { target: 2 }, 2);
    tracking.track_command_generation(5);
    tracking.issue_intent(RemoteIntent::Next { target: 3 }, 3);
    tracking.track_command_generation(7);
    let tracking_id = tracking.tracking_id();
    app.remote_tracker = Some(tracking);

    app.handle_session_event(SessionEvent::CommandError {
        error: "older command failed".into(),
        reconciliation: Some(ReconciliationCommand {
            session_id: "session".into(),
            tracking_id,
            tracker_epoch: 0,
            generation: 5,
        }),
    });

    assert!(app.remote_tracker.as_ref().unwrap().is_active());
    assert_eq!(
        app.remote_tracker
            .as_ref()
            .unwrap()
            .expected()
            .unwrap()
            .intent,
        RemoteIntent::Next { target: 3 }
    );
}

#[test]
fn remote_jump_target_is_independent_of_tracking() {
    let mut app = attached_app();
    app.player_tab.items[0].id = "a".into();
    app.player_tab.items[1].id = "a".into();
    app.player_tab.items.push(make_item("b", "Movie"));
    app.player_tab.items[2].id = "b".into();
    app.player_tab.items[0].playback_position_ticks = 10;
    app.player_tab.items[1].playback_position_ticks = 20;
    app.player_tab.items[2].playback_position_ticks = 30;

    // The untracked destination resolution resolves the first visible match.
    let base = crate::app::session_command_actions::remote_jump_target(
        &app.player_tab.items,
        Some("a"),
        1,
    );
    assert_eq!(base, Some((1, 20)));

    // Tracking presence never changes the destination or payload.
    app.remote_tracker = Some(tracker(&["a", "a", "b"]));
    assert_eq!(
        crate::app::session_command_actions::remote_jump_target(
            &app.player_tab.items,
            Some("a"),
            1
        ),
        base
    );
    assert_eq!(
        crate::app::session_command_actions::remote_jump_target(
            &app.player_tab.items,
            Some("b"),
            -1
        ),
        Some((1, 20))
    );

    // After projected prefix consumption the visible queue shrinks; the
    // destination is still chosen from the visible queue exactly as untracked.
    let items: Vec<_> = app.player_tab.items.iter().skip(1).cloned().collect();
    assert_eq!(
        crate::app::session_command_actions::remote_jump_target(&items, Some("a"), 1),
        Some((1, 30))
    );
}

#[test]
fn session_jump_track_records_occurrence_intent_for_untracked_destination() {
    let mut app = attached_app();
    app.player_tab.items[0].id = "a".into();
    app.player_tab.items[1].id = "b".into();
    app.player_tab.items.push(make_item("c", "Movie"));
    app.player_tab.items[2].id = "c".into();
    app.player_tab.sync_queue_model_from_items_if_needed();
    app.connected_session_state = Some({
        let mut state = make_session("Client", "Emby");
        state.id = "session".into();
        state.now_playing_item_id = Some("a".into());
        state
    });
    let mut tracking = tracker(&["a", "b", "c"]);
    tracking.observe(RemoteObservation::playing(1, "session", "a", 1, 100, 1));
    app.remote_tracker = Some(tracking);
    let slots = app.player_tab.queue.slots();
    app.remote_queue_projection = Some(projection(
        &app,
        &[
            (1, slots[0].slot_id),
            (2, slots[1].slot_id),
            (3, slots[2].slot_id),
        ],
    ));

    // Next from visible "a" (index 0) resolves to index 1 = "b" through the
    // untracked path and records the submitted occurrence intent.
    app.session_jump_track("session", 1, "NextTrack");

    assert_eq!(
        app.remote_tracker
            .as_ref()
            .unwrap()
            .expected()
            .unwrap()
            .intent,
        RemoteIntent::Next { target: 2 }
    );
}

#[test]
fn consume_followed_by_next_targets_the_projected_occurrence() {
    let mut app = attached_app();
    app.player_tab.items[0].id = "a".into();
    app.player_tab.items[0].playlist_item_id = "entry-a".into();
    app.player_tab.items[1].id = "b".into();
    app.player_tab.items.push(make_item("c", "Movie"));
    app.player_tab.items[2].id = "c".into();
    app.player_tab.sync_queue_model_from_items_if_needed();
    let slots = app.player_tab.queue.slots();
    app.remote_queue_projection = Some(projection(
        &app,
        &[
            (1, slots[0].slot_id),
            (2, slots[1].slot_id),
            (3, slots[2].slot_id),
        ],
    ));

    let mut tracking = ReconciliationTracker::new(
        "session",
        vec![
            SubmittedOccurrence::new(1, "a").playlist_entry("entry-a"),
            SubmittedOccurrence::new(2, "b"),
            SubmittedOccurrence::new(3, "c"),
        ],
        0,
        0,
    )
    .unwrap();
    tracking.observe(RemoteObservation::playing(1, "session", "a", 95, 100, 1));
    tracking.observe(RemoteObservation::playing(2, "session", "b", 1, 100, 2));
    assert!(tracking.mark_consumed(1));
    app.remote_tracker = Some(tracking);

    // Project the applied consume of occurrence 1, shrinking the visible
    // queue from [a, b, c] to [b, c].
    app.player_tab.sync_queue_model_from_items_if_needed();
    let consumed_slot = app.player_tab.queue.slots()[0].slot_id;
    assert!(matches!(
        app.player_tab.queue.consume_slot(consumed_slot),
        mbv_core::playback_queue::QueueMutationResult::Applied(_)
    ));
    app.player_tab.sync_items_from_queue_model();
    assert_eq!(app.player_tab.items.len(), 2);
    assert_eq!(app.player_tab.items[0].id, "b");

    app.connected_session_state = Some({
        let mut state = make_session("Client", "Emby");
        state.id = "session".into();
        state.now_playing_item_id = Some("b".into());
        state
    });

    // Next resolves visible "b" (index 0) to "c" (index 1) and records the
    // submitted occurrence for "c" without confusing visible index with the
    // submitted sequence.
    app.session_jump_track("session", 1, "NextTrack");
    assert_eq!(
        app.remote_tracker
            .as_ref()
            .unwrap()
            .expected()
            .unwrap()
            .intent,
        RemoteIntent::Next { target: 3 }
    );
    assert_eq!(
        app.remote_tracker.as_ref().unwrap().submitted().len(),
        3,
        "immutable Submitted sequence is preserved"
    );
}

// ── task 5.4: consume followed by Previous / direct selection / duplicate ──

/// Shared 5.4 setup: `[a, b, c]` submitted to a saved playlist, occurrence 1
/// completed and consumed, projection active, occurrence 2 confirmed.
fn consume_first_of_three(app: &mut App) {
    app.player_tab.items[0].id = "a".into();
    app.player_tab.items[0].playlist_item_id = "entry-a".into();
    app.player_tab.items[1].id = "b".into();
    app.player_tab.items.push(make_item("c", "Movie"));
    app.player_tab.items[2].id = "c".into();
    app.player_tab.items[0].playback_position_ticks = 100;
    app.player_tab.items[1].playback_position_ticks = 200;
    app.player_tab.items[2].playback_position_ticks = 300;
    app.player_tab.sync_queue_model_from_items_if_needed();
    let slots = app.player_tab.queue.slots();
    app.remote_queue_projection = Some(projection(
        app,
        &[
            (1, slots[0].slot_id),
            (2, slots[1].slot_id),
            (3, slots[2].slot_id),
        ],
    ));

    let mut tracking = ReconciliationTracker::new(
        "session",
        vec![
            SubmittedOccurrence::new(1, "a").playlist_entry("entry-a"),
            SubmittedOccurrence::new(2, "b"),
            SubmittedOccurrence::new(3, "c"),
        ],
        0,
        0,
    )
    .unwrap();
    tracking.observe(RemoteObservation::playing(1, "session", "a", 95, 100, 1));
    tracking.observe(RemoteObservation::playing(2, "session", "b", 1, 100, 2));
    assert!(tracking.mark_consumed(1));
    app.remote_tracker = Some(tracking);

    app.player_tab.sync_queue_model_from_items_if_needed();
    let consumed_slot = app.player_tab.queue.slots()[0].slot_id;
    assert!(matches!(
        app.player_tab.queue.consume_slot(consumed_slot),
        mbv_core::playback_queue::QueueMutationResult::Applied(_)
    ));
    app.player_tab.sync_items_from_queue_model();
    assert_eq!(app.player_tab.items.len(), 2);
    assert_eq!(app.player_tab.items[0].id, "b");
}

#[test]
fn consume_followed_by_previous_targets_the_projected_occurrence() {
    let mut app = attached_app();
    consume_first_of_three(&mut app);
    // Advance the tracked occurrence to c so Previous has a resolved source
    // and a real destination inside the visible queue.
    app.remote_tracker
        .as_mut()
        .unwrap()
        .observe(RemoteObservation::playing(3, "session", "c", 1, 100, 3));
    app.connected_session_state = Some({
        let mut state = make_session("Client", "Emby");
        state.id = "session".into();
        state.now_playing_item_id = Some("c".into());
        state
    });

    // Previous from visible "c" (index 1) resolves through the untracked path
    // to visible "b" (index 0), then translates that slot back to the
    // Submitted occurrence for "b" — never the consumed "a".
    app.session_jump_track("session", -1, "PreviousTrack");
    assert_eq!(
        app.remote_tracker
            .as_ref()
            .unwrap()
            .expected()
            .unwrap()
            .intent,
        RemoteIntent::Previous { target: 2 }
    );
    assert_eq!(
        app.remote_tracker.as_ref().unwrap().submitted().len(),
        3,
        "immutable Submitted sequence is preserved"
    );
}

#[test]
fn consume_followed_by_direct_selection_targets_the_projected_occurrence() {
    let mut app = attached_app();
    consume_first_of_three(&mut app);
    app.connected_session_state = Some({
        let mut state = make_session("Client", "Emby");
        state.id = "session".into();
        state.now_playing_item_id = Some("b".into());
        state
    });

    // Direct selection of visible "b" (index 0) issues a Select intent for the
    // Submitted occurrence that still owns that slot.
    app.player_tab.queue_cursor = 0;
    app.dispatch(crate::app::action::Command::QueuePlayCursor);
    assert_eq!(
        app.remote_tracker
            .as_ref()
            .unwrap()
            .expected()
            .unwrap()
            .intent,
        RemoteIntent::Select { target: 2 }
    );
}

#[test]
fn consume_followed_by_duplicate_selection_targets_distinct_occurrence() {
    let mut app = attached_app();
    app.player_tab.items[0].id = "x".into();
    app.player_tab.items[0].playlist_item_id = "entry-x1".into();
    app.player_tab.items[1].id = "x".into();
    app.player_tab.items[1].playlist_item_id = "entry-x2".into();
    app.player_tab.items.push(make_item("y", "Movie"));
    app.player_tab.items[2].id = "y".into();
    app.player_tab.sync_queue_model_from_items_if_needed();
    let slots = app.player_tab.queue.slots();
    app.remote_queue_projection = Some(projection(
        &app,
        &[
            (1, slots[0].slot_id),
            (2, slots[1].slot_id),
            (3, slots[2].slot_id),
        ],
    ));

    let mut tracking = ReconciliationTracker::new(
        "session",
        vec![
            SubmittedOccurrence::new(1, "x").playlist_entry("entry-x1"),
            SubmittedOccurrence::new(2, "x").playlist_entry("entry-x2"),
            SubmittedOccurrence::new(3, "y"),
        ],
        0,
        0,
    )
    .unwrap();
    tracking.observe(RemoteObservation::playing(1, "session", "x", 95, 100, 1));
    tracking.observe(RemoteObservation::playing(2, "session", "x", 1, 100, 2));
    assert!(tracking.mark_consumed(1));
    app.remote_tracker = Some(tracking);

    // Project the consume of the first "x": visible queue is now [x, y].
    app.player_tab.sync_queue_model_from_items_if_needed();
    let consumed_slot = app.player_tab.queue.slots()[0].slot_id;
    assert!(matches!(
        app.player_tab.queue.consume_slot(consumed_slot),
        mbv_core::playback_queue::QueueMutationResult::Applied(_)
    ));
    app.player_tab.sync_items_from_queue_model();
    assert_eq!(app.player_tab.items.len(), 2);
    assert_eq!(app.player_tab.items[0].id, "x");

    app.connected_session_state = Some({
        let mut state = make_session("Client", "Emby");
        state.id = "session".into();
        state.now_playing_item_id = Some("x".into());
        state
    });

    // Selecting the remaining duplicate targets occurrence 2, never the
    // consumed occurrence 1: duplicate media items keep distinct identities.
    app.player_tab.queue_cursor = 0;
    app.dispatch(crate::app::action::Command::QueuePlayCursor);
    assert_eq!(
        app.remote_tracker
            .as_ref()
            .unwrap()
            .expected()
            .unwrap()
            .intent,
        RemoteIntent::Select { target: 2 }
    );
    assert!(app
        .remote_tracker
        .as_ref()
        .unwrap()
        .consumed()
        .any(|id| id == 1));
}
