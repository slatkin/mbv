use super::*;
use crate::app::tests::{make_app_stub, make_item, make_items};
use crate::app::{BrowseLevel, ConfirmAction, LibraryTab, PanelFocus, TabSelection};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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
    }
}
