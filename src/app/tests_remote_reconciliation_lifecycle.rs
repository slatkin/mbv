//! Tracking lifecycle edge-case tests (poll-gap suspension, unresolved
//! presentation, stale outcomes, and queue-context controls), split out
//! of `tests_remote_reconciliation.rs` to keep that file within the
//! repository's file-size limit.

use super::{attached_app, tracker};
use crate::app::tests::{make_item, make_session};
use crate::app::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::remote_reconciliation::{RemoteObservation, TrackingState};

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
    app.tab = TabSelection::EmbyLibrary(0);
    let mut lib_item = make_item("Movies", "CollectionFolder");
    lib_item.id = "lib-movies".into();
    app.libs.push(LibraryTab::new(lib_item));
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
