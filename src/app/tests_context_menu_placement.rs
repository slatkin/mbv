//! Integration tests for context-menu anchor resolution (design §3):
//! keyboard placement must follow the *fresh* frame layout after a resize,
//! and mouse (pointer) placement must remain anchored to the click point.
use super::tests_podcast::add_emby_movie_library;
use super::*;
use crate::app::tests::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn render_at(app: &mut App, width: u16, height: u16) -> (u16, u16) {
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| app.render(f)).unwrap();
    let rect = app
        .layout
        .context_menu_rect
        .expect("context menu should be placed after render");
    (rect.x, rect.y)
}

fn library_app() -> App {
    let mut app = make_app_stub();
    add_emby_movie_library(&mut app);
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app
}

#[test]
fn keyboard_placement_follows_fresh_layout_after_resize() {
    let mut app = library_app();
    app.handle_key(KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE));
    assert!(
        app.context_menu.is_some(),
        "'.' should open the context menu"
    );

    // A keyboard anchor is resolved from the fresh frame's selected-item rect,
    // so a resize that changes the panel/selection geometry must move the menu.
    let a = render_at(&mut app, 100, 40);
    let b = render_at(&mut app, 60, 24);
    assert_ne!(
        a, b,
        "keyboard placement must follow the fresh layout after resize: {a:?} != {b:?}"
    );

    // Even after shrinking, the menu stays inside the containing panel.
    let panel = app.layout.main.left_area;
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

    // Render once and capture the rect placed from the click point.
    let a = render_at(&mut app, 100, 40);

    // Move the underlying selection so the keyboard anchor (selected-item
    // rect) would sit elsewhere; the pointer anchor must not chase it.
    if let Some(ref mut level) = app.libs[0].nav_stack.last_mut() {
        level.cursor = level.items.len().saturating_sub(1);
    }
    let b = render_at(&mut app, 100, 40);

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
    let backend = TestBackend::new(100, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| app.render(f)).unwrap();

    let rect = app.layout.context_menu_rect.unwrap();
    let first_label = terminal.backend().buffer().get(rect.x + 1, rect.y + 1);
    assert_eq!(first_label.symbol(), "P");
}
