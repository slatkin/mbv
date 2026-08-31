use super::components::{ComponentId, Msg, QueueRequest};
use super::tests::{make_built_app, make_item};
use super::{PanelFocus, QueueScope};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::component::Component;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};
use crate::app::shell::Model;

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
