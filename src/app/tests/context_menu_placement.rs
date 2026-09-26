//! Integration tests for context-menu anchor resolution (design §3):
//! keyboard placement must follow the *fresh* frame layout after a resize,
//! and mouse (pointer) placement must remain anchored to the click point.
//!
//! The menu is now an Interactive Component mounted by the shell from
//! `app.pending_overlay`; the shell recomputes its rect from `AppLayout`
//! each frame (task 5.3c). These tests drive `Model::render_context_menu_overlay`,
//! which sets the component's rect, then read `ContextMenuComponent::menu_rect()`.
use super::podcast::add_emby_movie_library;
use super::*;
use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::{ComponentId, ContextMenuComponent, Msg, OverlayId};
use ratatui::layout::Rect;

fn library_app() -> App {
    let mut app = make_app_stub();
    add_emby_movie_library(&mut app);
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app
}

fn mounted_context_menu_rect(model: &Model) -> Rect {
    let id = ComponentId::Overlay(OverlayId::ContextMenu);
    model
        .application
        .get_component(&id)
        .expect("context menu mounted")
        .as_any()
        .downcast_ref::<ContextMenuComponent>()
        .expect("context menu type")
        .menu_rect()
}

fn render_at(model: &mut Model, width: u16, height: u16) -> (u16, u16) {
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        model.app.compose_root_frame(f);
        model.render_context_menu_overlay(f);
    })
    .unwrap();
    let rect = mounted_context_menu_rect(model);
    (rect.x, rect.y)
}

#[test]
fn pointer_placement_stays_click_anchored_not_following_selection() {
    let mut app = library_app();
    app.open_context_menu_at(70, 20, false, None);

    let mut model = Model::new(app);
    model.sync_modal_requests();

    // Render once and capture the rect placed from the click point.
    let a = render_at(&mut model, 100, 40);

    // Move the underlying selection so the keyboard anchor (selected-item
    // rect) would sit elsewhere; the pointer anchor must not chase it.
    if let Some(ref mut level) = model.app.libs[0].nav_stack.last_mut() {
        level.set_resting_cursor(level.items.len().saturating_sub(1));
    }
    let b = render_at(&mut model, 100, 40);

    // Same fresh frame size, same click point: the pointer menu does not move
    // when the selection changes.
    assert_eq!(
        a, b,
        "pointer placement must stay click-anchored and ignore the selected item: {a:?} != {b:?}"
    );
}

