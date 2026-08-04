use super::*;
use crate::app::tests::{make_app_stub, make_item, make_session};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use mbv_core::playback_queue::QueueSlotId;
use mbv_core::remote_reconciliation::{
    ReconciliationTracker, RemoteIntent, RemoteObservation, SubmittedOccurrence, TrackingState,
};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn tracker(media: &[&str]) -> ReconciliationTracker {
    ReconciliationTracker::new(
        "session",
        media
            .iter()
            .enumerate()
            .map(|(index, media_id)| SubmittedOccurrence::new(index as u64 + 1, *media_id))
            .collect(),
        0,
        0,
    )
    .unwrap()
}

fn rendered_text(app: &mut App) -> String {
    let backend = TestBackend::new(160, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| app.render(frame)).unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol().to_string())
        .collect()
}

fn attached_app() -> App {
    let mut app = make_app_stub();
    app.connected_session_id = Some("session".into());
    app.connected_session_state = Some(make_session("Client", "Emby"));
    app.terminal_width = 160;
    app.player_tab.items = vec![make_item("a", "Movie"), make_item("b", "Movie")];
    app
}

#[test]
fn queue_title_renders_each_tracking_health_state() {
    let cases = [
        (TrackingState::Starting, "STARTING"),
        (TrackingState::Tracking, "TRACKING"),
        (TrackingState::Ambiguous, "AMBIGUOUS"),
        (TrackingState::Invalid, "INVALID"),
        (TrackingState::Suspended, "SUSPENDED"),
    ];

    for (state, label) in cases {
        let mut app = attached_app();
        let mut tracking = tracker(&["a", "b"]);
        match state {
            TrackingState::Starting => {}
            TrackingState::Tracking => {
                tracking.observe(RemoteObservation::playing(1, "session", "a", 1, 100, 1));
            }
            TrackingState::Ambiguous => {
                tracking = tracker(&["a", "a", "b"]);
                tracking.observe(RemoteObservation::playing(1, "session", "a", 80, 100, 1));
                tracking.observe(RemoteObservation::playing(2, "session", "a", 1, 100, 2));
            }
            TrackingState::Invalid => {
                tracking.observe(RemoteObservation::playing(1, "session", "a", 1, 100, 1));
                tracking.observe(RemoteObservation::playing(2, "session", "x", 1, 100, 2));
            }
            TrackingState::Suspended => {
                tracking.observe(RemoteObservation::playing(1, "session", "a", 1, 100, 1));
                tracking.session_disappeared();
            }
        }
        app.remote_tracker = Some(tracking);
        assert!(rendered_text(&mut app).contains(label), "missing {label}");
    }
}

#[test]
fn duplicate_reanchor_opens_picker_and_enter_selects_occurrence() {
    let mut app = attached_app();
    app.panel_focus = crate::app::PanelFocus::Queue;
    app.player_tab.items[0].id = "a".into();
    app.player_tab.items[1].id = "b".into();
    let mut tracking = tracker(&["a", "a", "b"]);
    tracking.observe(RemoteObservation::playing(1, "session", "a", 80, 100, 1));
    tracking.observe(RemoteObservation::playing(2, "session", "a", 1, 100, 2));
    app.remote_tracker = Some(tracking);

    app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    assert_eq!(app.remote_reanchor_popup.as_ref().unwrap().targets.len(), 2);

    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.remote_reanchor_popup.is_none());
    assert_eq!(
        app.remote_tracker.as_ref().unwrap().state(),
        TrackingState::Tracking
    );
    assert_eq!(
        app.remote_tracker.as_ref().unwrap().current_index(),
        Some(1)
    );
}

#[test]
fn reanchor_popup_blocks_mouse_dispatch() {
    let mut app = attached_app();
    app.player_tab.queue_cursor = 1;
    app.remote_reanchor_popup = Some(super::types_playback::RemoteReanchorPopup {
        targets: vec![(0, "a".into())],
        cursor: 0,
    });

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 2,
        row: 2,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(app.player_tab.queue_cursor, 1);
    assert!(app.remote_reanchor_popup.is_some());
}

#[test]
fn tracking_retirement_clears_reanchor_popup() {
    let mut app = attached_app();
    app.remote_tracker = Some(tracker(&["a", "b"]));
    app.remote_reanchor_popup = Some(super::types_playback::RemoteReanchorPopup {
        targets: vec![(0, "a".into())],
        cursor: 0,
    });
    app.retire_remote_tracking(false);
    assert!(app.remote_reanchor_popup.is_none());
}

#[test]
fn submitted_sequence_without_exact_visible_queue_has_no_projection() {
    let mut app = attached_app();
    app.player_tab.items[0].id = "visible-a".into();
    app.player_tab.items[1].id = "visible-b".into();
    app.player_tab.sync_queue_model_from_items_if_needed();
    let submitted = vec![
        make_item("submitted-a", "Movie"),
        make_item("submitted-b", "Movie"),
    ];
    app.submit_attached_sequence("session", &submitted, 0);
    assert!(app.remote_tracker.is_some());
    assert!(app.remote_queue_projection.is_none());
}

