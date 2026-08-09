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
    app.player_tab.items = make_items(3);
    app.player_tab.queue_cursor = 0;
    app.remote_tracker = Some(tracking_stub());

    app.move_queue_item_down();

    assert!(app.remote_tracker.is_none());
    assert_eq!(
        app.player_tab
            .items
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
    app.player_tab.items = make_items(3);
    app.remote_tracker = Some(tracking_stub());

    app.remove_from_queue(1);
    app.remote_tracker = Some(tracking_stub());
    app.undo_last_queue_edit(QueueScope::Local);

    assert!(app.remote_tracker.is_none());
    assert_eq!(app.player_tab.items.len(), 3);
}

#[test]
fn empty_undo_leaves_tracking_active() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.items = make_items(2);
    app.remote_tracker = Some(tracking_stub());

    app.undo_last_queue_edit(QueueScope::Local);

    assert!(app.remote_tracker.is_some());
}

#[test]
fn boundary_move_down_leaves_tracking_active() {
    let mut app = make_app_stub();
    app.player_tab.items = make_items(2);
    app.player_tab.queue_cursor = 1;
    app.remote_tracker = Some(tracking_stub());

    app.move_queue_item_down();

    assert!(app.remote_tracker.is_some());
    assert_eq!(app.player_tab.items.len(), 2);
}

#[test]
fn out_of_range_removal_leaves_tracking_active() {
    let mut app = make_app_stub();
    app.player_tab.items = make_items(2);
    app.remote_tracker = Some(tracking_stub());

    app.remove_from_queue(5);

    assert!(app.remote_tracker.is_some());
    assert_eq!(app.player_tab.items.len(), 2);
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
    app.libs.push(LibraryTab {
        library: movies_item,
        search: None,
        nav_stack: Vec::new(),
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    app.tab = TabSelection::Library(0);
    app.remote_tracker = Some(tracking_stub());

    app.enqueue_selected();

    assert!(app.remote_tracker.is_some());
    assert!(app.status.contains("Can't mix libraries in a routed queue"));
}

fn serve_one_response(listener: &std::net::TcpListener, body: &str) {
    let (stream, _) = listener.accept().unwrap();
    let mut writer = stream.try_clone().unwrap();
    use std::io::Write;
    let _ = writer.write_all(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .as_bytes(),
    );
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
    app.enqueue_selected();

    assert!(app.remote_tracker.is_some());
    assert!(app.player_tab.items.is_empty());
}

#[test]
fn empty_folder_enqueue_leaves_tracking_active() {
    let _guard = crate::config::TestStateDirGuard::new();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.server_url = url;
    let mut folder = make_item("Folder", "CollectionFolder");
    folder.is_folder = true;
    app.home.continue_items = vec![folder];
    app.home.continue_cursor = 0;
    app.remote_tracker = Some(tracking_stub());

    let handle = std::thread::spawn(move || serve_one_response(&listener, r#"{"Items":[]}"#));
    app.enqueue_selected();
    handle.join().unwrap();

    assert!(app.remote_tracker.is_some());
    assert!(app.player_tab.items.is_empty());
    assert!(app.status.contains("Nothing to enqueue"));
}

#[test]
fn canceled_active_item_removal_leaves_tracking_active() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.items = make_items(3);
    app.player_tab.queue_cursor = 1;
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 1;
    }
    app.remote_tracker = Some(tracking_stub());

    app.remove_from_queue(1);
    assert!(app.confirm_modal.is_some());

    app.handle_key_confirm_modal(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    assert!(app.confirm_modal.is_none());
    assert_eq!(app.player_tab.items.len(), 3);
    assert!(app.remote_tracker.is_some());
}

#[test]
fn confirmed_active_item_removal_retires_tracking() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.items = make_items(3);
    app.player_tab.queue_cursor = 1;
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 1;
    }
    app.remote_tracker = Some(tracking_stub());

    app.remove_from_queue(1);
    app.handle_key_confirm_modal(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));

    assert!(app.remote_tracker.is_none());
    assert_eq!(app.player_tab.items.len(), 2);
}

// ── task 6.5: two successive removals then a save containing both edits ───

#[test]
fn two_removals_apply_immediately_and_save_snapshots_both_edits() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.items = make_items(4);
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("pl-1".into()),
        name: "Saved".into(),
    };
    app.remote_tracker = Some(tracking_stub());

    app.remove_from_queue(0);
    app.remove_from_queue(1);
    assert!(app.remote_tracker.is_none());
    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["id1", "id3"]
    );

    app.save_playlist_to_emby();
    let item_ids = match app
        .playlist_mutations
        .get("pl-1")
        .and_then(|state| state.active.as_ref())
    {
        Some(super::types_playback::PlaylistMutation::Save {
            item_ids: Some(ids),
            ..
        }) => ids.clone(),
        other => panic!("expected active save, got {other:?}"),
    };
    assert_eq!(
        item_ids,
        vec!["id1", "id3"],
        "the following playlist save must contain both removals"
    );
}
