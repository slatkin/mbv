//! Queue-edit tracking-retention tests, split out of
//! `tests_queue_mutation.rs` to keep that file within the repository's
//! file-size limit.

use super::tracking_stub;
use crate::app::tests::*;
use crate::app::*;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

// ── tasks 1.4 / 6.2 / 6.3: queue edits retire tracking only on real mutation ──

#[test]
fn reorder_retires_tracking_after_successful_move() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab
        .set_items(make_items(3), app.player_tab.queue_cursor);
    app.player_tab.queue_cursor = 0;
    app.remote_tracker = Some(tracking_stub());

    app.move_queue_item_down();

    assert!(app.remote_tracker.is_none());
    assert_eq!(
        app.player_tab
            .emby_items()
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["id1", "id0", "id2"]
    );
}

#[test]
fn undo_retires_tracking_after_restoring_the_edit() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab
        .set_items(make_items(3), app.player_tab.queue_cursor);
    app.remote_tracker = Some(tracking_stub());

    app.remove_from_queue(1);
    app.remote_tracker = Some(tracking_stub());
    app.undo_last_queue_edit(QueueScope::Local);

    assert!(app.remote_tracker.is_none());
    assert_eq!(app.player_tab.emby_items().len(), 3);
}

#[test]
fn empty_undo_leaves_tracking_active() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab
        .set_items(make_items(2), app.player_tab.queue_cursor);
    app.remote_tracker = Some(tracking_stub());

    app.undo_last_queue_edit(QueueScope::Local);

    assert!(app.remote_tracker.is_some());
}

#[test]
fn boundary_move_down_leaves_tracking_active() {
    let mut app = make_app_stub();
    app.player_tab
        .set_items(make_items(2), app.player_tab.queue_cursor);
    app.player_tab.queue_cursor = 1;
    app.remote_tracker = Some(tracking_stub());

    app.move_queue_item_down();

    assert!(app.remote_tracker.is_some());
    assert_eq!(app.player_tab.emby_items().len(), 2);
}

#[test]
fn out_of_range_removal_leaves_tracking_active() {
    let mut app = make_app_stub();
    app.player_tab
        .set_items(make_items(2), app.player_tab.queue_cursor);
    app.remote_tracker = Some(tracking_stub());

    app.remove_from_queue(5);

    assert!(app.remote_tracker.is_some());
    assert_eq!(app.player_tab.emby_items().len(), 2);
}

#[test]
fn rejected_route_enqueue_leaves_tracking_active() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.library_routes
        .insert("music".to_string(), "living-room-pc".to_string());
    app.active_route = Some("music".to_string());
    let mut movies_item = make_item("Movies", "CollectionFolder");
    movies_item.id = "lib-movies".to_string();
    app.libs.push(LibraryTab::new(movies_item));
    app.tab = TabSelection::EmbyLibrary(0);
    app.remote_tracker = Some(tracking_stub());

    app.enqueue_selected(Some(0));

    assert!(app.remote_tracker.is_some());
    assert!(app.status.contains("Can't mix libraries in a routed queue"));
}

#[test]
fn failed_folder_enqueue_leaves_tracking_active() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    let mut folder = make_item("Folder", "CollectionFolder");
    folder.is_folder = true;
    app.home.continue_items = vec![folder];
    app.home.continue_cursor = 0;
    app.remote_tracker = Some(tracking_stub());

    // The stub client has no server URL configured, so the enqueue fetch
    // fails at the HTTP layer before anything can be appended.
    app.enqueue_selected(None);

    assert!(app.remote_tracker.is_some());
    assert!(app.player_tab.emby_items().is_empty());
}

#[test]
fn canceled_active_item_removal_leaves_tracking_active() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab
        .set_items(make_items(3), app.player_tab.queue_cursor);
    app.player_tab.queue_cursor = 1;
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 1;
    }
    app.remote_tracker = Some(tracking_stub());

    app.remove_from_queue(1);
    assert!(matches!(
        app.pending_overlay,
        Some(super::types_overlay::OverlayRequest::Confirm(_))
    ));

    let action = match app.pending_overlay.as_ref() {
        Some(super::types_overlay::OverlayRequest::Confirm(modal)) => modal.on_confirm.clone(),
        _ => panic!("confirmation request missing"),
    };
    app.apply_confirm_action(action, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    app.dismiss_confirm();

    assert!(!matches!(
        app.pending_overlay,
        Some(super::types_overlay::OverlayRequest::Confirm(_))
    ));
    assert_eq!(app.player_tab.emby_items().len(), 3);
    assert!(app.remote_tracker.is_some());
}

#[test]
fn confirmed_active_item_removal_retires_tracking() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab
        .set_items(make_items(3), app.player_tab.queue_cursor);
    app.player_tab.queue_cursor = 1;
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 1;
    }
    app.remote_tracker = Some(tracking_stub());

    app.remove_from_queue(1);
    let action = match app.pending_overlay.as_ref() {
        Some(super::types_overlay::OverlayRequest::Confirm(modal)) => modal.on_confirm.clone(),
        _ => panic!("confirmation request missing"),
    };
    app.apply_confirm_action(
        action,
        KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
    );

    assert!(app.remote_tracker.is_none());
    assert_eq!(app.player_tab.emby_items().len(), 2);
}

// ── task 6.5: two successive removals then a save containing both edits ───
