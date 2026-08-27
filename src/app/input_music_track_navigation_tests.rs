#![allow(dead_code, unused_imports)]

use super::music_track_test_support::*;
use super::*;
use crate::app::components::msg::ShellRequest;
use crate::app::components::music_workspace::MusicWorkspaceComponent;
use crate::app::components::{ComponentId, Msg};
use crate::app::render::{LibraryListRenderCtx, MusicWideRenderCtx};
use crate::app::shell::Model;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibraryTab, PanelFocus};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use std::io::{Read, Write};
use tuirealm::component::AppComponent;
use tuirealm::event::{
    Event, Key as TuiKey, KeyEvent as TuiKeyEvent, KeyModifiers as TuiKeyModifiers,
};

/// Wide music-group fixture with `album-1`'s cached tracks mirroring the
/// legacy `make_music_album_app` + `push_tracks` setup.
fn wide_track_focus_model(track_count: usize) -> (Model, ComponentId) {
    let mut app = make_music_album_app();
    push_tracks(&mut app, "album-1", track_count);
    let mut model = Model::new(app);
    model.app.layout.main.wide_music_area = Rect::new(0, 0, 100, 30);
    model.app.layout.main.wide_music_right_area = Rect::new(50, 0, 50, 30);
    model.sync_music_workspace();
    let id = model
        .music_workspace_id
        .clone()
        .expect("wide Music workspace mounted");
    (model, id)
}

fn drive(model: &mut Model, id: &ComponentId, code: TuiKey) -> Option<Msg> {
    model
        .application
        .get_component_mut(id)
        .unwrap()
        .on(&Event::Keyboard(TuiKeyEvent {
            code,
            modifiers: TuiKeyModifiers::NONE,
        }))
}

/// A component with the selected album's tracks cached and inline track
/// focus enabled (wide), ready to enter track mode.
fn component_with_tracks() -> MusicWorkspaceComponent {
    component_with_tracks_ctx(true)
}

/// `component_with_tracks` with the given panel focus (`focused`): the
/// Queue-panel regression needs `false` so track-mode keys fall through to
/// legacy queue handling.
fn component_with_tracks_ctx(focused: bool) -> MusicWorkspaceComponent {
    let album = make_item("First Album", "MusicAlbum");
    let tracks: Vec<_> = (0..3)
        .map(|i| {
            let mut t = make_item(&format!("Track {i}"), "Audio");
            t.id = format!("track-{i}");
            t
        })
        .collect();
    let mut component = MusicWorkspaceComponent::new();
    component.set_content(MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![album.clone()], 0, 0),
        Some(album),
        "Artist".into(),
        vec![make_item("Artist", "MusicArtist")],
        0,
        vec![("Artist".into(), "2024".into(), "First Album".into())],
        vec![0],
        focused,
        true,
        Some(tracks),
        false,
        None,
    ));
    component.set_inline_track_focus_enabled(true);
    component
}

#[test]
fn up_down_in_track_mode_move_only_track_focus_and_clamp() {
    let (mut model, id) = wide_track_focus_model(3);

    // Enter enters track mode at row 0.
    drive(&mut model, &id, TuiKey::Enter);

    drive(&mut model, &id, TuiKey::Down);
    drive(&mut model, &id, TuiKey::Down);
    drive(&mut model, &id, TuiKey::Down);

    let component = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<MusicWorkspaceComponent>()
        .unwrap();
    // Clamp at the end -- no wrap.
    assert_eq!(component.track_cursor(), Some(2));

    drive(&mut model, &id, TuiKey::Up);
    drive(&mut model, &id, TuiKey::Up);
    drive(&mut model, &id, TuiKey::Up);

    let component = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<MusicWorkspaceComponent>()
        .unwrap();
    // Clamp at the start -- no wrap.
    assert_eq!(component.track_cursor(), Some(0));

    assert_eq!(
        model.app.libs[0].nav_stack.last().unwrap().cursor,
        0,
        "track-mode Up/Down must not move the album cursor"
    );
}

