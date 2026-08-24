use super::*;
use crate::app::tests::{make_app_stub, make_item, make_items};
use crate::app::{BrowseLevel, ConfirmAction, LibraryTab, PanelFocus, TabSelection};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

pub(super) fn make_library_app() -> App {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.tab = TabSelection::EmbyLibrary(0);

    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.collection_type = "movies".into();
    library.is_folder = true;

    let items = make_items(2);
    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items,
            total_count: 2,
            cursor: 0,
            scroll: 0,
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
        ..LibraryTab::new(library)
    });
    app
}

pub(super) fn make_library_mouse_event(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

#[test]
fn left_panel_movement_saves_position_via_real_key_path() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_library_app();

    let handled = app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

    assert!(!handled);
    // The disk write is deferred (see `save_default_library_position`'s doc
    // comment); force it now so this test can read it back.
    app.flush_library_position_now();
    let position = crate::config::load_library_position_state()
        .libraries
        .get("lib-movies")
        .cloned()
        .expect("saved library");
    assert_eq!(
        position
            .levels
            .first()
            .and_then(|level| level.focused_item_id.as_deref()),
        Some("id1")
    );
}

#[test]
fn left_panel_refresh_clears_saved_position_via_real_key_path() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_library_app();
    let saved_position = crate::config::LibraryPosition {
        levels: vec![crate::config::LibraryPositionLevel {
            parent_id: "lib-movies".into(),
            title: "Saved".into(),
            focused_item_id: Some("id1".into()),
            cursor_index: 1,
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            letter_filter_index: None,
            library_total: None,
        }],
        ..Default::default()
    };
    app.replace_saved_library_position(0, saved_position);

    let handled = app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));

    assert!(!handled);
    assert!(!crate::config::load_library_position_state()
        .libraries
        .contains_key("lib-movies"));
}

#[test]
fn ctrl_r_confirmation_targets_active_library() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.tab = TabSelection::EmbyLibrary(1);

    for (lib_id, title) in [("lib-shows", "Shows"), ("lib-movies", "Movies")] {
        let mut library = make_item(title, "CollectionFolder");
        library.id = lib_id.into();
        library.collection_type = "movies".into();
        library.is_folder = true;
        app.libs.push(LibraryTab {
            nav_stack: vec![BrowseLevel {
                parent_id: lib_id.into(),
                title: title.into(),
                items: make_items(2),
                total_count: 2,
                cursor: 0,
                scroll: 0,
                item_types: Some("Movie".into()),
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
                music_grouping: None,
            }],
            ..LibraryTab::new(library)
        });
    }
    app.replace_saved_library_position(
        1,
        crate::config::LibraryPosition {
            levels: vec![crate::config::LibraryPositionLevel {
                parent_id: "lib-movies".into(),
                title: "Saved".into(),
                focused_item_id: Some("id0".into()),
                cursor_index: 0,
                item_types: Some("Movie".into()),
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                letter_filter_index: None,
                library_total: None,
            }],
            ..Default::default()
        },
    );

    let handled = app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    assert!(!handled);
    assert!(matches!(
        app.pending_overlay.as_ref(),
        Some(crate::app::types_overlay::OverlayRequest::Confirm(modal))
            if matches!(&modal.on_confirm, ConfirmAction::RescanLibrary(_))
    ));

    let action = match app.pending_overlay.as_ref() {
        Some(crate::app::types_overlay::OverlayRequest::Confirm(modal)) => modal.on_confirm.clone(),
        _ => panic!("confirmation request missing"),
    };
    app.apply_confirm_action(action, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(!crate::config::load_library_position_state()
        .libraries
        .contains_key("lib-movies"));
}

#[test]
fn left_panel_mouse_events_save_position() {
    for kind in [
        MouseEventKind::ScrollDown,
        MouseEventKind::Down(MouseButton::Left),
    ] {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut app = make_library_app();
        app.layout.main.left_area = Rect {
            x: 10,
            y: 5,
            width: 20,
            height: 5,
        };

        app.handle_mouse(make_library_mouse_event(kind, 12, 6));

        app.flush_library_position_now();
        let position = crate::config::load_library_position_state()
            .libraries
            .get("lib-movies")
            .cloned()
            .unwrap_or_else(|| panic!("saved library for mouse event kind {kind:?}"));
        assert_eq!(
            position
                .levels
                .first()
                .and_then(|level| level.focused_item_id.as_deref()),
            Some("id1"),
            "mouse event kind {kind:?} did not save the expected focused item"
        );
    }
}

