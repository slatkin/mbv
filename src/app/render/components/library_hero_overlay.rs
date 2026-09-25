use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Clear};
use ratatui::Frame;

use super::backdrop::dim_backdrop_in;
use crate::app::palette;

/// The overlay sheet's fill surface. Shared with the sheet's hint pill row so
/// the rectangle and its hints read as one surface, and handed to the shared
/// Hero composition so the content repaints the sheet (not a wide pane's fill)
/// wherever it needs the background.
pub(in crate::app) const OVERLAY_SHEET_SURFACE: palette::Surface = palette::Surface::PillRow;

/// Paint the frameless Library-local overlay surface and return the inset Hero
/// rectangle. Only `library_area` is dimmed: in non-Wide geometry the caller
/// supplies the browser's inset list box, so the reserved pill bar and its
/// spacer band above it stay undimmed and legible; the Queue is outside this
/// supplied rectangle. The overlay sheet is the popup's own dark chrome surface
/// (the same one its hint pill row paints), so the rectangle and its hints read
/// as one surface.
///
/// `hints` is the overlay's bottom-row hint bar content, derived centrally by
/// the caller from the active destination (a content change, not a painter
/// arm): one display-only chip per hint, painted by the shared pill-bar
/// shell, and shared Hero content must not repaint that row.
pub(in crate::app) fn paint_library_hero_overlay(
    f: &mut Frame,
    library_area: Rect,
    overlay: Rect,
    hints: &[&str],
) -> Rect {
    dim_backdrop_in(f, library_area);
    // Reset the covered cells' symbols first, then paint the sheet's own fill —
    // Block::style only patches, so without the Clear the dimmed rows
    // underneath would show through.
    f.render_widget(Clear, overlay);
    f.render_widget(
        Block::default()
            .style(Style::default().bg(palette::surface_colors(OVERLAY_SHEET_SURFACE, false).fill)),
        overlay,
    );
    if overlay.height > 0 {
        super::widgets::render_hint_pill_bar(
            f,
            Rect {
                x: overlay.x,
                y: overlay.bottom() - 1,
                width: overlay.width,
                height: 1,
            },
            hints,
        );
    }
    // The hint owns the last row; the content is inset one blank row from the
    // sheet's top and two blank columns from its sides, so the Hero never
    // touches the overlay's edge.
    Rect {
        x: overlay.x.saturating_add(2),
        y: overlay.y.saturating_add(1),
        width: overlay.width.saturating_sub(4),
        height: overlay.height.saturating_sub(1).saturating_sub(1),
    }
}
