//! Tracking lifecycle edge-case tests (poll-gap suspension, unresolved
//! presentation, stale outcomes, and queue-context controls), split out
//! of `tests_remote_reconciliation.rs` to keep that file within the
//! repository's file-size limit.

use super::{attached_app, rendered_text, tracker};
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
        .push(crate::app::types_playback::RemoteConsumeOperation {
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
        .push(crate::app::types_playback::RemoteConsumeOperation {
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
        .push(crate::app::types_playback::RemoteConsumeOperation {
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
