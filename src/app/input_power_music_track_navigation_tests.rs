#![allow(dead_code, unused_imports)]

use super::power_music_track_test_support::*;
use super::*;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibraryTab, PanelFocus};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use std::io::{Read, Write};
#[test]
fn up_down_in_track_mode_move_only_track_focus_and_clamp() {
    let mut app = make_power_music_album_app();
    push_tracks(&mut app, "album-1", 3);
    app.libs[0].album_track_focus = Some(1);
    let album_cursor_before = app.libs[0].nav_stack.last().unwrap().cursor;

    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.libs[0].album_track_focus, Some(2));
    // Clamp at the end -- no wrap.
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.libs[0].album_track_focus, Some(2));

    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(app.libs[0].album_track_focus, Some(0));
    // Clamp at the start -- no wrap.
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(app.libs[0].album_track_focus, Some(0));

    assert_eq!(
        app.libs[0].nav_stack.last().unwrap().cursor,
        album_cursor_before,
        "track-mode Up/Down must not move the album cursor"
    );
}

#[test]
fn track_mode_down_does_not_move_track_focus_when_queue_panel_has_focus() {
    let mut app = make_power_music_album_app();
    push_tracks(&mut app, "album-1", 3);
    app.libs[0].album_track_focus = Some(1);
    app.panel_focus = PanelFocus::Queue;
    let album_cursor_before = app.libs[0].nav_stack.last().unwrap().cursor;

    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

    assert_eq!(app.libs[0].album_track_focus, Some(1));
    assert_eq!(
        app.libs[0].nav_stack.last().unwrap().cursor,
        album_cursor_before
    );
}

#[test]
fn mouse_clicking_another_album_clears_track_focus() {
    let mut app = make_power_music_album_app();
    push_tracks(&mut app, "album-1", 3);
    app.libs[0].album_track_focus = Some(1);
    app.layout.main.left_area = Rect::new(10, 5, 29, 4);
    app.layout.main.left_row_map = vec![Some(1)];

    let handled = app.click_set_cursor(11, 5);

    assert!(handled);
    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 1);
    assert!(app.libs[0].album_track_focus.is_none());
}

#[test]
fn selecting_music_group_clears_track_focus() {
    let mut app = make_power_music_album_app();
    let mut group2 = make_item("Beta", "MusicArtist");
    group2.id = "group-1".into();
    group2.is_folder = true;
    app.libs[0].nav_stack[0].items.push(group2);
    app.libs[0].album_track_focus = Some(1);
    app.libs[0].artist_header_focus = Some(crate::app::ArtistHeaderSelection {
        first_album_id: "album-1".into(),
        artist_label: "Unknown Artist".into(),
    });

    app.select_music_group(0, 1);

    assert!(app.libs[0].album_track_focus.is_none());
    assert!(app.libs[0].artist_header_focus.is_none());
}

#[test]
fn switching_music_group_clears_track_focus() {
    let mut app = make_power_music_album_app();
    let mut group2 = make_item("Beta", "MusicArtist");
    group2.id = "group-1".into();
    group2.is_folder = true;
    app.libs[0].nav_stack[0].items.push(group2);
    app.libs[0].album_track_focus = Some(1);
    app.libs[0].artist_header_focus = Some(crate::app::ArtistHeaderSelection {
        first_album_id: "album-1".into(),
        artist_label: "Unknown Artist".into(),
    });

    app.switch_music_group(0, 1);

    assert!(app.libs[0].album_track_focus.is_none());
    assert!(app.libs[0].artist_header_focus.is_none());
}

#[test]
fn up_down_in_track_mode_with_no_cached_tracks_is_noop() {
    let mut app = make_power_music_album_app();
    // No `push_tracks` call -- album_tracks_cache has no entry for
    // "album-1", mirroring "not yet loaded".
    app.libs[0].album_track_focus = Some(0);

    let handled = app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(app.libs[0].album_track_focus, Some(0));
}

#[test]
fn escape_in_track_mode_clears_focus_without_go_back() {
    let mut app = make_power_music_album_app();
    push_tracks(&mut app, "album-1", 3);
    app.libs[0].album_track_focus = Some(2);
    let nav_len_before = app.libs[0].nav_stack.len();
    let album_cursor_before = app.libs[0].nav_stack.last().unwrap().cursor;

    let handled = app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    assert!(!handled);
    assert!(app.libs[0].album_track_focus.is_none());
    assert_eq!(
        app.libs[0].nav_stack.len(),
        nav_len_before,
        "Escape in track mode must not pop nav_stack (not a go_back)"
    );
    assert_eq!(
        app.libs[0].nav_stack.last().unwrap().cursor,
        album_cursor_before
    );
}

