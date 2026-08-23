#![allow(dead_code, unused_imports)]

use super::music_track_test_support::*;
use super::*;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibraryTab, PanelFocus, SelectionModalSource};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use std::io::{Read, Write};
use std::time::Duration;
/// Narrow (`is_wide_music_active() == false`, the default zero-area layout)
/// Enter on the album-folder listing opens the selection modal instead of
/// entering the in-hero `album_track_focus` mode (design.md decision 6;
/// task 3.3). Wide's unchanged behavior is covered by
/// `wide_album_enter_still_enters_track_focus_not_the_modal` in
/// `input_library_scope_routing_tests.rs`.
#[test]
fn narrow_enter_at_album_folder_listing_opens_selection_modal_without_nav_push() {
    let mut app = make_music_album_app();
    let nav_len_before = app.libs[0].nav_stack.len();
    assert!(app.is_viewing_album_folders(0));
    assert!(app.libs[0].album_track_focus.is_none());
    assert!(!app.layout.main.is_wide_music_active());

    let handled = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(!handled);
    assert!(
        app.libs[0].album_track_focus.is_none(),
        "narrow Enter must not enter the in-hero track-focus mode"
    );
    assert_eq!(app.libs[0].nav_stack.len(), nav_len_before);
    let modal = app
        .selection_modal
        .as_ref()
        .expect("narrow Enter must open the selection modal");
    assert!(matches!(modal.source, SelectionModalSource::Album { .. }));
}

#[test]
fn mouse_click_on_selected_album_folder_row_does_not_open_track_mode_or_modal() {
    // Only Enter activates the selected album-folder row (opening the
    // selection modal in narrow mode). A mouse click on the already-selected
    // row must not open it (and must not fall back to the legacy nav_stack
    // drilldown either).
    let mut app_key = make_music_album_app();
    let mut app_mouse = make_music_album_app();

    let nav_len_before = app_key.libs[0].nav_stack.len();
    assert_eq!(nav_len_before, app_mouse.libs[0].nav_stack.len());
    assert!(app_key.is_viewing_album_folders(0));
    assert!(app_key.libs[0].album_track_focus.is_none());
    assert!(app_mouse.libs[0].album_track_focus.is_none());

    let handled = app_key.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(!handled);

    // Arm focus and wait past the grace window so the click dispatches.
    app_mouse.note_focus_gained();
    std::thread::sleep(Duration::from_millis(200));
    app_mouse.layout.main.left_area = Rect::new(10, 5, 29, 4);
    app_mouse.layout.main.left_row_map = vec![Some(0)];
    app_mouse.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 11,
        row: 5,
        modifiers: KeyModifiers::NONE,
    });

    assert!(
        app_key.selection_modal.is_some(),
        "Enter must open the modal"
    );
    assert!(
        app_mouse.selection_modal.is_none(),
        "a single click must not open the modal"
    );
    assert_eq!(app_key.libs[0].album_track_focus, None);
    assert_eq!(app_mouse.libs[0].album_track_focus, None);
    assert_eq!(app_key.libs[0].nav_stack.len(), nav_len_before);
    assert_eq!(app_mouse.libs[0].nav_stack.len(), nav_len_before);
}

#[test]
fn narrow_music_track_target_does_not_compete_with_parent_hero() {
    let mut app = make_music_album_app();
    push_tracks(&mut app, "album-1", 2);
    app.layout.main.browse_destination = Some(TabSelection::EmbyLibrary(0));
    app.layout.main.hero_area = Rect::new(10, 5, 29, 8);
    app.layout.main.inline_hero_area = app.layout.main.hero_area;
    app.layout.main.wide_music_track_hitmap = vec![(Rect::new(12, 8, 20, 1), 1)];

    assert!(app.click_set_cursor(13, 8));
    assert_eq!(app.libs[0].album_track_focus, None);
}

#[test]
fn wide_music_track_target_changes_track_focus() {
    let mut app = make_music_album_app();
    push_tracks(&mut app, "album-1", 2);
    app.layout.main.browse_destination = Some(TabSelection::EmbyLibrary(0));
    app.layout.main.wide_music_right_area = Rect::new(40, 0, 20, 15);
    app.layout.main.wide_music_track_hitmap = vec![(Rect::new(12, 8, 20, 1), 1)];

    assert!(app.click_set_cursor(13, 8));
    assert_eq!(app.libs[0].album_track_focus, Some(1));
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
    // refocus_at is still Some (not consumed) -- the grace window check
    // only suppresses, it doesn't take the field.
    assert!(app.refocus_at.is_some());
}

#[test]
fn click_while_unfocused_is_suppressed() {
    let mut app = make_music_album_app();
    // Simulate an unfocused terminal.
    app.refocus_at = None;
    assert!(app.refocus_at.is_none());
    app.layout.main.left_area = Rect::new(10, 5, 29, 4);
    app.layout.main.left_row_map = vec![Some(1)];

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 11,
        row: 5,
        modifiers: KeyModifiers::NONE,
    });

    // Suppressed: cursor did not move.
    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 0);
}

#[test]
fn click_outside_refocus_window_dispatches_normally() {
    let mut app = make_music_album_app();
    // Simulate having been focused long ago (past the grace window).
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
    // First click: within grace window -- suppressed.
    app.handle_mouse(click);
    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 0);

    // Simulate time passing past the grace window.
    app.refocus_at = Some(Instant::now() - Duration::from_millis(500));
    app.handle_mouse(click);
    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 1);
}

#[test]
fn focus_lost_then_regained_suppresses_until_focus_gained() {
    let mut app = make_music_album_app();
    app.note_focus_gained();
    std::thread::sleep(Duration::from_millis(200));
    app.note_focus_lost();
    app.layout.main.left_area = Rect::new(10, 5, 29, 4);
    app.layout.main.left_row_map = vec![Some(1)];

    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 11,
        row: 5,
        modifiers: KeyModifiers::NONE,
    };

    // After focus lost: click is suppressed.
    app.handle_mouse(click);
    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 0);

    // Regain focus: first click is the refocusing click (suppressed).
    app.note_focus_gained();
    app.handle_mouse(click);
    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 0);

    // After grace window: click dispatches.
    app.refocus_at = Some(Instant::now() - Duration::from_millis(500));
    app.handle_mouse(click);
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
