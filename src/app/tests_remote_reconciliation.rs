use super::*;
use crate::app::tests::{make_app_stub, make_item, make_session};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::remote_reconciliation::{
    ReconciliationTracker, RemoteObservation, SubmittedOccurrence, TrackingState,
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
fn stop_tracking_and_edit_confirmation_are_input_gated() {
    let mut app = attached_app();
    app.remote_tracker = Some(tracker(&["a", "b"]));
    app.home.continue_items = vec![make_item("a", "Movie")];

    app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));
    assert!(app.remote_tracker.is_none());

    app.remote_tracker = Some(tracker(&["a", "b"]));
    app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
    assert!(matches!(app.confirm_modal, Some(ConfirmModal { .. })));
    assert_eq!(app.player_tab.items.len(), 2);

    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.confirm_modal.is_none());
    assert_eq!(app.player_tab.items.len(), 2);
    assert!(app.remote_tracker.is_some());
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
fn replacement_tracker_ignores_an_earlier_in_flight_poll() {
    let mut app = attached_app();
    app.player_tab.items[0].id = "a".into();
    app.player_tab.items[1].id = "b".into();
    app.player_tab.queue_cursor = 0;
    app.session_poll_generation = 4;
    let items = app.player_tab.items.clone();
    app.remote_tracker = App::build_remote_tracker("session", &items, 1, 5);

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
