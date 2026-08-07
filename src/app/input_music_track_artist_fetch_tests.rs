#![allow(dead_code, unused_imports)]

use super::music_track_test_support::*;
use super::*;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibraryTab, PanelFocus};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use std::io::{Read, Write};
#[test]
fn selectable_artist_header_direct_play_fetches_header_albums_not_stale_cursor() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_selectable_artist_header_bulk_app();
    let server = RecursiveFetchServer::start(vec![
        (
            "album-1",
            Ok(vec![("a1-t2", "A1 Track 2", 2), ("a1-t1", "A1 Track 1", 1)]),
        ),
        ("album-2", Ok(vec![("a2-t1", "A2 Track 1", 1)])),
    ]);
    configure_recursive_fetch_server(&mut app, &server);

    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));

    let queued_ids: Vec<&str> = app
        .player_tab
        .items
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    assert_eq!(queued_ids, vec!["a1-t1", "a1-t2", "a2-t1"]);
    assert_eq!(app.player_tab.queue_cursor, 0);
    let mut first_seen = server.first_seen_parent_ids();
    first_seen.sort();
    assert_eq!(
        first_seen,
        vec!["album-1".to_string(), "album-2".to_string()]
    );
}

#[test]
fn selectable_artist_header_context_shuffle_fetches_header_albums_not_stale_cursor() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_selectable_artist_header_bulk_app();
    let server = RecursiveFetchServer::start(vec![
        ("album-1", Ok(vec![("a1-t1", "A1 Track 1", 1)])),
        ("album-2", Ok(vec![("a2-t1", "A2 Track 1", 1)])),
    ]);
    configure_recursive_fetch_server(&mut app, &server);
    app.open_context_menu();
    let action = app
        .context_menu
        .as_ref()
        .and_then(|menu| {
            menu.entries
                .iter()
                .find(|entry| entry.label == "Shuffle")
                .and_then(|entry| entry.action.clone())
        })
        .expect("expected Shuffle header action");

    app.execute_context_action(Some(action));

    let mut queued_ids: Vec<&str> = app
        .player_tab
        .items
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    queued_ids.sort_unstable();
    assert_eq!(queued_ids, vec!["a1-t1", "a2-t1"]);
    let mut first_seen = server.first_seen_parent_ids();
    first_seen.sort();
    assert_eq!(
        first_seen,
        vec!["album-1".to_string(), "album-2".to_string()]
    );
}

#[test]
fn selectable_artist_header_fetch_error_leaves_queue_and_playback_unchanged() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_selectable_artist_header_bulk_app();
    let mut existing = make_item("Existing", "Audio");
    existing.id = "existing-track".into();
    existing.media_type = "Audio".into();
    app.player_tab.set_items(vec![existing], 0);
    let before_ids: Vec<String> = app
        .player_tab
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect();
    let server = RecursiveFetchServer::start(vec![
        ("album-1", Ok(vec![("a1-t1", "A1 Track 1", 1)])),
        ("album-2", Err("album fetch failed")),
    ]);
    configure_recursive_fetch_server(&mut app, &server);

    app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));

    let after_ids: Vec<String> = app
        .player_tab
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect();
    assert_eq!(
        after_ids, before_ids,
        "enqueue must abort before mutation when any album fetch fails"
    );
    assert!(
        app.status.contains("status code 500"),
        "expected one surfaced fetch error, got {:?}; seen parent ids: {:?}",
        app.status,
        server.seen_parent_ids()
    );
    let mut first_seen = server.first_seen_parent_ids();
    first_seen.sort();
    assert_eq!(
        first_seen,
        vec!["album-1".to_string(), "album-2".to_string()]
    );
}