#[test]
fn up_down_outside_track_mode_still_move_album_cursor() {
    let mut app = make_power_music_album_app();
    assert!(app.libs[0].album_track_focus.is_none());

    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

    assert!(app.libs[0].album_track_focus.is_none());
    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 1);
}

#[test]
fn escape_outside_track_mode_still_calls_go_back_unchanged() {
    // `make_power_music_album_app`'s grouped `["group","album"]` fixture
    // sits at the *root* of the synthetic music-group view (nav_stack
    // len == 2), which `go_back`'s own pre-existing guard already
    // no-ops on ("don't pop when already at the root of a synthetic
    // group view" -- see `go_back`'s doc comment in actions.rs). The
    // regression this proves is narrower than "pops": Task 3 must route
    // Escape to the exact same `go_back()` call as before when
    // `album_track_focus` is `None`, whatever `go_back()` itself does --
    // demonstrated by comparing `handle_key(Esc)` against calling
    // `go_back()` directly on an identical, freshly-built app.
    let mut via_go_back = make_power_music_album_app();
    via_go_back.go_back();

    let mut via_escape_key = make_power_music_album_app();
    assert!(via_escape_key.libs[0].album_track_focus.is_none());
    let handled = via_escape_key.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(
        via_escape_key.libs[0].nav_stack.len(),
        via_go_back.libs[0].nav_stack.len()
    );
    assert_eq!(
        via_escape_key.libs[0].nav_stack.last().unwrap().cursor,
        via_go_back.libs[0].nav_stack.last().unwrap().cursor
    );
}

#[test]
fn page_down_in_album_list_mode_pages_by_rendered_rows_with_inline_detail() {
    let mut app = make_power_music_album_list_app(60, 0);
    push_tracks(&mut app, "album-0", 4);
    render_full_app(&mut app, 100, 40);
    let viewport_rows = app.layout.main.left_area.height as usize;
    assert_eq!(
        viewport_rows, 30,
        "fixture sanity: expected 30 rendered list rows"
    );
    assert!(app.power_right_panel_image_renders_allowed());

    let handled = app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));

    assert!(!handled);
    assert!(!app.power_right_panel_image_renders_allowed());
    // The selected artist block starts with its border, padding, header, and
    // pinned hint, then renders every album. A 30-row page from album 0's
    // display row lands on album 30.
    assert_eq!(
        app.libs[0].nav_stack.last().unwrap().cursor,
        30,
        "PageDown should move by rendered display rows, not raw album count"
    );
    assert!(app.libs[0].album_track_focus.is_none());
}

#[test]
fn page_up_in_album_list_mode_pages_by_rendered_rows_with_inline_detail() {
    let mut app = make_power_music_album_list_app(60, 35);
    push_tracks(&mut app, "album-35", 4);
    render_full_app(&mut app, 100, 40);
    let viewport_rows = app.layout.main.left_area.height as usize;
    assert_eq!(
        viewport_rows, 30,
        "fixture sanity: expected 30 rendered list rows"
    );

    let handled = app.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));

    assert!(!handled);
    // The selected artist block contains the header, pinned hint, and every
    // album. A 30-row page up from album 35 lands on album 5.
    assert_eq!(
        app.libs[0].nav_stack.last().unwrap().cursor,
        5,
        "PageUp should move by rendered display rows, not raw album count"
    );
    assert!(app.libs[0].album_track_focus.is_none());
}

#[test]
fn paging_past_display_edges_clamps_in_display_order_not_api_order() {
    let mut app = make_power_music_album_list_app(3, 0);
    app.libs[0].nav_stack.last_mut().unwrap().items[0].artist = "Zulu".into();
    app.libs[0].nav_stack.last_mut().unwrap().items[1].artist = "Alpha".into();
    app.libs[0].nav_stack.last_mut().unwrap().items[2].artist = "Bravo".into();
    push_tracks(&mut app, "album-0", 4);
    render_full_app(&mut app, 100, 40);
    app.layout.main.left_area.height = 100;

    let handled = app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(
        app.libs[0].nav_stack.last().unwrap().cursor,
        0,
        "PageDown past the last display row should clamp to the last display-order album"
    );

    let mut app = make_power_music_album_list_app(3, 1);
    app.libs[0].nav_stack.last_mut().unwrap().items[0].artist = "Zulu".into();
    app.libs[0].nav_stack.last_mut().unwrap().items[1].artist = "Alpha".into();
    app.libs[0].nav_stack.last_mut().unwrap().items[2].artist = "Bravo".into();
    push_tracks(&mut app, "album-1", 4);
    render_full_app(&mut app, 100, 40);
    app.layout.main.left_area.height = 100;

    let handled = app.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(
        app.libs[0].nav_stack.last().unwrap().cursor,
        1,
        "PageUp past the first display row should clamp to the first display-order album"
    );
}