#[test]
fn stop_tracking_and_queue_edits_are_input_gated() {
    let mut app = attached_app();
    app.panel_focus = crate::app::PanelFocus::Queue;
    app.remote_tracker = Some(tracker(&["a", "b"]));
    app.home.continue_items = vec![make_item("a", "Movie")];

    app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));
    assert!(app.remote_tracker.is_none());

    // Enqueue is an ordinary queue edit from Home focus: it applies without
    // a tracking-specific confirmation and retires tracking.
    app.remote_tracker = Some(tracker(&["a", "b"]));
    app.panel_focus = crate::app::PanelFocus::Library;
    app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
    assert!(app.confirm_modal.is_none());
    assert_eq!(app.player_tab.items.len(), 3);
    assert!(app.remote_tracker.is_none());
}

#[test]
fn unresolved_count_is_passive_in_queue_title() {
    let mut app = attached_app();
    app.remote_tracker = Some(tracker(&["a", "b"]));
    app.remote_unresolved_outcomes = 2;
    let text = rendered_text(&mut app);
    assert!(
        text.contains("· !"),
        "rendered queue did not contain unresolved indicator: {text:?}"
    );
}

#[test]
fn replacement_tracker_ignores_an_earlier_in_flight_poll_but_applies_item_change_cursor() {
    let mut app = attached_app();
    app.player_tab.items[0].id = "a".into();
    app.player_tab.items[1].id = "b".into();
    app.player_tab.queue_cursor = 0;
    app.session_poll_generation = 4;
    let items = app.player_tab.items.clone();
    app.remote_tracker = App::build_remote_tracker_with_source("session", &items, 1, 5, None);

    let mut stale_session = make_session("Client", "Emby");
    stale_session.id = "session".into();
    stale_session.now_playing_item_id = Some("a".into());
    stale_session.position_ticks = 1;
    stale_session.runtime_ticks = 100;
    app.handle_session_event(SessionEvent::Loaded {
        sessions: vec![stale_session],
        generation: 4,
    });

    assert_eq!(
        app.remote_tracker.as_ref().unwrap().state(),
        TrackingState::Starting
    );

    let mut submitted_session = make_session("Client", "Emby");
    submitted_session.id = "session".into();
    submitted_session.now_playing_item_id = Some("b".into());
    submitted_session.position_ticks = 1;
    submitted_session.runtime_ticks = 100;
    app.handle_session_event(SessionEvent::Loaded {
        sessions: vec![submitted_session],
        generation: 5,
    });

    assert_eq!(
        app.remote_tracker.as_ref().unwrap().state(),
        TrackingState::Tracking
    );
    assert_eq!(app.player_tab.queue_cursor, 1);
}

#[test]
fn repeated_same_item_poll_does_not_move_queue_cursor() {
    let mut app = attached_app();
    app.player_tab.items[0].id = "a".into();
    app.player_tab.items[1].id = "b".into();
    app.player_tab.queue_cursor = 0;
    app.connected_session_state = Some({
        let mut state = make_session("Client", "Emby");
        state.id = "session".into();
        state.now_playing_item_id = Some("a".into());
        state
    });

    let mut repeated = app.connected_session_state.clone().unwrap();
    repeated.now_playing_item_id = Some("b".into());
    app.handle_session_event(SessionEvent::Loaded {
        sessions: vec![repeated.clone()],
        generation: 1,
    });
    assert_eq!(app.player_tab.queue_cursor, 1);

    app.player_tab.queue_cursor = 0;
    app.handle_session_event(SessionEvent::Loaded {
        sessions: vec![repeated],
        generation: 2,
    });
    assert_eq!(app.player_tab.queue_cursor, 0);
}

#[test]
fn failed_remote_volume_command_does_not_invalidate_tracking() {
    let mut app = attached_app();
    app.remote_tracker = Some(tracker(&["a", "b"]));

    app.handle_session_event(SessionEvent::CommandError {
        error: "volume failed".into(),
        reconciliation: None,
    });

    assert!(app.remote_tracker.as_ref().unwrap().is_active());
    assert_eq!(
        app.remote_tracker.as_ref().unwrap().state(),
        TrackingState::Starting
    );
}

#[test]
fn stale_reconciliation_command_failure_does_not_invalidate_replacement_tracker() {
    let mut app = attached_app();
    let mut original = tracker(&["a", "b"]);
    original.track_command_generation(4);
    app.remote_tracker = Some(original);

    let mut replacement = tracker(&["a", "b"]);
    replacement.track_command_generation(5);
    app.remote_tracker = Some(replacement);

    app.handle_session_event(SessionEvent::CommandError {
        error: "old command failed".into(),
        reconciliation: Some(ReconciliationCommand {
            session_id: "session".into(),
            tracking_id: 0,
            tracker_epoch: 0,
            generation: 4,
        }),
    });

    assert!(app.remote_tracker.as_ref().unwrap().is_active());
    assert_eq!(
        app.remote_tracker.as_ref().unwrap().state(),
        TrackingState::Starting
    );
}

