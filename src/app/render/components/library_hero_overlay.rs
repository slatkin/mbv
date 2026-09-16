use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::Frame;

use super::backdrop::dim_backdrop_in;
use crate::app::palette;

/// Paint the frameless Library-local overlay surface and return the inset Hero
/// rectangle. Only `library_area` is dimmed; the Queue is outside this supplied
/// rectangle. The overlay interior restores the Library column's own undimmed
/// surface so shared Hero content renders with the same colours as the Wide
/// Hero pane.
pub(in crate::app) fn paint_library_hero_overlay(
    f: &mut Frame,
    library_area: Rect,
    overlay: Rect,
) -> Rect {
    dim_backdrop_in(f, library_area);
    // Reset the covered cells' symbols first, then restore the Library
    // column's undimmed surface fill — Block::style only patches, so without
    // the Clear the dimmed rows underneath would show through.
    f.render_widget(Clear, overlay);
    f.render_widget(
        Block::default().style(
            Style::default()
                .bg(palette::surface_colors(palette::Surface::LibraryColumn, false).fill),
        ),
        overlay,
    );
    if overlay.height > 0 {
        f.render_widget(
            Paragraph::new(" Esc dismiss ").style(Style::default().fg(palette::TEXT_SECONDARY)),
            Rect {
                x: overlay.x,
                y: overlay.bottom() - 1,
                width: overlay.width,
                height: 1,
            },
        );
    }
    // The hint owns the final row; shared Hero content must not repaint it.
    // The content itself is inset: one blank row on top and two blank columns
    // on the left and right, so the Hero never touches the overlay's edge.
    Rect {
        x: overlay.x.saturating_add(2),
        y: overlay.y.saturating_add(1),
        width: overlay.width.saturating_sub(4),
        height: overlay
            .height
            .saturating_sub(1)
            .saturating_sub(1)
            .saturating_sub(1),
    }
}
