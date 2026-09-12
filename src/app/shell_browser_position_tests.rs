use super::super::*;
use super::test_support::*;
use crate::app::components::Msg;
use crate::app::render::make_movie_app;
use crate::app::tests::make_item;
use crate::app::PanelMode;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

#[test]
fn shell_mounts_and_syncs_the_generic_emby_browser() {
    let mut model = Model::new(make_movie_app());
    model.sync_emby_browser();
    model.sync_active_destination();
    let id = model.emby_browser_id.clone().expect("browser mounted");
    let message = {
        model
            .application
            .get_component_mut(&id)
            .unwrap()
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Down,
                modifiers: KeyModifiers::NONE,
            }))
    };
    // Down routes through the typed browser request.
    let Some(Msg::Shell(ShellRequest::BrowserCursorIndex { index })) = message else {
        panic!("browser movement should emit the typed index request");
    };
    assert_eq!(index, 1, "Down must resolve to item 1");
    model.handle_browser_request(ShellRequest::BrowserCursorIndex { index });
    model.sync_emby_browser();
    model.sync_active_destination();
    assert_eq!(
        model
            .application
            .get_component(&id)
            .unwrap()
            .as_any()
            .downcast_ref::<BrowserComponent>()
            .unwrap()
            .cursor(),
        1
    );
    assert_eq!(model.app.libs[0].nav_stack[0].resting().cursor(), 1);
    assert!(model
        .application
        .get_component(&id)
        .unwrap()
        .as_any()
        .downcast_ref::<BrowserComponent>()
        .is_some());
}

/// Task 5.3d, Emby browser local navigation through the Model boundary:
/// the focused `BrowserComponent` returns typed `BrowserCursorIndex`
/// requests in place of the raw legacy key, and the shell derives the
/// active Emby library index from its own tab state and runs the same
/// `App` cursor methods the legacy `handle_lib_key` movement arms call.
/// The App cursor must move through that typed path (never a raw
/// cursor-field write). Task 5.8 deleted the Grid presentation as
/// unreachable (design D13), so every painted list is one-column: Down
/// strides one item, Home/End jump to the first/last item, and Left/
/// Right/h/l stay unbound locally (raw key consumed by the component
/// without movement, App cursor unchanged).
#[test]
fn shell_emby_browser_movement_drives_app_cursor_via_typed_requests() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = browser_app_with_flat_movies(10);
    // LibraryOnly hides the queue column so the library pane spans the
    // full window and clears the two-column threshold at render width;
    // the panel mode is a state the app already supports, not hand-set
    // layout rects (the whole frame is painted into a TestBackend).
    app.panel_mode = PanelMode::LibraryOnly;
    let mut model = Model::new(app);
    model.sync_emby_browser();
    model.sync_active_destination();
    let id = model.emby_browser_id.clone().expect("browser mounted");

    // Paint the App and the mounted browser at 150 columns: both derive
    // the same two-column stride from the same painted geometry (the
    // generic library never takes the wide-Movies 1-column rail).
    render_browser_model(&mut model, 150, 40);
    model.sync_emby_browser();
    model.sync_active_destination();

    // Down: the focused component returns a typed `BrowserCursorIndex`
    // (one selectable row, in place of the raw key), and the shell applies
    // it — the one-column Inline presentation strides the App cursor one
    // item, exactly like the legacy arm.
    let Some(Msg::Shell(ShellRequest::BrowserCursorIndex { index })) =
        drive_browser_key(&mut model, &id, Key::Down, KeyModifiers::NONE)
    else {
        panic!("focused browser Down must emit BrowserCursorIndex, got no typed request");
    };
    assert_eq!(index, 1, "Down must resolve to the next item");
    let navigation_before = model.app.last_nav_at;
    model.app.library_position_dirty = false;
    model.handle_browser_request(ShellRequest::BrowserCursorIndex { index });
    assert_eq!(
        model.app.libs[0].nav_stack[0].resting().cursor(),
        1,
        "Down must apply the component-resolved index"
    );
    assert!(
        model.app.library_position_dirty,
        "cursor application must persist the library position"
    );
    assert!(
        model.app.last_nav_at > navigation_before,
        "cursor application must mark library navigation"
    );
    assert_eq!(
        model.app.library_position_state.libraries["lib-films"].levels[0].cursor_index, 1,
        "the single cursor application must persist the resolved index"
    );
    let component_cursor = model
        .application
        .get_component(&id)
        .unwrap()
        .as_any()
        .downcast_ref::<BrowserComponent>()
        .unwrap()
        .cursor();
    assert_eq!(
        component_cursor, 1,
        "component cursor remains locally resolved"
    );

    // End/Home jump the App cursor to the last/first item through
    // `App::jump_lib_cursor`.
    let Some(Msg::Shell(ShellRequest::BrowserCursorIndex { index })) =
        drive_browser_key(&mut model, &id, Key::End, KeyModifiers::NONE)
    else {
        panic!("focused browser End must emit BrowserCursorIndex, got no typed request");
    };
    assert_eq!(index, 9, "End must resolve to the last item");
    model.handle_browser_request(ShellRequest::BrowserCursorIndex { index });
    assert_eq!(model.app.libs[0].nav_stack[0].resting().cursor(), 9);
    let Some(Msg::Shell(ShellRequest::BrowserCursorIndex { index })) =
        drive_browser_key(&mut model, &id, Key::Home, KeyModifiers::NONE)
    else {
        panic!("focused browser Home must emit BrowserCursorIndex, got no typed request");
    };
    assert_eq!(index, 0, "Home must resolve to the first item");
    model.handle_browser_request(ShellRequest::BrowserCursorIndex { index });
    assert_eq!(model.app.libs[0].nav_stack[0].resting().cursor(), 0);

    // Left/Right/h/l stay unbound locally with no movement request (task
    // 5.8 deleted the Grid presentation's column navigation, design D13),
    // leaving the component and App cursors unchanged.
    model.app.panel_mode = PanelMode::Both;
    render_browser_model(&mut model, 100, 40);
    model.sync_emby_browser();
    model.sync_active_destination();
    for key in [Key::Left, Key::Right, Key::Char('h'), Key::Char('l')] {
        assert_eq!(
            drive_browser_key(&mut model, &id, key, KeyModifiers::NONE),
            None,
            "focused {key:?} must stay unclaimed: the list is one-column"
        );
    }
    let comp_cursor = model
        .application
        .get_component(&id)
        .unwrap()
        .as_any()
        .downcast_ref::<BrowserComponent>()
        .unwrap()
        .cursor();
    assert_eq!(
        comp_cursor, 0,
        "unbound Left/Right/h/l must not move the component cursor"
    );
    assert_eq!(
        model.app.libs[0].nav_stack[0].resting().cursor(),
        0,
        "unbound Left/Right/h/l must not move the App cursor"
    );
    // Down keeps its typed stride of one in the one-column list.
    let Some(Msg::Shell(ShellRequest::BrowserCursorIndex { index })) =
        drive_browser_key(&mut model, &id, Key::Down, KeyModifiers::NONE)
    else {
        panic!("focused browser Down must still emit BrowserCursorIndex, got no typed request");
    };
    assert_eq!(index, 1);
    model.handle_browser_request(ShellRequest::BrowserCursorIndex { index });
    assert_eq!(
        model.app.libs[0].nav_stack[0].resting().cursor(),
        1,
        "Down must stride the App cursor one item"
    );
}

