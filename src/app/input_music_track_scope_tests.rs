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

// ── Task 4: scope-correct actions (#145) ─────────────────────────────

#[test]
fn current_lib_item_in_list_mode_returns_album_folder_not_a_track() {
    // Regression: album-list mode (`album_track_focus == None`) must
    // keep resolving to the selected album folder itself, exactly as
    // before Task 4.
    let mut app = make_music_album_app();
    push_tracks(&mut app, "album-1", 3);
    assert!(app.libs[0].album_track_focus.is_none());

    let item = app.current_lib_item(0);

    let item = item.expect("current_lib_item should resolve the selected album");
    assert_eq!(item.id, "album-1");
    assert!(item.is_folder, "list mode must resolve to the album folder");
}

#[test]
fn current_lib_item_in_track_mode_returns_focused_track() {
    let mut app = make_music_album_app();
    push_tracks(&mut app, "album-1", 3);
    app.libs[0].album_track_focus = Some(1);

    let item = app.current_lib_item(0);

    let item = item.expect("current_lib_item should resolve the focused track");
    assert_eq!(item.id, "album-1-track-1");
    assert!(
        !item.is_folder,
        "track mode must resolve to the track, not the album folder"
    );
}

#[test]
fn current_lib_item_in_track_mode_falls_back_safely_when_cache_missing() {
    // Async fetch still in flight: `album_tracks_cache` has no entry for
    // "album-1" yet. Must not panic and must not index out of bounds.
    let mut app = make_music_album_app();
    app.libs[0].album_track_focus = Some(0);
    assert!(!app.album_tracks_cache.contains_key("album-1"));

    let item = app.current_lib_item(0);

    let item = item.expect("must fall back to the album folder item, not None");
    assert_eq!(item.id, "album-1");
    assert!(item.is_folder);
}

#[test]
fn enter_again_in_track_mode_with_missing_cache_does_not_panic() {
    let mut app = make_music_album_app();
    // No `push_tracks` -- cache miss, async fetch still in flight.
    app.libs[0].album_track_focus = Some(0);
    let nav_len_before = app.libs[0].nav_stack.len();

    let handled = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(app.libs[0].album_track_focus, Some(0));
    assert_eq!(app.libs[0].nav_stack.len(), nav_len_before);
}

#[test]
fn context_menu_in_list_mode_offers_folder_scoped_actions_for_selected_album() {
    // Regression: album-list mode's context menu must still target the
    // selected ALBUM's id via the folder-scoped actions.
    let mut app = make_music_album_app();
    assert!(app.libs[0].album_track_focus.is_none());

    app.open_context_menu();

    let menu = match app.pending_overlay.as_ref() {
        Some(crate::app::types_overlay::OverlayRequest::ContextMenu(menu)) => menu,
        _ => panic!("context menu should open"),
    };
    let actions: Vec<_> = menu
        .entries
        .iter()
        .filter_map(|e| e.action.clone())
        .collect();
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, ContextAction::PlayFolder(id) if id == "album-1")),
        "expected PlayFolder(\"album-1\"), got: {actions:?}"
    );
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, ContextAction::ShuffleFolder(id) if id == "album-1")),
        "expected ShuffleFolder(\"album-1\"), got: {actions:?}"
    );
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, ContextAction::EnqueueFolder(item) if item.id == "album-1")),
        "expected EnqueueFolder(album-1), got: {actions:?}"
    );
}

#[test]
fn context_menu_in_track_mode_offers_track_scoped_actions_not_folder_actions() {
    let mut app = make_music_album_app();
    push_tracks(&mut app, "album-1", 3);
    app.libs[0].album_track_focus = Some(1);

    app.open_context_menu();

    let menu = match app.pending_overlay.as_ref() {
        Some(crate::app::types_overlay::OverlayRequest::ContextMenu(menu)) => menu,
        _ => panic!("context menu should open"),
    };
    let actions: Vec<_> = menu
        .entries
        .iter()
        .filter_map(|e| e.action.clone())
        .collect();
    assert!(
        actions.iter().any(|a| matches!(a, ContextAction::Play)),
        "track mode must offer the generic per-item Play action, got: {actions:?}"
    );
    assert!(
        actions.iter().any(|a| matches!(a, ContextAction::Enqueue)),
        "track mode must offer the generic per-item Enqueue action, got: {actions:?}"
    );
    assert!(
        !actions.iter().any(|a| matches!(
            a,
            ContextAction::PlayFolder(_)
                | ContextAction::ShuffleFolder(_)
                | ContextAction::EnqueueFolder(_)
        )),
        "track mode must not offer album-folder-scoped actions, got: {actions:?}"
    );
}