#[test]
fn stale_consume_validation_failure_does_not_increment_new_tracker() {
    let mut app = attached_app();
    app.remote_tracker = Some(tracker(&["new-a", "new-b"]));
    app.remote_consume_operations
        .push(super::types_playback::RemoteConsumeOperation {
            operation_id: 7,
            mutation_id: 7,
            session_id: "old-session".into(),
            tracking_id: 0,
            epoch: 2,
            occurrence_id: 41,
            playlist_id: "old-playlist".into(),
            entry_id: "old-entry".into(),
            media_id: "old-a".into(),
            queue_slot_id: None,
            queue_lineage: app.remote_queue_lineage,
        });

    app.handle_session_event(SessionEvent::ConsumeValidated {
        mutation_id: 7,
        operation_id: 7,
        tracking_id: 0,
        session_id: "old-session".into(),
        epoch: 2,
        occurrence_id: 41,
        playlist_id: "old-playlist".into(),
        entry_id: "old-entry".into(),
        media_id: "old-a".into(),
        result: Err("stale validation".into()),
    });

    assert_eq!(app.remote_unresolved_outcomes, 0);
    assert!(app.remote_consume_operations.is_empty());
}

#[test]
fn successful_remote_consume_removes_exact_occurrence_from_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = attached_app();
    app.player_tab.items[0].id = "a".into();
    app.player_tab.items[1].id = "b".into();
    app.player_tab.items[0].id = "a".into();
    app.player_tab.items[0].playlist_item_id = "entry-a".into();
    app.player_tab.items[1].id = "b".into();
    app.player_tab.items[1].playlist_item_id = "entry-b".into();
    app.player_tab.queue_cursor = 1;
    app.player_tab.sync_queue_model_from_items_if_needed();

    let mut tracking = ReconciliationTracker::new(
        "session",
        vec![
            SubmittedOccurrence::new(1, "a").playlist_entry("entry-a"),
            SubmittedOccurrence::new(2, "b").playlist_entry("entry-b"),
        ],
        0,
        0,
    )
    .unwrap();
    tracking.observe(RemoteObservation::playing(1, "session", "a", 95, 100, 1));
    tracking.observe(RemoteObservation::playing(2, "session", "b", 1, 100, 2));
    assert!(tracking.mark_consumed(1));
    let tracking_id = tracking.tracking_id();
    app.remote_tracker = Some(tracking);
    app.remote_consume_operations
        .push(super::types_playback::RemoteConsumeOperation {
            operation_id: 1,
            mutation_id: 1,
            session_id: "session".into(),
            tracking_id,
            epoch: 0,
            occurrence_id: 1,
            playlist_id: "playlist-1".into(),
            entry_id: "entry-a".into(),
            media_id: "a".into(),
            queue_slot_id: Some(app.player_tab.queue.slots()[0].slot_id),
            queue_lineage: app.remote_queue_lineage,
        });

    app.handle_session_event(SessionEvent::ConsumeOutcome {
        mutation_id: 1,
        operation_id: 1,
        tracking_id,
        session_id: "session".into(),
        epoch: 0,
        occurrence_id: 1,
        playlist_id: "playlist-1".into(),
        entry_id: "entry-a".into(),
        media_id: "a".into(),
        result: Ok(()),
    });

    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|item| item.playlist_item_id.as_str())
            .collect::<Vec<_>>(),
        vec!["entry-b"]
    );
    assert_eq!(app.player_tab.queue_cursor, 0);
    assert_eq!(app.remote_tracker.as_ref().unwrap().submitted().len(), 2);
    let persisted = crate::config::load_queue_state().expect("projected queue persisted");
    assert_eq!(persisted.items.len(), 1);
    assert_eq!(persisted.items[0].playlist_item_id, "entry-b");
}

