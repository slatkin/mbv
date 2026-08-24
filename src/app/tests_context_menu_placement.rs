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
use crate::app::components::{ComponentId, ContextMenuComponent, OverlayId};
use crate::app::shell::Model;
use crate::app::tests::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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
        model.app.render(f);
        model.render_context_menu_overlay(f);
    })
    .unwrap();
    let rect = mounted_context_menu_rect(model);
    (rect.x, rect.y)
}

#[test]
fn keyboard_placement_follows_fresh_layout_after_resize() {
    let mut app = library_app();
    app.handle_key(KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE));
    assert!(
        matches!(
            app.pending_overlay,
            Some(super::types_overlay::OverlayRequest::ContextMenu(_))
        ),
        "'.' should open the context menu"
    );

    let mut model = Model::new(app);
    model.sync_modal_requests();

    // A keyboard anchor is resolved from the fresh frame's selected-item rect,
    // so a resize that changes the panel/selection geometry must move the menu.
    let a = render_at(&mut model, 100, 40);
    let b = render_at(&mut model, 60, 24);
    assert_ne!(
        a, b,
        "keyboard placement must follow the fresh layout after resize: {a:?} != {b:?}"
    );

    // Even after shrinking, the menu stays inside the containing panel.
    let panel = model.app.layout.main.left_area;
    let (x, y) = b;
    assert!(
        x >= panel.x && y >= panel.y,
        "menu must stay inside the panel after resize: ({x},{y}) vs {:?}",
        panel
    );
}

#[test]
fn pointer_placement_stays_click_anchored_not_following_selection() {
    let mut app = library_app();
    app.open_context_menu_at(70, 20);

    let mut model = Model::new(app);
    model.sync_modal_requests();

    // Render once and capture the rect placed from the click point.
    let a = render_at(&mut model, 100, 40);

    // Move the underlying selection so the keyboard anchor (selected-item
    // rect) would sit elsewhere; the pointer anchor must not chase it.
    if let Some(ref mut level) = model.app.libs[0].nav_stack.last_mut() {
        level.cursor = level.items.len().saturating_sub(1);
    }
    let b = render_at(&mut model, 100, 40);

    // Same fresh frame size, same click point: the pointer menu does not move
    // when the selection changes.
    assert_eq!(
        a, b,
        "pointer placement must stay click-anchored and ignore the selected item: {a:?} != {b:?}"
    );
}

#[test]
fn context_menu_entries_render_below_the_reserved_top_row() {
    let mut app = library_app();
    app.open_context_menu();

    let mut model = Model::new(app);
    model.sync_modal_requests();

    let backend = TestBackend::new(100, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    // The shell sets the component rect from `AppLayout` and the component
    // paints via `render_context_menu_content` (task 2.5 / 5.3c).
    terminal
        .draw(|f| {
            model.app.render(f);
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
