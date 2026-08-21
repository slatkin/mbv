use super::test_helpers::buffer_to_string;
use crate::app::layout::{AppLayout, LayoutMain};
use crate::app::tests::make_app_stub;
use crate::app::{ContextAction, ContextMenu, ContextMenuAnchor, ContextMenuEntry, PanelFocus};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

fn render_menu(width: u16, height: u16, cursor: usize) -> String {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.context_menu = Some(ContextMenu {
        anchor: ContextMenuAnchor::Pointer { x: 5, y: 5 },
        entries: vec![
            ContextMenuEntry {
                label: "Play",
                action: Some(ContextAction::Play),
            },
            ContextMenuEntry {
                label: "Queue",
                action: Some(ContextAction::Play),
            },
        ],
        cursor,
    });
    let mut layout = AppLayout::default();
    layout.main = LayoutMain {
        left_area: Rect::new(0, 0, width, height),
        ..layout.main
    };
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| app.render_context_menu(f, &mut layout))
        .unwrap();
    buffer_to_string(&terminal)
}

#[test]
fn context_menu_buffer_characterization_covers_default_focused_narrow_and_selected_states() {
    for (width, height, cursor) in [(60, 20, 0), (60, 20, 1), (20, 10, 1), (32, 12, 0)] {
        let output = render_menu(width, height, cursor);
        assert!(output.contains("Play"), "context menu missing: {output:?}");
    }
}