#[test]
fn stale_consume_cannot_remove_a_replacement_slot() {
    let mut app = attached_app();
    app.player_tab.sync_queue_model_from_items_if_needed();
    let old_slot = app.player_tab.queue.slots()[0].slot_id;
    app.remote_consume_operations
        .push(super::types_playback::RemoteConsumeOperation {
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
        .push(super::types_playback::RemoteConsumeOperation {
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
        .push(super::types_playback::RemoteConsumeOperation {
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
    let base =
        super::session_command_actions::remote_jump_target(&app.player_tab.items, Some("a"), 1);
    assert_eq!(base, Some((1, 20)));

    // Tracking presence never changes the destination or payload.
    app.remote_tracker = Some(tracker(&["a", "a", "b"]));
    assert_eq!(
        super::session_command_actions::remote_jump_target(&app.player_tab.items, Some("a"), 1),
        base
    );
    assert_eq!(
        super::session_command_actions::remote_jump_target(&app.player_tab.items, Some("b"), -1),
        Some((1, 20))
    );

    // After projected prefix consumption the visible queue shrinks; the
    // destination is still chosen from the visible queue exactly as untracked.
    let items: Vec<_> = app.player_tab.items.iter().skip(1).cloned().collect();
    assert_eq!(
        super::session_command_actions::remote_jump_target(&items, Some("a"), 1),
        Some((1, 30))
    );
}

fn projection(
    app: &App,
    occurrences: &[(u64, QueueSlotId)],
) -> super::types_playback::RemoteQueueProjection {
    let occurrence_slots: std::collections::HashMap<_, _> = occurrences.iter().copied().collect();
    let slot_occurrences = occurrence_slots
        .iter()
        .map(|(occurrence_id, slot_id)| (*slot_id, *occurrence_id))
        .collect();
    super::types_playback::RemoteQueueProjection {
        session_id: "session".into(),
        epoch: 0,
        queue_lineage: app.remote_queue_lineage,
        occurrence_slots,
        slot_occurrences,
    }
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
    app.dispatch(super::action::Command::QueuePlayCursor);
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
    app.dispatch(super::action::Command::QueuePlayCursor);
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

// ── task 1.3: tracked vs untracked command payload parity ────────────────

fn remote_command_app(listener: &std::net::TcpListener) -> App {
    let url = format!("http://{}", listener.local_addr().unwrap());
    let mut app = attached_app();
    app.client.lock().unwrap().config.server_url = url;
    app.player_tab.items[0].id = "a".into();
    app.player_tab.items[1].id = "b".into();
    app.player_tab.items.push(make_item("c", "Movie"));
    app.player_tab.items[2].id = "c".into();
    app.player_tab.items[0].playback_position_ticks = 100;
    app.player_tab.items[1].playback_position_ticks = 200;
    app.player_tab.items[2].playback_position_ticks = 300;
    app.player_tab.sync_queue_model_from_items_if_needed();
    let mut s = make_session("Client", "Emby");
    s.id = "session".into();
    s.now_playing_item_id = Some("a".into());
    s.position_s = 60;
    s.runtime_s = 300;
    s.position_ticks = 60 * mbv_core::api::TICKS_PER_SECOND;
    s.runtime_ticks = 300 * mbv_core::api::TICKS_PER_SECOND;
    app.connected_session_state = Some(s);
    app
}

fn attach_tracking_to(app: &mut App) {
    let slots = app.player_tab.queue.slots();
    app.remote_tracker = Some(tracker(&["a", "b", "c"]));
    app.remote_queue_projection = Some(projection(
        app,
        &[
            (1, slots[0].slot_id),
            (2, slots[1].slot_id),
            (3, slots[2].slot_id),
        ],
    ));
}

fn accept_one(
    listener: &std::net::TcpListener,
    timeout: std::time::Duration,
) -> Option<std::net::TcpStream> {
    listener.set_nonblocking(true).unwrap();
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                listener.set_nonblocking(false).unwrap();
                return Some(stream);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if std::time::Instant::now() > deadline {
                    listener.set_nonblocking(false).unwrap();
                    return None;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err(e) => {
                listener.set_nonblocking(false).unwrap();
                panic!("listener accept failed: {e}");
            }
        }
    }
}

fn read_http_request(stream: &std::net::TcpStream) -> String {
    use std::io::{BufRead, BufReader, Read};
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut head = String::new();
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        if let Some(len) = line
            .strip_prefix("Content-Length:")
            .or_else(|| line.strip_prefix("content-length:"))
            .and_then(|rest| rest.trim().parse::<usize>().ok())
        {
            content_length = len;
        }
        let trimmed = line.trim_end();
        head.push_str(&line);
        if trimmed.is_empty() {
            break;
        }
    }
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body).unwrap();
    }
    format!("{head}{}", String::from_utf8_lossy(&body))
}

fn respond_http(stream: std::net::TcpStream, status: u16, body: &str) {
    use std::io::Write;
    let mut writer = stream.try_clone().unwrap();
    let _ = writer.write_all(
        format!(
            "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .as_bytes(),
    );
}

/// Runs `act` (which dispatches a remote command on a background thread),
/// captures the command HTTP request that reaches `listener`, responds, and
/// drains the dispatch thread's follow-up `GET /Sessions` poll so the thread
/// completes instead of leaking.
fn capture_remote_command(
    listener: &std::net::TcpListener,
    app: &mut App,
    act: impl FnOnce(&mut App),
) -> String {
    act(app);
    let stream = accept_one(listener, std::time::Duration::from_secs(10))
        .expect("dispatch thread must send the command request");
    let request = read_http_request(&stream);
    respond_http(stream, 200, "");
    if let Some(second) = accept_one(listener, std::time::Duration::from_secs(10)) {
        let _ = read_http_request(&second);
        respond_http(second, 200, "[]");
    }
    request
}

#[test]
fn next_command_payload_is_identical_with_and_without_tracking() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut untracked = remote_command_app(&listener);
    let untracked_req = capture_remote_command(&listener, &mut untracked, |app| {
        app.dispatch(super::action::Command::NextTrack);
    });

    let mut tracked = remote_command_app(&listener);
    attach_tracking_to(&mut tracked);
    let tracked_req = capture_remote_command(&listener, &mut tracked, |app| {
        app.dispatch(super::action::Command::NextTrack);
    });

    assert_eq!(tracked_req, untracked_req);
    assert_eq!(
        tracked
            .remote_tracker
            .as_ref()
            .unwrap()
            .expected()
            .unwrap()
            .intent,
        RemoteIntent::Next { target: 2 },
        "tracking records the occurrence at the untracked destination"
    );
}

#[test]
fn previous_command_payload_is_identical_with_and_without_tracking() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let now_playing_b = |app: &mut App| {
        if let Some(s) = app.connected_session_state.as_mut() {
            s.now_playing_item_id = Some("b".into());
        }
    };
    let mut untracked = remote_command_app(&listener);
    now_playing_b(&mut untracked);
    let untracked_req = capture_remote_command(&listener, &mut untracked, |app| {
        app.dispatch(super::action::Command::PreviousTrack);
    });

    let mut tracked = remote_command_app(&listener);
    now_playing_b(&mut tracked);
    attach_tracking_to(&mut tracked);
    let tracked_req = capture_remote_command(&listener, &mut tracked, |app| {
        app.dispatch(super::action::Command::PreviousTrack);
    });

    assert_eq!(tracked_req, untracked_req);
    assert_eq!(
        tracked
            .remote_tracker
            .as_ref()
            .unwrap()
            .expected()
            .unwrap()
            .intent,
        RemoteIntent::Previous { target: 1 }
    );
}

#[test]
fn direct_selection_payload_is_identical_with_and_without_tracking() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut untracked = remote_command_app(&listener);
    untracked.player_tab.queue_cursor = 1;
    let untracked_req = capture_remote_command(&listener, &mut untracked, |app| {
        app.dispatch(super::action::Command::QueuePlayCursor);
    });

    let mut tracked = remote_command_app(&listener);
    tracked.player_tab.queue_cursor = 1;
    attach_tracking_to(&mut tracked);
    let tracked_req = capture_remote_command(&listener, &mut tracked, |app| {
        app.dispatch(super::action::Command::QueuePlayCursor);
    });

    assert_eq!(tracked_req, untracked_req);
    assert_eq!(
        tracked
            .remote_tracker
            .as_ref()
            .unwrap()
            .expected()
            .unwrap()
            .intent,
        RemoteIntent::Select { target: 2 }
    );
}

#[test]
fn restart_current_payload_is_identical_with_and_without_tracking() {
    // Restart maps to re-activating the now-playing item from the queue:
    // the untracked path re-submits the visible sequence at that index while
    // tracking issues a Select intent for the current occurrence — both send
    // the identical session_play_items payload.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut untracked = remote_command_app(&listener);
    untracked.player_tab.queue_cursor = 0;
    let untracked_req = capture_remote_command(&listener, &mut untracked, |app| {
        app.dispatch(super::action::Command::QueuePlayCursor);
    });

    let mut tracked = remote_command_app(&listener);
    tracked.player_tab.queue_cursor = 0;
    attach_tracking_to(&mut tracked);
    let tracked_req = capture_remote_command(&listener, &mut tracked, |app| {
        app.dispatch(super::action::Command::QueuePlayCursor);
    });

    assert_eq!(tracked_req, untracked_req);
    assert_eq!(
        tracked
            .remote_tracker
            .as_ref()
            .unwrap()
            .expected()
            .unwrap()
            .intent,
        RemoteIntent::Select { target: 1 }
    );
}

#[test]
fn seek_payload_is_identical_with_and_without_tracking() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut untracked = remote_command_app(&listener);
    let untracked_req = capture_remote_command(&listener, &mut untracked, |app| {
        app.dispatch(super::action::Command::SeekRelative(5.0));
    });

    let mut tracked = remote_command_app(&listener);
    attach_tracking_to(&mut tracked);
    let tracked_req = capture_remote_command(&listener, &mut tracked, |app| {
        app.dispatch(super::action::Command::SeekRelative(5.0));
    });

    assert_eq!(tracked_req, untracked_req);
    assert_eq!(
        tracked
            .remote_tracker
            .as_ref()
            .unwrap()
            .expected()
            .unwrap()
            .intent,
        RemoteIntent::Seek
    );
}