#[test]
fn browser_navigation_persists_live_scroll_at_level_boundaries() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut model = Model::new(browser_app_with_folder_and_movie());
    model.app.libs[0].nav_stack[0].set_resting_scroll(7);
    model.sync_emby_browser();
    model.sync_active_destination();
    let mut folder = make_item("Folder A", "CollectionFolder");
    folder.id = "folder-a".into();
    folder.is_folder = true;

    // Task 4.2: the restored position lands in the one shared owner, whose
    // scroll is clamped to its own two-row flow. Drill-in persists that live
    // owner scroll over the stale resting value (task 4.2 removed the second
    // control that used to absorb the seed invisibly).
    model.handle_browser_request(ShellRequest::BrowserActivate { item: folder });
    assert_eq!(model.app.libs[0].nav_stack.len(), 2);
    assert_eq!(model.app.libs[0].nav_stack[0].resting().scroll(), 1);
    assert_eq!(
        model.app.library_position_state.libraries["lib-movies"].levels[0].cursor_index,
        0
    );

    model.app.libs[0].nav_stack[1].set_resting_scroll(3);
    model.sync_emby_browser();
    model.sync_active_destination();
    // Back persists the owner's live scroll for the child level, then restores
    // the parent's own live scroll saved at drill-in (task 4.2: one owner, so
    // the parent's restored viewport is its real last position).
    model.handle_browser_request(ShellRequest::BrowserBack);
    assert_eq!(model.app.libs[0].nav_stack.len(), 1);
    assert_eq!(model.app.libs[0].nav_stack[0].resting().scroll(), 1);
}

#[test]
fn teardown_flush_captures_live_browser_scroll_without_navigation() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut model = Model::new(browser_app_with_folder_and_movie());
    model.app.libs[0].nav_stack[0].set_resting_scroll(6);
    model.sync_emby_browser();
    model.sync_active_destination();
    let id = model.emby_browser_id.clone().expect("browser mounted");
    model
        .application
        .get_component_mut(&id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<BrowserComponent>()
        .unwrap()
        .apply_position(0, 6);
    model.app.libs[0].nav_stack[0].set_resting_scroll(0);

    model.persist_emby_browser_scroll_for_active_library();
    model.app.flush_library_position_now();

    // Task 4.2: the teardown flush captures the live scroll of the one shared
    // owner — the explicit `apply_position(0, 6)` seed clamped to the owner's
    // two-row flow — overwriting the stale resting 0.
    assert_eq!(model.app.libs[0].nav_stack[0].resting().scroll(), 1);
    assert!(!model.app.library_position_dirty);
}
