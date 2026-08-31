use super::components::{
    ComponentId, ContextMenuComponent, Msg, QueueComponent, QueueRequest,
};
use super::tests::{make_built_app, make_item};
use super::types_context_menu::{ContextMenu, ContextMenuAnchor};
use super::{PanelFocus, QueueScope};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::component::Component;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};
use crate::app::shell::Model;

#[test]
fn shell_frame_publishes_queue_geometry_to_queue_component_and_layout() {
    let mut app = make_built_app();
    app.player_tab.set_items(
        vec![make_item("first", "Movie"), make_item("second", "Movie")],
        0,
    );
    app.panel_focus = PanelFocus::Queue;
    app.set_queue_scope(QueueScope::Local);

    let mut model = Model::new(app);
    model.sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal.draw(|frame| model.draw_frame(frame, false, false)).unwrap();

    let layout_area = model.app.layout.main.queue_area;
    assert!(layout_area.height > 0, "shell must publish a usable queue area");
    let selected = model
        .application
        .get_component(&ComponentId::Queue)
        .and_then(|component| component.as_any().downcast_ref::<QueueComponent>())
        .and_then(QueueComponent::selected_row_rect)
        .expect("queue component must publish selected row geometry");
    assert!(
        layout_area.x <= selected.x
            && layout_area.y <= selected.y
            && selected.right() <= layout_area.right()
            && selected.bottom() <= layout_area.bottom(),
        "selected row must be inside queue area"
    );
    assert_eq!(model.app.layout.main.queue_selected_item_rect, Some(selected));
}

#[test]
fn shell_frame_uses_queue_component_geometry_for_keyboard_context_menu_anchor() {
    let mut app = make_built_app();
    app.player_tab.set_items(
        vec![make_item("first", "Movie"), make_item("second", "Movie")],
        0,
    );
    app.panel_focus = PanelFocus::Queue;
    app.set_queue_scope(QueueScope::Local);

    let mut model = Model::new(app);
    model.sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal.draw(|frame| model.draw_frame(frame, false, false)).unwrap();

    let queue_id = ComponentId::Queue;
    let message = model
        .application
        .get_component_mut(&queue_id)
        .expect("queue mounted")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }))
        .expect("queue cursor movement emits a request");
    let mut resize_music = false;
    let mut resize_tv = false;
    model.handle_terminal_message(
        message,
        Some(&queue_id),
        &mut resize_music,
        &mut resize_tv,
    );
    terminal.draw(|frame| model.draw_frame(frame, false, false)).unwrap();

    let queue_selected = model.app.layout.main.queue_selected_item_rect
        .expect("shell must publish selected queue row");
    assert!(queue_selected.y > model.app.layout.main.queue_area.y);
    model.app.open_context_menu(false, None);
    model.sync_mounted_surfaces();
    terminal.draw(|frame| model.draw_frame(frame, false, false)).unwrap();
    let menu = model
        .application
        .get_component(&ComponentId::Overlay(super::components::OverlayId::ContextMenu))
        .and_then(|component| component.as_any().downcast_ref::<ContextMenuComponent>())
        .expect("context menu mounted");
    assert!(matches!(menu.anchor(), ContextMenuAnchor::SelectedItem(PanelFocus::Queue)));
    let size = ContextMenu::rendered_size(menu.entries());
    let (x, y) = ContextMenu::place(model.app.layout.main.queue_area, size, Some(&queue_selected), None);
    assert_eq!(menu.menu_rect(), ratatui::layout::Rect::new(x, y, size.0, size.1));
}

#[test]
fn queue_arrow_press_leaves_exactly_one_highlighted_row() {
    let mut app = make_built_app();
    app.player_tab.set_items(
        vec![make_item("first", "Movie"), make_item("second", "Movie")],
        0,
    );
    app.panel_focus = PanelFocus::Queue;
    app.set_queue_scope(QueueScope::Local);

    let mut model = Model::new(app);
    model.sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal
        .draw(|frame| model.draw_frame(frame, false, false))
        .unwrap();
    let queue_id = ComponentId::Queue;
    let message = model
        .application
        .get_component_mut(&queue_id)
        .expect("queue mounted")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }))
        .expect("arrow press emits queue request");
    assert!(matches!(message, Msg::Queue(QueueRequest::Cursor { .. })));
    let mut resize_music = false;
    let mut resize_tv = false;
    model.handle_terminal_message(
        message,
        Some(&queue_id),
        &mut resize_music,
        &mut resize_tv,
    );

    terminal
        .draw(|frame| model.draw_frame(frame, false, false))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let queue_area = model.app.layout.main.queue_area;
    let highlighted_rows = (queue_area.y..queue_area.bottom())
        .filter(|&y| {
            (queue_area.x..queue_area.right()).any(|x| {
                buffer[(x, y)].style().bg == Some(crate::app::palette::SURFACE_FOCUSED)
            })
        })
        .count();
    assert_eq!(
        highlighted_rows, 1,
        "exactly one queue row must have the focused background"
    );
}