#[test]
fn mouse_click_on_already_selected_folder_row_is_a_noop() {
    // Regression test: clicking a folder row (e.g. a TV series) that is
    // already the selected/highlighted row must not re-trigger the
    // Enter-emulating drill-in (`select`/`activate_recursive_album`).
    // Before this fix, `click_set_cursor` returning `true` was
    // sufficient to drill in even when the click landed on the row
    // already under the cursor, producing an unrequested navigation.
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_library_app();
    for item in app.libs[0].nav_stack[0].items.iter_mut() {
        item.is_folder = true;
    }
    app.libs[0].nav_stack[0].cursor = 0;
    app.layout.main.left_area = Rect {
        x: 10,
        y: 5,
        width: 20,
        height: 5,
    };

    // Row 5 -> click_y 0 -> item index 0, the row already selected.
    app.handle_mouse(make_library_mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        12,
        5,
    ));

    assert_eq!(
        app.libs[0].nav_stack.len(),
        1,
        "re-clicking the already-selected folder row must not drill in"
    );
    assert_eq!(app.libs[0].nav_stack[0].cursor, 0);
}

#[test]
fn mouse_click_on_a_different_folder_row_only_focuses_it() {
    // A single click never drills into a folder -- it only moves the
    // cursor/highlight. Drilling in is a double-click gesture.
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_library_app();
    for item in app.libs[0].nav_stack[0].items.iter_mut() {
        item.is_folder = true;
    }
    app.libs[0].nav_stack[0].cursor = 0;
    app.layout.main.left_area = Rect {
        x: 10,
        y: 5,
        width: 20,
        height: 5,
    };

    // Row 6 -> click_y 1 -> item index 1, not the previously-selected row.
    app.handle_mouse(make_library_mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        12,
        6,
    ));

    assert_eq!(
        app.libs[0].nav_stack.len(),
        1,
        "a single click on a different folder row must not drill in"
    );
    assert_eq!(app.libs[0].nav_stack[0].cursor, 1);
}

#[test]
fn double_click_on_a_folder_row_drills_in() {
    // The second click of a double-click on a folder row is the
    // activation gesture: it must drill into the folder just like Enter.
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_library_app();
    for item in app.libs[0].nav_stack[0].items.iter_mut() {
        item.is_folder = true;
    }
    app.libs[0].nav_stack[0].cursor = 0;
    app.layout.main.left_area = Rect {
        x: 10,
        y: 5,
        width: 20,
        height: 5,
    };

    // Two Down(Left) events at the same cell, back to back: the first
    // focuses row 1, the second is the activation.
    let click = make_library_mouse_event(MouseEventKind::Down(MouseButton::Left), 12, 6);
    app.handle_mouse(click);
    assert_eq!(app.libs[0].nav_stack[0].cursor, 1, "first click focuses");
    app.handle_mouse(click);

    assert_eq!(
        app.libs[0].nav_stack.len(),
        2,
        "double-click on a folder row should drill in"
    );
    assert_eq!(app.libs[0].nav_stack[0].cursor, 1);
}

#[test]
fn movie_enter_and_parent_double_click_match_narrow_and_wide() {
    for wide in [false, true] {
        let mut keyboard = make_library_app();
        keyboard.layout.main.left_area = Rect::new(10, 5, 20, 5);
        if wide {
            keyboard.layout.main.movies_wide_right_area = Rect::new(40, 0, 20, 15);
        }
        assert_eq!(keyboard.layout.main.is_wide_movies_active(), wide);

        keyboard.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(keyboard.status, "Emby is unavailable");
        assert!(!matches!(
            keyboard.pending_overlay.as_ref(),
            Some(crate::app::types_overlay::OverlayRequest::SelectionModal(_))
        ));

        let mut mouse = make_library_app();
        mouse.layout.main.left_area = Rect::new(10, 5, 20, 5);
        if wide {
            mouse.layout.main.movies_wide_right_area = Rect::new(40, 0, 20, 15);
        }
        let click = make_library_mouse_event(MouseEventKind::Down(MouseButton::Left), 12, 6);
        mouse.handle_mouse(click);
        mouse.handle_mouse(click);

        assert_eq!(
            mouse.status, keyboard.status,
            "Movie double-click must use the same activation as Enter (wide={wide})"
        );
        assert!(!matches!(
            mouse.pending_overlay.as_ref(),
            Some(crate::app::types_overlay::OverlayRequest::SelectionModal(_))
        ));
    }
}