#[test]
fn paging_from_non_selectable_hint_and_header_rows_chooses_nearest_album_by_direction() {
    // Inline tracks (and the rule/loading rows around them) no longer
    // render in the music-group view until track-selection mode is
    // entered (Enter pressed), so paging can no longer land on those --
    // browsing-mode paging is disabled entirely once track-selection
    // mode is active (see `page_power_grouped_album_cursor`'s
    // `album_track_focus.is_some()` guard). The two non-selectable rows
    // paging can still land on while merely *browsing* the album list
    // are: the artist header, and the collapsed action-hint row that
    // sits directly under the selected album.
    let mut down_app = make_power_music_album_list_app(10, 0);
    render_full_app(&mut down_app, 100, 40);
    assert!(down_app.libs[0].album_track_focus.is_none());
    down_app.layout.main.left_area.height = 1;

    let handled = down_app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));

    assert!(!handled);
    // Display rows: 0 artist header; selected album 0 is wrapped in the
    // colored-block frame (1 top border, 2 colored top padding, 3
    // album row, 4 collapsed action hint, 5 colored bottom padding, 6
    // bottom border), then 7 = album 1. With a 1-row
    // page, PageDown targets the hint row, so paging resolves forward to
    // album 1.
    assert_eq!(down_app.libs[0].nav_stack.last().unwrap().cursor, 1);

    let mut up_app = make_power_music_album_list_app(10, 3);
    render_full_app(&mut up_app, 100, 40);
    assert!(up_app.libs[0].album_track_focus.is_none());
    up_app.layout.main.left_area.height = 4;

    let handled = up_app.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));

    assert!(!handled);
    // The selected artist block contains the header, pinned hint, and every
    // album. With a 4-row page, PageUp from album 3 resolves to album 0
    // rather than leaving the cursor on the non-album header row.
    assert_eq!(up_app.libs[0].nav_stack.last().unwrap().cursor, 0);
}

#[test]
fn oversized_artist_block_scrolls_inline_without_moving_the_outer_block() {
    let mut app = make_power_music_album_list_app(60, 0);
    render_full_app(&mut app, 100, 40);
    let initial_offset = app.libs[0].nav_stack.last().unwrap().scroll;

    for _ in 0..35 {
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    }
    render_full_app(&mut app, 100, 40);
    let down_offset = app.libs[0].nav_stack.last().unwrap().scroll;
    assert_eq!(down_offset, initial_offset);
    assert!(app
        .layout
        .main
        .left_row_targets
        .iter()
        .any(|target| matches!(target, Some(LibraryRowTarget::Album(35)))));
    let cursor_y = app
        .layout
        .main
        .cursor_screen_y
        .expect("expected the active album marker on screen");
    let area = app.layout.main.left_area;
    assert!(cursor_y >= area.y && cursor_y < area.y + area.height);

    for _ in 0..35 {
        app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    }
    render_full_app(&mut app, 100, 40);
    let up_offset = app.libs[0].nav_stack.last().unwrap().scroll;
    assert_eq!(up_offset, initial_offset);
    assert!(app
        .layout
        .main
        .left_row_targets
        .iter()
        .any(|target| matches!(target, Some(LibraryRowTarget::Album(0)))));
}

#[test]
fn oversized_artist_navigation_reaches_hidden_albums_before_following_artist() {
    let mut app = make_power_music_album_list_app(60, 0);
    add_following_artist_albums(&mut app, 2);
    render_full_app(&mut app, 100, 40);

    for expected_cursor in 1..60 {
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(
            app.libs[0].nav_stack.last().unwrap().cursor,
            expected_cursor
        );
        assert!(app.libs[0].artist_header_focus.is_none());
    }

    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    let beta_header = app.libs[0]
        .artist_header_focus
        .as_ref()
        .expect("expected navigation to reach the following artist header");
    assert_eq!(beta_header.artist_label, "Beta");

    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert!(app.libs[0].artist_header_focus.is_none());
    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 60);
}

#[test]
fn page_down_crosses_oversized_artist_window_to_following_artist() {
    let mut app = make_power_music_album_list_app(60, 59);
    add_following_artist_albums(&mut app, 2);
    render_full_app(&mut app, 100, 40);
    app.layout.main.left_area.height = 1;

    app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));

    assert!(app.libs[0].artist_header_focus.is_none());
    assert_eq!(
        app.libs[0].nav_stack.last().unwrap().cursor,
        60,
        "PageDown should leave the oversized artist at its boundary"
    );
}

fn buffer_to_string(term: &ratatui::Terminal<ratatui::backend::TestBackend>) -> String {
    let buf = term.backend().buffer();
    let area = *buf.area();
    let mut out = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}