#[test]
fn track_mode_down_does_not_move_track_focus_when_queue_panel_has_focus() {
    // With the Queue panel focused (`context.focused == false`), track-mode
    // Up/Down are unhandled instead of moving the focused track.
    let mut component = component_with_tracks_ctx(false);
    component.set_inline_track_focus_enabled(true);
    let msg = component.on(&Event::Keyboard(TuiKeyEvent {
        code: TuiKey::Down,
        modifiers: TuiKeyModifiers::NONE,
    }));
    assert!(matches!(
        msg,
        Some(Msg::Shell(ShellRequest::GlobalViewKey(key)))
            if key.code == crossterm::event::KeyCode::Down
    ));
    assert_eq!(component.track_cursor(), None);
}

#[test]
fn selecting_music_group_clears_track_focus() {
    let (mut model, id) = wide_track_focus_model(3);
    let mut group2 = make_item("Beta", "MusicArtist");
    group2.id = "group-1".into();
    group2.is_folder = true;
    model.app.libs[0].nav_stack[0].items.push(group2);

    drive(&mut model, &id, TuiKey::Enter);
    model.app.select_music_group(0, 1);
    model.push_music_workspace_content();

    let component = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<MusicWorkspaceComponent>()
        .unwrap();
    assert_eq!(component.track_cursor(), None);
}

#[test]
fn switching_music_group_clears_track_focus() {
    let (mut model, id) = wide_track_focus_model(3);
    let mut group2 = make_item("Beta", "MusicArtist");
    group2.id = "group-1".into();
    group2.is_folder = true;
    model.app.libs[0].nav_stack[0].items.push(group2);

    drive(&mut model, &id, TuiKey::Enter);
    model.app.switch_music_group(0, 1);
    model.push_music_workspace_content();

    let component = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<MusicWorkspaceComponent>()
        .unwrap();
    assert_eq!(component.track_cursor(), None);
}

#[test]
fn up_down_in_track_mode_with_no_cached_tracks_is_noop() {
    // No `push_tracks` -- album_tracks_cache has no entry for "album-1",
    // mirroring "not yet loaded". A focused cursor over an empty track list
    // stays put (the move clamps to zero rows).
    let (mut model, id) = wide_track_focus_model(0);
    drive(&mut model, &id, TuiKey::Enter); // cannot enter: no tracks

    let component = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<MusicWorkspaceComponent>()
        .unwrap();
    assert_eq!(component.track_cursor(), None);
    // The album cursor still owns the keys.
    drive(&mut model, &id, TuiKey::Down);
    let component = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<MusicWorkspaceComponent>()
        .unwrap();
    assert_eq!(component.track_cursor(), None);
}

#[test]
fn escape_in_track_mode_clears_focus_without_go_back() {
    let mut component = component_with_tracks();
    let _ = component.on(&Event::Keyboard(TuiKeyEvent {
        code: TuiKey::Enter,
        modifiers: TuiKeyModifiers::NONE,
    }));
    assert_eq!(component.track_cursor(), Some(0));

    // Esc exits track mode locally without emitting a message.
    let msg = component.on(&Event::Keyboard(TuiKeyEvent {
        code: TuiKey::Esc,
        modifiers: TuiKeyModifiers::NONE,
    }));
    assert_eq!(component.track_cursor(), None);
    assert_eq!(msg, None);
}

#[test]
fn escape_outside_track_mode_still_calls_go_back_unchanged() {
    // `make_music_album_app`'s grouped `["group","album"]` fixture
    // sits at the *root* of the synthetic music-group view (nav_stack
    // len == 2), which `go_back`'s own pre-existing guard already
    // no-ops on ("don't pop when already at the root of a synthetic
    // group view" -- see `go_back`'s doc comment in actions.rs). The
    // regression this proves: outside track mode the Esc key still routes
    // to the exact same `go_back()` call as before, whatever `go_back()`
    // itself does -- demonstrated by comparing `handle_key(Esc)` against
    // calling `go_back()` directly on an identical, freshly-built app.
    let mut via_go_back = make_music_album_app();
    via_go_back.go_back(0);

    let mut via_escape_key = make_music_album_app();
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
fn page_down_in_album_list_mode_pages_by_rendered_rows_with_hero() {
    let mut app = make_music_album_list_app(60, 0);
    push_tracks(&mut app, "album-0", 4);
    render_full_app(&mut app, 100, 40);
    let viewport_rows = app.layout.main.left_area.height as usize;
    assert!(
        viewport_rows >= 19,
        "fixture sanity: expected at least 19 rendered list rows below the hero panel, got {viewport_rows}"
    );
    assert!(app.right_panel_image_renders_allowed());

    let handled = app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));

    assert!(!handled);
    assert!(!app.right_panel_image_renders_allowed());
    // The hero panel above the list shows only title + hint (no tracks),
    // so the list gets more rows. PageDown moves by the rendered row count.
    let cursor_after = app.libs[0].nav_stack.last().unwrap().cursor;
    assert!(
        cursor_after >= 19,
        "PageDown should move by rendered display rows, not raw album count; got cursor {cursor_after}"
    );
}

