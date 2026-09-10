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
            );
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

/// `unify-surface-colour` 4.2: the sidebar body and its header band name the
/// declared sidebar surfaces (`SidebarBody` / `SidebarBand`), not a panel
/// row. Pin both through the production resolver and observe the painted
/// cells.
#[test]
fn help_sidebar_body_and_band_paint_their_surfaces() {
    use crate::app::palette;

    let width = 60;
    let height = 20;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut scroll = 0;
    terminal
        .draw(|f| {
            render_help_panel(
                f,
                Some(Rect::new(0, 0, width, height)),
                &mut scroll,
                HelpDestination::EmbyLibrary,
            );
        })
        .unwrap();
    let body = palette::surface_colors_for_column_focus(palette::Surface::SidebarBody, false).fill;
    let band = palette::surface_colors_for_column_focus(palette::Surface::SidebarBand, false).fill;
    assert_eq!(
        body,
        palette::SURFACE_RESTING,
        "the expanded sidebar body keeps today's resting value"
    );
    assert_eq!(
        band,
        palette::SURFACE_CHROME,
        "the sidebar band keeps today's chrome value"
    );
    let buffer = terminal.backend().buffer();
    // Body row between the header (y+1) and the footer (y+height-2); the
    // left gutter is never covered by content.
    assert_eq!(buffer[(0, 5)].style().bg, Some(body), "sidebar body fill");
    // Header band at x+2, y+1.
    assert_eq!(buffer[(2, 1)].style().bg, Some(band), "sidebar header band");
}