/// Task 5.3d + 5.11, Home menu-placement geometry: when the Home owner is
/// the migrated active destination with Library focus, the shell places the
/// context menu from the mounted `LibraryPanel`'s own painted geometry —
/// never the legacy `AppLayout.left_area`/`selected_item_rect` copies. To
/// prove the source, the legacy copies are poisoned far outside the panel
/// and the menu must still land exactly where the panel's paint implies,
/// while a fallback to the poisoned rect would land elsewhere. Paint only
/// the panel (and the overlay), never the legacy `App::render` underpaint,
/// so the poisoned legacy copies stay stale for the whole placement.
#[test]
fn home_menu_uses_component_painted_geometry_not_poisoned_legacy_layout() {
    use crate::app::state::types::context_menu::ContextMenu;
    let _guard = crate::config::TestStateDirGuard::new();

    let mut model = crate::app::shell::Model::new(make_app_stub());
    model.app.tab = TabSelection::Home;
    model.app.panel_focus = PanelFocus::Library;
    model.home_content.continue_items = make_items(5);
    model.handle_terminal_message(
        Msg::Shell(Box::new(
            crate::app::components::ShellRequest::RowContextMenu(
                crate::app::state::types::context_menu::ContextMenuTargets::Home(vec![
                    crate::app::components::msg::HomeRowTarget {
                        item_id: Some("id0".into()),
                        source: None,
                        from_continue_watching: true,
                    },
                ]),
                None,
            ),
        )),
        &mut false,
        &mut false,
    );
    assert!(
        matches!(
            model.app.pending_overlay,
            Some(crate::app::state::types::overlay::OverlayRequest::ContextMenu(_))
        ),
        "'.' should open the context menu"
    );

    model.sync_modal_requests();
    model.push_home_content();
    // The panel paints from `RootFrame.library` (task 5.9); publish it
    // directly, wide enough for the Wide skeleton's split.
    let library_area = Rect::new(0, 0, 100, 30);
    model.app.layout.root_frame.library = Some(library_area);
    // The mounted panel only paints its active owner's surface; the shell's
    // sync pass points it at the Home owner (`sync_library_panel`), which
    // this test drives directly rather than through `sync_mounted_surfaces`.
    model.sync_library_panel();

    let backend = TestBackend::new(100, 30);
    let mut term = Terminal::new(backend).unwrap();
    // Paint only the panel so its `view()` produces the authoritative placed
    // geometry while the legacy `AppLayout` copies stay untouched.
    let area = model.app.layout.root_frame.library.unwrap();
    term.draw(|f| model.render_library_panel_at(f, area))
        .unwrap();

    // Capture the panel-painted geometry the shell must anchor to, then
    // poison the corresponding legacy copies far outside the panel.
    let painted = model
        .application
        .get_component(&ComponentId::Library)
        .and_then(|c| c.as_any().downcast_ref::<LibraryPanel>())
        .expect("Library panel mounted")
        .menu_geometry()
        .expect("panel painted geometry");
    let (panel, selected) = painted;
    model.app.layout.left_area = Rect::new(0, 0, 200, 200);

    term.draw(|f| model.render_context_menu_overlay(f)).unwrap();

    let rect = mounted_context_menu_rect(&model);

    let id = ComponentId::Overlay(OverlayId::ContextMenu);
    let (size, anchor) = {
        let comp = model
            .application
            .get_component(&id)
            .expect("context menu mounted")
            .as_any()
            .downcast_ref::<ContextMenuComponent>()
            .expect("context menu type");
        (ContextMenu::rendered_size(comp.entries()), comp.anchor())
    };
    // Keyboard anchor: `SelectedItem(Library)` resolved to the component's
    // painted panel + selected rect (no pointer).
    assert!(matches!(
        anchor,
        ContextMenuAnchor::SelectedItem(PanelFocus::Library)
    ));
    let (ex, ey) = ContextMenu::place(panel, size, selected.as_ref(), None);
    let expected = Rect::new(ex, ey, size.0, size.1);
    assert_eq!(
        rect, expected,
        "menu must anchor to the component-painted Home geometry, got {rect:?}"
    );

    // A fallback to the poisoned legacy `left_area` (the shell mirror no
    // longer carries a `selected_item_rect` to poison at all) would land the
    // menu at the panel's bottom-right corner instead of the component's
    // placement.
    let (px, py) = ContextMenu::place(Rect::new(0, 0, 200, 200), size, None, None);
    assert_ne!(
        (rect.x, rect.y),
        (px, py),
        "menu must not fall back to the poisoned AppLayout geometry"
    );
}

