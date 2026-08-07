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

    assert!(app.libs[0].artist_header_focus.is_none());
    assert_eq!(
        app.libs[0].nav_stack.last().unwrap().cursor,
        2,
        "down from a group's last row moves to the next group's first album"
    );

    // Up from Beta's first album (the first row of its group) returns to
    // the previous group's last album.
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));

    assert!(app.libs[0].artist_header_focus.is_none());
    assert_eq!(
        app.libs[0].nav_stack.last().unwrap().cursor,
        1,
        "up from a group's first row moves to the previous group's last album"
    );
}

#[test]
fn artist_header_selection_survives_group_size_change() {
    let mut app = make_music_album_app();
    let mut zeta_album = make_item("Zeta Album", "MusicAlbum");
    zeta_album.id = "album-zeta".into();
    zeta_album.artist = "Zeta".into();
    zeta_album.is_folder = true;
    app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .items
        .push(zeta_album);
    app.libs[0].artist_header_focus = Some(crate::app::ArtistHeaderSelection {
        first_album_id: "album-zeta".into(),
        artist_label: "Zeta".into(),
    });

    let mut zeta_album_two = make_item("Zeta Album Two", "MusicAlbum");
    zeta_album_two.id = "album-zeta-2".into();
    zeta_album_two.artist = "Zeta".into();
    zeta_album_two.is_folder = true;
    app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .items
        .push(zeta_album_two);

    render_full_app(&mut app, 100, 24);

    assert!(
        app.libs[0].artist_header_focus.is_some(),
        "revalidation should keep the same artist header focused when the \
             loaded sibling count changes"
    );
    assert_eq!(
        app.selected_artist_header_album_items(0)
            .expect("expected Zeta header selection to remain valid")
            .1
            .len(),
        2,
        "the same focused header should resolve the expanded group after another album loads"
    );
}

#[test]
fn selectable_artist_header_enter_is_consumed_noop() {
    let mut app = make_music_album_app();
    app.libs[0].artist_header_focus = Some(crate::app::ArtistHeaderSelection {
        first_album_id: "album-1".into(),
        artist_label: "Unknown Artist".into(),
    });
    let nav_len = app.libs[0].nav_stack.len();

    let handled = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(!handled);
    assert_eq!(app.libs[0].nav_stack.len(), nav_len);
    assert!(app.libs[0].album_track_focus.is_none());
    assert!(app.libs[0].artist_header_focus.is_some());
}

#[test]
fn selectable_artist_header_mouse_click_selects_header() {
    let mut app = make_music_album_app();
    add_beta_album(&mut app);
    // Tall enough that the album hero leaves the Beta header visible in the
    // list below it.
    render_full_app(&mut app, 100, 40);
    let row = app
        .layout
        .main
        .left_row_targets
        .iter()
        .position(|target| {
            matches!(
                target,
                Some(LibraryRowTarget::ArtistHeader(selection))
                    if selection.artist_label == "Beta"
            )
        })
        .expect("expected Beta header row target");
    let x = app.layout.main.left_area.x;
    let y = app.layout.main.left_area.y + row as u16;

    let handled = app.click_set_cursor(x, y);

    assert!(handled);
    assert_eq!(
        app.libs[0].artist_header_focus,
        Some(crate::app::ArtistHeaderSelection {
            first_album_id: "album-beta".into(),
            artist_label: "Beta".into(),
        })
    );
    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 0);
}

#[test]
fn selectable_artist_header_context_menu_uses_header_actions() {
    let mut app = make_music_album_app();
    app.libs[0].artist_header_focus = Some(crate::app::ArtistHeaderSelection {
        first_album_id: "album-1".into(),
        artist_label: "Unknown Artist".into(),
    });

    app.open_context_menu();

    let menu = app.context_menu.as_ref().expect("expected header menu");
    let labels: Vec<&str> = menu.entries.iter().map(|entry| entry.label).collect();
    assert_eq!(labels, vec!["Play All", "Shuffle", "Add to Queue"]);
    assert!(menu
        .entries
        .iter()
        .all(|entry| !matches!(entry.action, Some(ContextAction::PlayFolder(_)))));
}

#[test]
fn selectable_artist_header_members_use_current_display_plan_albums_only() {
    let mut app = make_music_album_app();
    add_beta_album(&mut app);
    app.libs[0].artist_header_focus = Some(crate::app::ArtistHeaderSelection {
        first_album_id: "album-1".into(),
        artist_label: "Unknown Artist".into(),
    });

    let (_, albums) = app
        .selected_artist_header_album_items(0)
        .expect("expected selected header members");
    let ids: Vec<&str> = albums.iter().map(|album| album.id.as_str()).collect();

    assert_eq!(
        ids,
        vec!["album-1", "album-2"],
        "member resolution should preserve display album order and exclude Beta"
    );
}

#[test]
fn selectable_artist_header_stale_selection_is_cleared_on_revalidation() {
    let mut app = make_music_album_app();
    app.libs[0].artist_header_focus = Some(crate::app::ArtistHeaderSelection {
        first_album_id: "missing-album".into(),
        artist_label: "Unknown Artist".into(),
    });

    let albums = app.selected_artist_header_album_items(0);

    assert!(albums.is_none());
    assert!(app.libs[0].artist_header_focus.is_none());
}

#[test]
fn selectable_artist_header_direct_enqueue_fetches_header_albums_not_stale_cursor() {
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

    app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));

    let queued_ids: Vec<&str> = app
        .player_tab
        .items
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    assert_eq!(
        queued_ids,
        vec!["a1-t1", "a1-t2", "a2-t1"],
        "enqueue should preserve display album order and per-album track order"
    );
    let mut first_seen = server.first_seen_parent_ids();
    first_seen.sort();
    assert_eq!(
        first_seen,
        vec!["album-1".to_string(), "album-2".to_string()],
        "recursive fetches should target the selected header's albums, not stale album-beta"
    );
}