#[test]
fn play_pause_payload_is_identical_with_and_without_tracking() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut untracked = remote_command_app(&listener);
    let untracked_req = capture_remote_command(&listener, &mut untracked, |app| {
        app.dispatch(super::action::Command::TogglePlayPause);
    });

    let mut tracked = remote_command_app(&listener);
    attach_tracking_to(&mut tracked);
    let tracked_req = capture_remote_command(&listener, &mut tracked, |app| {
        app.dispatch(super::action::Command::TogglePlayPause);
    });

    assert_eq!(tracked_req, untracked_req);
}

#[test]
fn stop_payload_is_identical_with_and_without_tracking() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut untracked = remote_command_app(&listener);
    let untracked_req = capture_remote_command(&listener, &mut untracked, |app| {
        app.dispatch(super::action::Command::Stop);
    });

    let mut tracked = remote_command_app(&listener);
    attach_tracking_to(&mut tracked);
    let tracked_req = capture_remote_command(&listener, &mut tracked, |app| {
        app.dispatch(super::action::Command::Stop);
    });

    assert_eq!(tracked_req, untracked_req);
    assert_eq!(
        tracked
            .remote_tracker
            .as_ref()
            .unwrap()
            .expected()
            .unwrap()
            .intent,
        RemoteIntent::Stop
    );
}

