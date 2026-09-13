use super::*;
use crate::app::tests::{make_app_stub, make_item, make_session};
use mbv_core::remote_reconciliation::{ReconciliationTracker, SubmittedOccurrence, TrackingState};

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

fn attached_app() -> App {
    let mut app = make_app_stub();
    app.connected_session_id = Some("session".into());
    app.connected_session_state = Some(make_session("Client", "Emby"));
    app.terminal_width = 160;
    app.player_tab.set_items(
        vec![make_item("a", "Movie"), make_item("b", "Movie")],
        app.player_tab.queue_cursor,
    );
    app
}

#[test]
fn tracking_retirement_clears_reanchor_popup() {
    let mut app = attached_app();
    app.remote_tracker = Some(tracker(&["a", "b"]));
    app.pending_overlay = Some(super::types_overlay::OverlayRequest::RemoteReanchor(
        super::types_playback::RemoteReanchorPopup {
            targets: vec![(0, "a".into())],
            cursor: 0,
        },
    ));
    app.retire_remote_tracking(false);
    assert!(matches!(
        app.pending_overlay,
        Some(super::types_overlay::OverlayRequest::DismissRemoteReanchor)
    ));
}

#[test]
fn session_item_change_updates_state_without_mutating_queue() {
    let mut app = attached_app();
    let mut item_a = app.player_tab.emby_items()[0].clone();
    item_a.id = "a".into();
    let mut item_b = app.player_tab.emby_items()[1].clone();
    item_b.id = "b".into();
    app.player_tab.set_item_at(
        0,
        mbv_core::playback_queue::QueueItem::Emby(Box::new(item_a)),
    );
    app.player_tab.set_item_at(
        1,
        mbv_core::playback_queue::QueueItem::Emby(Box::new(item_b)),
    );
    app.player_tab.queue_cursor = 0;
    app.queue_dirty = true;
    let slots: Vec<_> = app
        .player_tab
        .queue
        .slots()
        .iter()
        .map(|slot| (slot.slot_id, slot.item.id().to_string()))
        .collect();
    let mutation_state = format!("{:?}", app.playlist_mutations);

    let mut changed = make_session("Client", "Emby");
    changed.id = "session".into();
    changed.now_playing_item_id = Some("b".into());
    app.handle_session_event(SessionEvent::Loaded {
        sessions: vec![changed],
        generation: 1,
    });

    assert_eq!(
        app.connected_session_state
            .as_ref()
            .and_then(|state| state.now_playing_item_id.as_deref()),
        Some("b")
    );
    let current_slots: Vec<_> = app
        .player_tab
        .queue
        .slots()
        .iter()
        .map(|slot| (slot.slot_id, slot.item.id().to_string()))
        .collect();
    assert_eq!(current_slots, slots);
    assert_eq!(app.player_tab.queue_cursor, 0);
    assert!(app.queue_dirty);
    assert_eq!(format!("{:?}", app.playlist_mutations), mutation_state);
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




