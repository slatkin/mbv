//! Integration tests for context-menu anchor resolution (design §3):
//! keyboard placement must follow the *fresh* frame layout after a resize,
//! and mouse (pointer) placement must remain anchored to the click point.
//!
//! The menu is now an Interactive Component mounted by the shell from
//! `app.pending_overlay`; the shell recomputes its rect from `AppLayout`
//! each frame (task 5.3c). These tests drive `Model::render_context_menu_overlay`,
//! which sets the component's rect, then read `ContextMenuComponent::menu_rect()`.
use super::tests_podcast::add_emby_movie_library;
use super::*;
use crate::app::components::{
    ComponentId, ContextMenuComponent, HomeComponent, OverlayId, ShellRequest,
};
use crate::app::shell::Model;
use crate::app::tests::*;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

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
        model.app.compose_base_frame(f, None);
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

/// Task 5.3d, Home menu-placement geometry: when Home is the active
/// destination with Library focus, the shell places the context menu from the
/// mounted `HomeComponent`'s own painted geometry — never the legacy
/// `AppLayout.left_area`/`selected_item_rect` copies. To prove the source, the
/// legacy copies are poisoned far outside the panel and the menu must still
/// land exactly where the component's paint implies, while a fallback to the
/// poisoned rect would land elsewhere. Paint only the Home component (and the
/// overlay), never the legacy `App::render` underpaint, so the poisoned legacy
/// copies stay stale for the whole placement.
#[test]
fn home_menu_uses_component_painted_geometry_not_poisoned_legacy_layout() {
    use crate::app::types_context_menu::ContextMenu;
    let _guard = crate::config::TestStateDirGuard::new();

    let mut model = crate::app::shell::Model::new(make_app_stub());
    model.app.tab = TabSelection::Home;
    model.app.panel_focus = PanelFocus::Library;
    model.home_content.continue_items = make_items(5);
    model.handle_home_request(ShellRequest::HomeContextMenu {
        home_cw_selected: model.home_continue_watching_selected(),
        cw_item: model.home_cw_item(),
    });
    assert!(
        matches!(
            model.app.pending_overlay,
            Some(super::types_overlay::OverlayRequest::ContextMenu(_))
        ),
        "'.' should open the context menu"
    );

    model.sync_modal_requests();
    model.push_home_content();
    model.app.layout.main.home_area = Rect::new(0, 0, 80, 24);

    let backend = TestBackend::new(80, 24);
    let mut term = Terminal::new(backend).unwrap();
    // Paint only the Home component so its `view()` produces the authoritative
    // placed geometry while the legacy `AppLayout` copies stay untouched.
    term.draw(|f| model.render_home_component(f)).unwrap();

    // Capture the component-painted geometry the shell must anchor to, then
    // poison the corresponding legacy copies far outside the panel.
    let home = model
        .application
        .get_component(&ComponentId::Home)
        .and_then(|c| c.as_any().downcast_ref::<HomeComponent>())
        .expect("Home component mounted");
    let (panel, selected) = home.menu_placement_geometry();
    model.app.layout.main.left_area = Rect::new(0, 0, 200, 200);
    model.app.layout.main.selected_item_rect = Some(Rect::new(500, 500, 1, 1));

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

    // A fallback to the poisoned legacy geometry would land the menu at the
    // panel's bottom-right corner instead of the component's placement.
    let (px, py) = ContextMenu::place(
        Rect::new(0, 0, 200, 200),
        size,
        Some(&Rect::new(500, 500, 1, 1)),
        None,
    );
    assert_ne!(
        (rect.x, rect.y),
        (px, py),
        "menu must not fall back to the poisoned AppLayout geometry"
    );
}

#[test]
fn context_menu_entries_render_below_the_reserved_top_row() {
    let mut app = library_app();
    app.open_context_menu(false, None);

    let mut model = Model::new(app);
    model.sync_modal_requests();

    let backend = TestBackend::new(100, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    // The shell sets the component rect from `AppLayout` and the component
    // paints via `render_context_menu_content` (task 2.5 / 5.3c).
    terminal
        .draw(|f| {
            model.app.compose_base_frame(f, None);
            model.render_context_menu_overlay(f);
        })
        .unwrap();

    let rect = mounted_context_menu_rect(&model);
    let first_label = terminal
        .backend()
        .buffer()
        .cell((rect.x + 1, rect.y + 1))
        .unwrap();
    assert_eq!(first_label.symbol(), "P");
}