/// Task 5.11 review correction: the Narrow-mode counterpart of
/// `home_menu_uses_component_painted_geometry_not_poisoned_legacy_layout`.
/// `LibraryPanel::menu_geometry()` only read `wide_geometry`, so when Home is
/// active in Narrow mode with `PanelFocus::Library` it returned `None` and the
/// shell fell through to the poisoned legacy `AppLayout` copies. Force the
/// Narrow skeleton (width below `TWO_COLUMN_THRESHOLD`) and prove the menu
/// still anchors to the panel's own Narrow-painted geometry.
#[test]
fn home_menu_uses_component_painted_geometry_not_poisoned_legacy_layout_narrow() {
    use crate::app::state::types::context_menu::ContextMenu;
    let _guard = crate::config::TestStateDirGuard::new();

    let mut model = crate::app::shell::Model::new(make_app_stub());
    model.app.tab = TabSelection::Home;
    model.app.panel_focus = PanelFocus::Library;
    model.home_content.continue_items = make_items(5);
    model.handle_terminal_message(
        Msg::Shell(Box::new(
            crate::app::components::ShellRequest::RowContextMenu(
                crate::app::state::types::context_menu::ContextMenuTargets::Home(vec![
                    crate::app::components::msg::HomeRowTarget {
                        item_id: Some("id0".into()),
                        source: None,
                        from_continue_watching: true,
                    },
                ]),
                None,
            ),
        )),
        &mut false,
        &mut false,
    );
    assert!(
        matches!(
            model.app.pending_overlay,
            Some(crate::app::state::types::overlay::OverlayRequest::ContextMenu(_))
        ),
        "'.' should open the context menu"
    );

    model.sync_modal_requests();
    model.push_home_content();
    // Narrow enough (below `TWO_COLUMN_THRESHOLD`) to force the Narrow
    // skeleton instead of the Wide split.
    let library_area = Rect::new(0, 0, 60, 30);
    model.app.layout.root_frame.library = Some(library_area);
    model.sync_library_panel();

    let backend = TestBackend::new(60, 30);
    let mut term = Terminal::new(backend).unwrap();
    // Paint only the panel so its `view()` produces the authoritative placed
    // geometry while the legacy `AppLayout` copies stay untouched.
    let area = model.app.layout.root_frame.library.unwrap();
    term.draw(|f| model.render_library_panel_at(f, area))
        .unwrap();

    // Capture the panel-painted geometry the shell must anchor to, then
    // poison the corresponding legacy copies far outside the panel.
    let painted = model
        .application
        .get_component(&ComponentId::Library)
        .and_then(|c| c.as_any().downcast_ref::<LibraryPanel>())
        .expect("Library panel mounted")
        .menu_geometry()
        .expect("panel painted geometry");
    let (panel, selected) = painted;
    // The non-Wide anchor is the Wide pane's panel rect (unify-narrow 6.1):
    // the pane the frame painted and retained, not the row-flow inset the
    // pre-parity path anchored to.
    let narrow = model
        .application
        .get_component(&ComponentId::Library)
        .and_then(|c| c.as_any().downcast_ref::<LibraryPanel>())
        .and_then(LibraryPanel::test_narrow_geometry)
        .expect("the non-Wide frame painted");
    assert_eq!(
        panel, narrow.list_panel,
        "the anchor is the painted Browser pane"
    );
    assert_ne!(
        panel, narrow.list_area,
        "the anchor is the pane, not the row-flow inset"
    );
    model.app.layout.left_area = Rect::new(0, 0, 200, 200);

    term.draw(|f| model.render_context_menu_overlay(f)).unwrap();

    let rect = mounted_context_menu_rect(&model);

    let id = ComponentId::Overlay(OverlayId::ContextMenu);
    let (size, anchor) = {
        let comp = model
            .application
            .get_component(&id)
            .expect("context menu mounted")
            .as_any()
            .downcast_ref::<ContextMenuComponent>()
            .expect("context menu type");
        (ContextMenu::rendered_size(comp.entries()), comp.anchor())
    };
    // Keyboard anchor: `SelectedItem(Library)` resolved to the component's
    // painted panel + selected rect (no pointer).
    assert!(matches!(
        anchor,
        ContextMenuAnchor::SelectedItem(PanelFocus::Library)
    ));
    let (ex, ey) = ContextMenu::place(panel, size, selected.as_ref(), None);
    let expected = Rect::new(ex, ey, size.0, size.1);
    assert_eq!(
        rect, expected,
        "menu must anchor to the component-painted Narrow Home geometry, got {rect:?}"
    );

    // A fallback to the poisoned legacy `left_area` (the shell mirror no
    // longer carries a `selected_item_rect` to poison at all) would land the
    // menu at the panel's bottom-right corner instead of the component's
    // placement.
    let (px, py) = ContextMenu::place(Rect::new(0, 0, 200, 200), size, None, None);
    assert_ne!(
        (rect.x, rect.y),
        (px, py),
        "menu must not fall back to the poisoned AppLayout geometry"
    );
}