#[test]
fn single_item_replacement_payload_is_identical_with_and_without_tracking() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut untracked = remote_command_app(&listener);
    let item = untracked.player_tab.items[0].clone();
    let untracked_req = capture_remote_command(&listener, &mut untracked, |app| {
        app.play_item(item.clone());
    });

    let mut tracked = remote_command_app(&listener);
    attach_tracking_to(&mut tracked);
    let tracked_req = capture_remote_command(&listener, &mut tracked, |app| {
        app.play_item(item.clone());
    });

    assert_eq!(tracked_req, untracked_req);
    assert!(
        tracked.remote_tracker.is_none(),
        "a single-item replacement retires tracking as a target replacement"
    );
}

// ── task 7.2: SUSPENDED retention vs the three-miss policy ────────────────

#[test]
fn temporary_poll_gap_keeps_tracking_and_third_miss_retires_with_attachment() {
    let mut app = attached_app();
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
    app.remote_tracker = Some(tracking);

    // Miss 1 and 2: the logical attachment is still held, so tracking is
    // retained as SUSPENDED and may resume observing a returning session.
    app.handle_session_event(SessionEvent::Loaded {
        sessions: vec![],
        generation: 2,
    });
    assert!(app.remote_tracker.as_ref().unwrap().is_active());
    assert_eq!(
        app.remote_tracker.as_ref().unwrap().state(),
        TrackingState::Suspended,
        "temporary unavailability while attached must retain SUSPENDED"
    );
    assert_eq!(app.connected_session_id.as_deref(), Some("session"));
    assert_eq!(app.session_miss_count, 1);

    app.handle_session_event(SessionEvent::Loaded {
        sessions: vec![],
        generation: 3,
    });
    assert!(app.remote_tracker.as_ref().unwrap().is_active());
    assert_eq!(
        app.remote_tracker.as_ref().unwrap().state(),
        TrackingState::Suspended
    );
    assert_eq!(app.session_miss_count, 2);

    // Miss 3: the three-miss policy clears the attachment and retires
    // tracking in the same transition; no hidden tracker survives.
    app.handle_session_event(SessionEvent::Loaded {
        sessions: vec![],
        generation: 4,
    });
    assert!(app.remote_tracker.is_none());
    assert!(app.connected_session_id.is_none());
    assert!(app.connected_session_state.is_none());
    assert_eq!(app.session_miss_count, 0);
}

#[test]
fn suspended_tracker_resumes_when_session_returns_before_third_miss() {
    let mut app = attached_app();
    app.connected_session_state = Some({
        let mut state = make_session("Client", "Emby");
        state.id = "session".into();
        state.now_playing_item_id = Some("b".into());
        state.position_ticks = 1;
        state.runtime_ticks = 100;
        state
    });
    let mut tracking = tracker(&["a", "b"]);
    tracking.observe(RemoteObservation::playing(1, "session", "a", 1, 100, 1));
    tracking.observe(RemoteObservation::playing(2, "session", "b", 1, 100, 2));
    app.remote_tracker = Some(tracking);

    // One poll gap suspends tracking while the logical attachment is held.
    app.handle_session_event(SessionEvent::Loaded {
        sessions: vec![],
        generation: 2,
    });
    assert_eq!(
        app.remote_tracker.as_ref().unwrap().state(),
        TrackingState::Suspended
    );

    // The session returns before the three-miss policy fires: tracking
    // resumes against the same immutable Submitted sequence.
    let mut returned = make_session("Client", "Emby");
    returned.id = "session".into();
    returned.now_playing_item_id = Some("b".into());
    returned.position_ticks = 1;
    returned.runtime_ticks = 100;
    app.handle_session_event(SessionEvent::Loaded {
        sessions: vec![returned],
        generation: 3,
    });

    assert!(app.remote_tracker.as_ref().unwrap().is_active());
    assert_eq!(
        app.remote_tracker.as_ref().unwrap().state(),
        TrackingState::Tracking
    );
    assert_eq!(
        app.remote_tracker
            .as_ref()
            .unwrap()
            .current_occurrence()
            .unwrap()
            .occurrence_id,
        2
    );
}

#[test]
fn returning_session_after_attachment_clear_leaves_no_hidden_tracker() {
    let mut app = attached_app();
    app.connected_session_state = Some({
        let mut state = make_session("Client", "Emby");
        state.id = "session".into();
        state.now_playing_item_id = Some("a".into());
        state.position_ticks = 1;
        state.runtime_ticks = 100;
        state
    });
    app.remote_tracker = Some(tracker(&["a", "b"]));

    for generation in 1..=3 {
        app.handle_session_event(SessionEvent::Loaded {
            sessions: vec![],
            generation,
        });
    }
    assert!(app.remote_tracker.is_none());
    assert!(app.connected_session_id.is_none());

    // The session returns to the poll after the attachment was cleared: no
    // hidden tracker is resurrected and no session observation re-attaches.
    let mut returned = make_session("Client", "Emby");
    returned.id = "session".into();
    returned.now_playing_item_id = Some("a".into());
    returned.position_ticks = 1;
    returned.runtime_ticks = 100;
    app.handle_session_event(SessionEvent::Loaded {
        sessions: vec![returned],
        generation: 4,
    });
    assert!(app.remote_tracker.is_none());
    assert!(app.sessions_rx.try_recv().is_err());
}

