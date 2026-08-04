use super::*;
use crate::app::tests::{make_app_stub, make_item, make_session};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use mbv_core::playback_queue::QueueSlotId;
use mbv_core::remote_reconciliation::{
    ReconciliationTracker, RemoteObservation, SubmittedOccurrence, TrackingState,
};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[cfg(test)]
#[path = "tests_remote_reconciliation_consume.rs"]
mod tests_remote_reconciliation_consume;

#[cfg(test)]
#[path = "tests_remote_reconciliation_commands.rs"]
mod tests_remote_reconciliation_commands;

#[cfg(test)]
#[path = "tests_remote_reconciliation_lifecycle.rs"]
mod tests_remote_reconciliation_lifecycle;

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
fn replacement_tracker_ignores_an_earlier_in_flight_poll() {
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

/// The point of remote tracking: an item the remote client finishes leaves
/// the queue. Consume is queue removal only, so this must work for an
/// ordinary ad-hoc queue with no saved playlist behind it.
#[test]
fn completed_remote_occurrence_is_consumed_from_an_ad_hoc_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = attached_app();
    app.client.lock().unwrap().config.consume_videos = true;
    app.player_tab.items[0].id = "a".into();
    app.player_tab.items[1].id = "b".into();
    app.player_tab.queue_cursor = 0;
    app.session_poll_generation = 5;
    let items = app.player_tab.items.clone();
    app.remote_tracker = App::build_remote_tracker_with_source("session", &items, 0, 5, None);
    app.player_tab.sync_queue_model_from_items_if_needed();
    let slots: Vec<QueueSlotId> = app
        .player_tab
        .queue
        .slots()
        .iter()
        .map(|slot| slot.slot_id)
        .collect();
    app.remote_queue_projection = Some(projection(&app, &[(1, slots[0]), (2, slots[1])]));

    let poll = |media: &str, position: i64| {
        let mut session = make_session("Client", "Emby");
        session.id = "session".into();
        session.now_playing_item_id = Some(media.into());
        session.position_ticks = position;
        session.runtime_ticks = 100;
        session
    };
    app.handle_session_event(SessionEvent::Loaded {
        sessions: vec![poll("a", 1)],
        generation: 5,
    });
    app.handle_session_event(SessionEvent::Loaded {
        sessions: vec![poll("a", 99)],
        generation: 6,
    });
    app.handle_session_event(SessionEvent::Loaded {
        sessions: vec![poll("b", 1)],
        generation: 7,
    });

    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["b"],
        "the finished item is consumed from the queue"
    );
    assert_eq!(app.player_tab.queue_cursor, 0);
    assert_eq!(
        app.remote_tracker.as_ref().unwrap().submitted().len(),
        2,
        "the immutable submitted sequence is preserved"
    );
    let persisted = crate::config::load_queue_state().expect("projected queue persisted");
    assert_eq!(persisted.items.len(), 1);
    assert_eq!(persisted.items[0].id, "b");
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
