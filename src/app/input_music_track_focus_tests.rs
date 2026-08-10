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
fn enter_at_album_folder_listing_enters_track_mode_without_nav_push() {
    let mut app = make_music_album_app();
    let nav_len_before = app.libs[0].nav_stack.len();
    assert!(app.is_viewing_album_folders(0));
    assert!(app.libs[0].album_track_focus.is_none());

    let handled = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(app.libs[0].album_track_focus, Some(0));
    assert_eq!(app.libs[0].nav_stack.len(), nav_len_before);
}

#[test]
fn mouse_click_on_selected_album_folder_row_does_not_open_track_mode() {
    // Only Enter opens inline track-selection mode. A mouse click on the
    // already-selected album-folder row must not open it (and must not
    // fall back to the legacy nav_stack drilldown either).
    let mut app_key = make_music_album_app();
    let mut app_mouse = make_music_album_app();

    let nav_len_before = app_key.libs[0].nav_stack.len();
    assert_eq!(nav_len_before, app_mouse.libs[0].nav_stack.len());
    assert!(app_key.is_viewing_album_folders(0));
    assert!(app_key.libs[0].album_track_focus.is_none());
    assert!(app_mouse.libs[0].album_track_focus.is_none());

    let handled = app_key.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(!handled);

    app_mouse.layout.main.left_area = Rect::new(10, 5, 29, 4);
    app_mouse.layout.main.left_row_map = vec![Some(0)];
    app_mouse.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 11,
        row: 5,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(app_key.libs[0].album_track_focus, Some(0));
    assert_eq!(app_mouse.libs[0].album_track_focus, None);
    assert_eq!(app_key.libs[0].nav_stack.len(), nav_len_before);
    assert_eq!(app_mouse.libs[0].nav_stack.len(), nav_len_before);
}

#[test]
fn refocus_click_after_focus_gained_is_suppressed() {
    let mut app = make_music_album_app();
    app.note_focus_gained();
    app.layout.main.left_area = Rect::new(10, 5, 29, 4);
    app.layout.main.left_row_map = vec![Some(1)];

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 11,
        row: 5,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 0);
    assert!(app.refocus_at.is_none());
}

#[test]
fn click_without_focus_event_dispatches_normally() {
    let mut app = make_music_album_app();
    assert!(app.refocus_at.is_none());
    app.layout.main.left_area = Rect::new(10, 5, 29, 4);
    app.layout.main.left_row_map = vec![Some(1)];

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 11,
        row: 5,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 1);
}

#[test]
fn click_outside_refocus_window_dispatches_normally() {
    let mut app = make_music_album_app();
    app.refocus_at = Some(Instant::now() - Duration::from_millis(500));
    app.layout.main.left_area = Rect::new(10, 5, 29, 4);
    app.layout.main.left_row_map = vec![Some(1)];

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 11,
        row: 5,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 1);
}

#[test]
fn second_click_after_refocus_dispatches() {
    let mut app = make_music_album_app();
    app.note_focus_gained();
    app.layout.main.left_area = Rect::new(10, 5, 29, 4);
    app.layout.main.left_row_map = vec![Some(1)];

    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 11,
        row: 5,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse(click);
    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 0);

    app.handle_mouse(click);
    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 1);
}

#[test]
fn focus_lost_clears_pending_refocus() {
    let mut app = make_music_album_app();
    app.note_focus_gained();
    app.note_focus_lost();
    app.layout.main.left_area = Rect::new(10, 5, 29, 4);
    app.layout.main.left_row_map = vec![Some(1)];

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 11,
        row: 5,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 1);
}

#[test]
fn up_down_at_group_boundary_moves_between_groups_skipping_headers() {
    let mut app = make_music_album_list_app(2, 1);
    add_following_artist_albums(&mut app, 2);
    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 1);

    // Down from Alpha's last album (the last row of its group) jumps to
    // the first album of the next group (Beta) -- the artist header is
    // not a resting position for arrow movement (see `grouped_cursor_target`).
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

    assert_eq!(
        app.libs[0].nav_stack.last().unwrap().cursor,
        2,
        "down from a group's last row moves to the next group's first album"
    );

    // Up from Beta's first album (the first row of its group) returns to
    // the previous group's last album.
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));

    assert_eq!(
        app.libs[0].nav_stack.last().unwrap().cursor,
        1,
        "up from a group's first row moves to the previous group's last album"
    );
}