#[test]
fn mouse_click_on_a_new_series_row_only_moves_the_cursor() {
    // Regression test: clicking a *different, not-yet-selected* TV
    // Series row must not drill in either. Series rows are always
    // `is_folder`, but Enter on one opens the inline series-selection
    // detail (`enter_series_selection`), not a generic folder browse --
    // a mouse click has no equivalent gesture for that, so it should
    // only move the cursor/highlight onto the clicked row.
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_library_app();
    for item in app.libs[0].nav_stack[0].items.iter_mut() {
        item.is_folder = true;
        item.item_type = "Series".into();
    }
    app.libs[0].library.collection_type = "tvshows".into();
    app.libs[0].nav_stack[0].cursor = 0;
    app.layout.main.left_area = Rect {
        x: 10,
        y: 5,
        width: 20,
        height: 5,
    };

    // Row 6 -> click_y 1 -> item index 1, not the previously-selected row.
    app.handle_mouse(make_library_mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        12,
        6,
    ));

    assert_eq!(
        app.libs[0].nav_stack.len(),
        1,
        "clicking a new Series row must not drill into a folder browse"
    );
    assert_eq!(app.libs[0].nav_stack[0].cursor, 1);
}

#[test]
fn breadcrumb_click_saves_position() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_library_app();
    app.libs[0].nav_stack.push(BrowseLevel {
        parent_id: "folder-1".into(),
        title: "Folder".into(),
        items: make_items(2),
        total_count: 2,
        cursor: 1,
        scroll: 0,
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        music_grouping: None,
    });
    app.layout.main.breadcrumbs = vec![(10, 20, 2, 1)];

    app.handle_mouse(make_library_mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        12,
        2,
    ));

    app.flush_library_position_now();
    let position = crate::config::load_library_position_state()
        .libraries
        .get("lib-movies")
        .cloned()
        .expect("saved library");
    assert_eq!(position.levels.len(), 1);
    assert_eq!(app.libs[0].nav_stack.len(), 1);
}

#[test]
fn mouse_tab_selection_from_queue_focus_applies_restore_result() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Queue;
    app.layout.tabs_area = Rect {
        x: 0,
        y: 0,
        width: 29,
        height: 1,
    };

    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.collection_type = "movies".into();
    library.is_folder = true;
    app.libs.push(LibraryTab::new(library));
    let position = crate::config::LibraryPosition {
        levels: vec![crate::config::LibraryPositionLevel {
            parent_id: "lib-movies".into(),
            title: "Power".into(),
            focused_item_id: Some("id1".into()),
            cursor_index: 1,
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            letter_filter_index: None,
            library_total: None,
        }],
        ..Default::default()
    };
    app.replace_saved_library_position(0, position.clone());

    app.handle_mouse(make_library_mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        8,
        0,
    ));

    assert_eq!(app.tab, TabSelection::EmbyLibrary(0));
    assert_eq!(app.panel_focus, PanelFocus::Library);
    assert_eq!(app.libs[0].nav_stack.len(), 1);
    assert!(app.libs[0].nav_stack[0].loading);

    app.handle_lib_event(crate::app::LibEvent::RestoreLibraryPosition {
        lib_idx: 0,
        requested_position: position.clone(),
        position,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Power restored".into(),
            items: make_items(2),
            total_count: 2,
            cursor: 1,
            scroll: 0,
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
    });

    assert_eq!(app.libs[0].nav_stack[0].title, "Power restored");
    assert!(!app.libs[0].nav_stack[0].loading);
}