// ── task 7.3: unresolved presentation ownership ───────────────────────────

#[test]
fn stale_consume_error_after_reanchor_epoch_does_not_increment_unresolved() {
    let mut app = attached_app();
    let mut tracking = tracker(&["a", "b"]);
    // Drive the tracker to Invalid (material reset: position dropped by more
    // than the 3-second jitter with no duplicate successor) so re-anchor has
    // a single recoverable target and advances the epoch.
    let ticks = mbv_core::api::TICKS_PER_SECOND;
    tracking.observe(RemoteObservation::playing(
        1,
        "session",
        "a",
        100 * ticks,
        200 * ticks,
        1,
    ));
    tracking.observe(RemoteObservation::playing(
        2,
        "session",
        "a",
        10 * ticks,
        200 * ticks,
        2,
    ));
    assert_eq!(tracking.state(), TrackingState::Invalid);
    let tracking_id = tracking.tracking_id();
    let epoch_before = tracking.epoch();
    app.remote_tracker = Some(tracking);
    app.remote_consume_operations
        .push(super::types_playback::RemoteConsumeOperation {
            operation_id: 30,
            mutation_id: 30,
            session_id: "session".into(),
            tracking_id,
            epoch: epoch_before,
            occurrence_id: 1,
            playlist_id: "playlist".into(),
            entry_id: "entry".into(),
            media_id: "a".into(),
            queue_slot_id: None,
            queue_lineage: app.remote_queue_lineage,
        });

    app.reanchor_remote_tracking();
    let epoch_after = app.remote_tracker.as_ref().unwrap().epoch();
    assert!(
        epoch_after > epoch_before,
        "re-anchor must advance the tracker epoch"
    );

    app.handle_session_event(SessionEvent::ConsumeValidated {
        mutation_id: 30,
        operation_id: 30,
        tracking_id,
        session_id: "session".into(),
        epoch: epoch_before,
        occurrence_id: 1,
        playlist_id: "playlist".into(),
        entry_id: "entry".into(),
        media_id: "a".into(),
        result: Err("stale after re-anchor".into()),
    });

    assert_eq!(
        app.remote_unresolved_outcomes, 0,
        "a stale error from the pre-re-anchor epoch must not increment the current target"
    );
}

#[test]
fn disconnect_clears_unresolved_presentation_and_retires_tracking() {
    let mut app = attached_app();
    app.remote_tracker = Some(tracker(&["a", "b"]));
    app.remote_unresolved_outcomes = 2;

    app.disconnect_remote();

    assert!(app.remote_tracker.is_none());
    assert_eq!(app.remote_unresolved_outcomes, 0);
    assert!(app.connected_session_id.is_none());
}

#[test]
fn target_replacement_clears_unresolved_presentation() {
    let mut app = attached_app();
    app.remote_tracker = Some(tracker(&["a", "b"]));
    app.remote_unresolved_outcomes = 3;

    let mut other = make_session("Other", "Emby");
    other.id = "other-session".into();
    app.connect_to_session(&other);

    assert!(app.remote_tracker.is_none());
    assert_eq!(app.remote_unresolved_outcomes, 0);
    assert_eq!(app.connected_session_id.as_deref(), Some("other-session"));
}

// ── task 7.4: stale outcomes cannot affect the current target ─────────────

#[test]
fn stale_validation_failure_after_queue_edit_does_not_increment_unresolved() {
    let mut app = attached_app();
    app.player_tab.items[0].playlist_item_id = "entry-a".into();
    app.player_tab.items[1].playlist_item_id = "entry-b".into();
    app.player_tab.sync_queue_model_from_items_if_needed();
    app.remote_tracker = Some(tracker(&["a", "b"]));
    app.remote_consume_operations
        .push(super::types_playback::RemoteConsumeOperation {
            operation_id: 40,
            mutation_id: 40,
            session_id: "session".into(),
            tracking_id: 0,
            epoch: 0,
            occurrence_id: 1,
            playlist_id: "playlist".into(),
            entry_id: "entry-a".into(),
            media_id: "a".into(),
            queue_slot_id: None,
            queue_lineage: app.remote_queue_lineage,
        });

    // A successful queue edit retires tracking; the tracker is gone, so a
    // late validation failure from the retired session cannot be attributed
    // to any current target.
    app.remove_from_queue(0);
    assert!(app.remote_tracker.is_none());

    app.handle_session_event(SessionEvent::ConsumeValidated {
        mutation_id: 40,
        operation_id: 40,
        tracking_id: 0,
        session_id: "session".into(),
        epoch: 0,
        occurrence_id: 1,
        playlist_id: "playlist".into(),
        entry_id: "entry-a".into(),
        media_id: "a".into(),
        result: Err("stale after edit".into()),
    });

    assert_eq!(app.remote_unresolved_outcomes, 0);
    assert!(app.remote_consume_operations.is_empty());
}