#[test]
fn page_up_in_album_list_mode_pages_by_rendered_rows_with_hero() {
    let mut app = make_music_album_list_app(60, 35);
    push_tracks(&mut app, "album-35", 4);
    render_full_app(&mut app, 100, 40);
    let viewport_rows = app.layout.main.left_area.height as usize;
    assert!(
        viewport_rows >= 19,
        "fixture sanity: expected at least 19 rendered list rows below the hero panel, got {viewport_rows}"
    );

    let handled = app.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));

    assert!(!handled);
    // The hero panel above the list shows only title + hint (no tracks),
    // so the list gets more rows. PageUp moves by the rendered row count.
    let cursor_after = app.libs[0].nav_stack.last().unwrap().cursor;
    assert!(
        cursor_after <= 35,
        "PageUp should move backwards; got cursor {cursor_after}"
    );
}

#[test]
fn paging_past_display_edges_clamps_in_display_order_not_api_order() {
    let mut app = make_music_album_list_app(3, 0);
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

    let mut app = make_music_album_list_app(3, 1);
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
    // mode is active (see `page_grouped_album_cursor`'s shell gate). The
    // two non-selectable rows paging can still land on while merely
    // *browsing* the album list are: the artist header, and the collapsed
    // action-hint row that sits directly under the selected album.
    let mut down_app = make_music_album_list_app(10, 0);
    render_full_app(&mut down_app, 100, 40);
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

    let mut up_app = make_music_album_list_app(10, 3);
    render_full_app(&mut up_app, 100, 40);
    up_app.layout.main.left_area.height = 4;

    let handled = up_app.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));

    assert!(!handled);
    // The selected artist block contains the header, pinned hint, and every
    // album. With a 4-row page, PageUp from album 3 resolves to album 0
    // rather than leaving the cursor on the non-album header row.
    assert_eq!(up_app.libs[0].nav_stack.last().unwrap().cursor, 0);
}

#[test]
fn two_column_album_navigation_strides_rows_and_crosses_groups() {
    let mut app = make_music_album_list_app(4, 0);
    add_following_artist_albums(&mut app, 2);
    render_full_app(&mut app, 100, 40);
    // Force the two-column layout the movement derives from the pane width
    // (82 is POWER_TWO_COLUMN_THRESHOLD); the 100-wide render above lands
    // at 1 column behind the 40-wide queue column.
    app.layout.main.left_area.width = 82;

    // Down strides one row (cols = 2) within the Alpha group.
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 2);

    // Left/right move by a single album.
    app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 3);

    // Down from Alpha's last row moves by cols (2) in the flat album-only
    // target list, crossing the group boundary without resting on a header.
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 5);

    // Up from the second Beta album moves back by cols (2).
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(app.libs[0].nav_stack.last().unwrap().cursor, 3);
}

#[test]
fn oversized_artist_navigation_reaches_hidden_albums_before_following_artist() {
    let mut app = make_music_album_list_app(60, 0);
    add_following_artist_albums(&mut app, 2);
    render_full_app(&mut app, 100, 40);

    // All 60 Alpha albums are traversed before the cursor leaves the
    // oversized artist; the following artist's first album is reached
    // directly, since the Beta artist header is not a navigation stop
    // for arrow movement (see `grouped_cursor_target`).
    for expected_cursor in 1..=60 {
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(
            app.libs[0].nav_stack.last().unwrap().cursor,
            expected_cursor
        );
    }
}

#[test]
fn page_down_crosses_oversized_artist_window_to_following_artist() {
    let mut app = make_music_album_list_app(60, 59);
    add_following_artist_albums(&mut app, 2);
    render_full_app(&mut app, 100, 40);
    app.layout.main.left_area.height = 1;

    app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));

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
