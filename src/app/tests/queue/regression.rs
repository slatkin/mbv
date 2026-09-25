use crate::app::components::{
    ComponentId, ContextMenuComponent, Msg, QueueComponent, ShellRequest,
};
use crate::app::shell::Model;
use crate::app::state::types::context_menu::{ContextMenu, ContextMenuAnchor};
use crate::app::tests::{make_built_app, make_item};
use crate::app::{PanelFocus, QueueScope};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

#[test]
fn shell_frame_leaves_queue_geometry_retained_in_the_queue_component() {
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

    let layout_area = queue_panel_content_area(&model);
    assert!(
        layout_area.height > 0,
        "the queue panel must retain a usable content area"
    );
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
}

/// The queue panel's framed content area, read from the mounted component
/// (task 3.1: component-retained geometry, no legacy chrome geometry mirror).
fn mounted_queue_selected_row(model: &Model) -> ratatui::layout::Rect {
    model
        .application
        .get_component(&ComponentId::Queue)
        .and_then(|component| component.as_any().downcast_ref::<QueueComponent>())
        .and_then(QueueComponent::selected_row_rect)
        .expect("queue component retains its selected row")
}

fn queue_panel_content_area(model: &Model) -> ratatui::layout::Rect {
    model
        .application
        .get_component(&ComponentId::Queue)
        .and_then(|component| component.as_any().downcast_ref::<QueueComponent>())
        .map(crate::app::components::QueueComponent::content_area)
        .expect("QueueComponent mounted")
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
        .expect("queue cursor movement emits a request");
    let mut resize_music = false;
    let mut resize_tv = false;
    model.handle_terminal_message(message, &mut resize_music, &mut resize_tv);
    terminal
        .draw(|frame| model.draw_frame(frame, false, false))
        .unwrap();

    let queue_selected = mounted_queue_selected_row(&model);
    assert!(queue_selected.y > queue_panel_content_area(&model).y);
    let message = Msg::Shell(Box::new(ShellRequest::RowContextMenu(
        crate::app::state::types::context_menu::ContextMenuTargets::Queue(vec![]),
        None,
    )));
    model.handle_terminal_message(message, &mut resize_music, &mut resize_tv);
    model.sync_mounted_surfaces();
    terminal
        .draw(|frame| model.draw_frame(frame, false, false))
        .unwrap();
    let menu = model
        .application
        .get_component(&ComponentId::Overlay(
            crate::app::components::OverlayId::ContextMenu,
        ))
        .and_then(|component| component.as_any().downcast_ref::<ContextMenuComponent>())
        .expect("context menu mounted");
    assert!(matches!(
        menu.anchor(),
        ContextMenuAnchor::SelectedItem(PanelFocus::Queue)
    ));
    let size = ContextMenu::rendered_size(menu.entries());
    let (x, y) = ContextMenu::place(
        queue_panel_content_area(&model),
        size,
        Some(&queue_selected),
        None,
    );
    assert_eq!(
        menu.menu_rect(),
        ratatui::layout::Rect::new(x, y, size.0, size.1)
    );
}

#[test]
fn mini_view_panel_does_not_overlay_queue_on_mode_switch() {
    // Entering queue-only mini view used to repaint the player panel with the
    // rect published by the *previous* frame, so the panel sat on top of the
    // queue's rows until the next pass. The component paints with the rect the
    // base frame publishes for the frame being drawn, which is empty while the
    // legacy frame owns the queue-only panel.
    use crate::app::dispatch::action::Command;
    let rows = |terminal: &Terminal<TestBackend>, range: std::ops::Range<u16>| -> Vec<String> {
        let buf = terminal.backend().buffer().clone();
        range
            .map(|y| {
                (0..buf.area().width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect()
            })
            .collect()
    };

    let mut app = make_built_app();
    app.player_tab
        .set_items(crate::app::tests::make_items(12), 1);
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 1;
        status.title = "Movie One".into();
    }
    app.terminal_width = 70;
    app.mini_view_focus = PanelFocus::Library;

    let mut model = Model::new(app);
    let mut terminal = Terminal::new(TestBackend::new(70, 24)).unwrap();
    model.sync_mounted_surfaces();
    terminal
        .draw(|frame| model.draw_frame(frame, false, false))
        .unwrap();

    // A second sync pass, so the projection now carries this frame's panel rect.
    model.sync_mounted_surfaces();
    model.app.dispatch(Command::CyclePanelMode);
    assert_eq!(
        model.app.effective_panel_mode(),
        crate::app::PanelMode::QueueOnly
    );
    terminal
        .draw(|frame| model.draw_frame(frame, false, false))
        .unwrap();
    let transition = rows(&terminal, 3..8);

    model.sync_mounted_surfaces();
    terminal
        .draw(|frame| model.draw_frame(frame, false, false))
        .unwrap();
    assert_eq!(
        transition,
        rows(&terminal, 3..8),
        "the frame that switches into queue-only mini view must already match \
         the steady queue-only frame in the rows above the queue list"
    );
}

/// Row 5.2 / row 9.2 block correction: at the mini breakpoint the queue panel
/// is the sole painted surface. The library destination's mounted browser
/// used the app's left area as its paint area, but that field is only
/// republished as the library content rect while the base frame renders the
/// library; in queue-only mode it stayed the full queue column, so the Emby
/// browser painted its rows straight over the queue. Assert
/// the compact fixed-row queue paints and the library does not leak in.
#[test]
fn mini_view_queue_panel_paints_only_the_queue_not_the_library() {
    let mut app = crate::app::render::make_queue_app(6);
    app.terminal_width = 70;
    app.terminal_height = 24;
    app.mini_view_focus = PanelFocus::Queue;

    let mut model = Model::new(app);
    model.sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(70, 24)).unwrap();
    terminal
        .draw(|frame| model.draw_frame(frame, false, false))
        .unwrap();

    assert_eq!(
        model.app.effective_panel_mode(),
        crate::app::PanelMode::QueueOnly,
        "the mini breakpoint must show the queue panel only"
    );

    let queue_area = queue_panel_content_area(&model);
    let buffer = terminal.backend().buffer();
    let queue_text = (queue_area.y..queue_area.bottom())
        .map(|y| {
            (queue_area.x..queue_area.right())
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        queue_text.contains("Queue Item 0"),
        "the queue mini view must paint its fixed rows:\n{queue_text}"
    );
    assert!(
        !queue_text.contains("Focused Movie") && !queue_text.contains("Second Movie"),
        "the library browser must not paint over the queue mini view:\n{queue_text}"
    );
    assert!(
        !queue_text.contains("compact movie banner"),
        "the library browser must not leak into the queue mini view:\n{queue_text}"
    );
}