#[test]
fn stale_deletion_after_target_replacement_cannot_remove_current_slot() {
    let mut app = attached_app();
    app.player_tab.sync_queue_model_from_items_if_needed();
    let old_slot = app.player_tab.queue.slots()[0].slot_id;
    app.remote_consume_operations
        .push(super::types_playback::RemoteConsumeOperation {
            operation_id: 41,
            mutation_id: 41,
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

    // Attach to another session: this advances the visible-queue lineage and
    // retires the old tracker, so the old operation's projection is invalid.
    let mut other = make_session("Other", "Emby");
    other.id = "other".into();
    app.connect_to_session(&other);
    assert!(app.remote_queue_lineage > 0 || app.remote_tracker.is_none());

    app.handle_session_event(SessionEvent::ConsumeOutcome {
        mutation_id: 41,
        operation_id: 41,
        tracking_id: 0,
        session_id: "session".into(),
        epoch: 0,
        occurrence_id: 1,
        playlist_id: "playlist".into(),
        entry_id: "entry".into(),
        media_id: "a".into(),
        result: Ok(()),
    });

    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["id", "id"],
        "a stale deletion from the replaced target must not remove a current slot"
    );
}

// ── task 7.5: stopped-but-present stays tracked ───────────────────────────

#[test]
fn stopped_but_present_session_keeps_tracking_idle() {
    let mut app = attached_app();
    app.connected_session_state = Some({
        let mut state = make_session("Client", "Emby");
        state.id = "session".into();
        state.now_playing_item_id = Some("a".into());
        state.position_ticks = 50;
        state.runtime_ticks = 100;
        state
    });
    let mut tracking = tracker(&["a", "b"]);
    tracking.observe(RemoteObservation::playing(1, "session", "a", 50, 100, 1));
    app.remote_tracker = Some(tracking);

    // The remote reports stopped (no now-playing item) while mbv remains
    // attached: tracking stays active and idle at the resolved occurrence.
    let mut stopped = make_session("Client", "Emby");
    stopped.id = "session".into();
    stopped.now_playing_item_id = None;
    app.handle_session_event(SessionEvent::Loaded {
        sessions: vec![stopped],
        generation: 2,
    });

    assert!(app.remote_tracker.as_ref().unwrap().is_active());
    assert_eq!(
        app.remote_tracker.as_ref().unwrap().state(),
        TrackingState::Tracking
    );
    assert_eq!(app.connected_session_id.as_deref(), Some("session"));
}

// ── task 8.2: tracking controls are explicit queue-context routes ─────────

#[test]
fn tracking_controls_fire_only_in_queue_context() {
    let mut app = attached_app();
    app.panel_focus = crate::app::PanelFocus::Queue;
    let ticks = mbv_core::api::TICKS_PER_SECOND;
    let mut tracking = tracker(&["a", "b"]);
    tracking.observe(RemoteObservation::playing(
        1,
        "session",
        "a",
        100 * ticks,
        200 * ticks,
        1,
    ));
    tracking.observe(RemoteObservation::playing(
        2,
        "session",
        "a",
        10 * ticks,
        200 * ticks,
        2,
    ));
    assert_eq!(tracking.state(), TrackingState::Invalid);
    app.remote_tracker = Some(tracking);

    app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    assert_eq!(
        app.remote_tracker.as_ref().unwrap().state(),
        TrackingState::Tracking,
        "Ctrl+R in queue context re-anchors from Invalid"
    );

    app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));
    assert!(app.remote_tracker.is_none());
}

#[test]
fn library_focus_rescan_does_not_trigger_tracking_controls() {
    let mut app = attached_app();
    app.panel_focus = crate::app::PanelFocus::Library;
    app.library_tab = 1;
    let mut lib_item = make_item("Movies", "CollectionFolder");
    lib_item.id = "lib-movies".into();
    app.libs.push(LibraryTab {
        library: lib_item,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    app.remote_tracker = Some(tracker(&["a", "b"]));

    // Ctrl+R in library focus is the established library rescan, not the
    // queue-context re-anchor.
    app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    assert!(matches!(
        app.confirm_modal.as_ref().map(|m| &m.on_confirm),
        Some(ConfirmAction::RescanLibrary(_))
    ));
    assert!(app.remote_reanchor_popup.is_none());
    assert!(app.remote_tracker.is_some());

    // Ctrl+T in library focus is swallowed by the library context; tracking
    // must not be stopped.
    app.confirm_modal = None;
    app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));
    assert!(app.remote_tracker.is_some());
}

#[test]
fn help_documents_tracking_controls_in_queue_context() {
    let mut app = attached_app();
    app.show_help = true;
    app.panel_focus = crate::app::PanelFocus::Queue;
    app.remote_tracker = Some(tracker(&["a", "b"]));
    let text = rendered_text(&mut app);
    assert!(
        text.contains("Re-anchor tracking"),
        "help must document re-anchor"
    );
    assert!(
        text.contains("Stop remote tracking"),
        "help must document Stop Tracking"
    );
}
