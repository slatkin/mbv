use super::test_helpers::buffer_to_string;
use crate::app::render::components::help::{help_destination, render_help_panel, HelpDestination};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

fn render_help(width: u16, height: u16, scroll: u16) -> String {
    let mut scroll = scroll;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            render_help_panel(
                f,
                Some(Rect::new(0, 0, width, height)),
                &mut scroll,
                HelpDestination::EmbyLibrary,
            )
        })
        .unwrap();
    buffer_to_string(&terminal)
}

#[test]
fn help_buffer_characterization_covers_default_focused_narrow_and_selected_states() {
    for (width, height, scroll) in [(60, 20, 0), (60, 20, 2), (18, 8, 0), (32, 12, 1)] {
        let output = render_help(width, height, scroll);
        assert!(
            output.contains("KEYBOARD"),
            "help shell missing: {output:?}"
        );
    }
}

#[test]
fn help_destination_queue_focus_returns_queue() {
    use crate::app::{PanelFocus, TabSelection};
    assert_eq!(
        help_destination(PanelFocus::Queue, TabSelection::Home),
        HelpDestination::Queue
    );
}
