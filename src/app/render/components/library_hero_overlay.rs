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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;
    use ratatui::Terminal;

    fn draw(area: Rect, overlay: Rect, hints: &[&str]) -> (ratatui::buffer::Buffer, Rect) {
        let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
        let mut inner = Rect::default();
        terminal
            .draw(|f| {
                inner = paint_library_hero_overlay(f, area, overlay, hints);
            })
            .unwrap();
        (terminal.backend().buffer().clone(), inner)
    }

    fn row_text(buf: &ratatui::buffer::Buffer, y: u16, x_range: (u16, u16)) -> String {
        let mut line = String::new();
        for x in x_range.0..x_range.1 {
            line.push_str(buf[(x, y)].symbol());
        }
        line
    }

    /// The bottom row is a display-only pill bar: each hint is one joined chip
    /// on the row surface, filled from the rotating foam/yellow/orange order.
    #[test]
    fn hint_bar_paints_bottom_row_pills_in_rotating_fills() {
        let area = Rect::new(0, 0, 48, 12);
        let overlay = Rect::new(4, 2, 40, 8);
        let hints = ["Enter:Play", "Esc:Dismiss"];
        let (buf, inner) = draw(area, overlay, &hints);
        let row_bg = palette::surface_colors(palette::Surface::PillRow, false).fill;
        let foam = palette::HINT_PILL_FILLS[0];
        let yellow = palette::HINT_PILL_FILLS[1];
        let bar_y = overlay.bottom() - 1;
        let line = row_text(&buf, bar_y, (overlay.x, overlay.right()));
        assert!(line.contains("Enter:Play"));
        assert!(line.contains("Esc:Dismiss"));
        // The chips are centered: chip 0 is "◢Enter:Play◤" = 12 columns and
        // chip 1 is "Esc:Dismiss◤" = 12 (no leading glyph, no inner pads), so
        // 24 columns of a 40-column row leave 8 columns of row surface either
        // side of the group.
        assert_eq!(buf[(overlay.x + 8, bar_y)].bg, row_bg); // chip 0's angled end
        assert_eq!(buf[(overlay.x + 9, bar_y)].bg, foam);
        assert_eq!(buf[(overlay.x + 19, bar_y)].bg, yellow); // chip 0's trailing edge
                                                             // The label abuts the seam: no pad column before "Esc".
        assert_eq!(buf[(overlay.x + 20, bar_y)].symbol(), "E");
        assert_eq!(buf[(overlay.x + 20, bar_y)].bg, yellow);
        assert_eq!(buf[(overlay.x + 21, bar_y)].symbol(), "s");
        // Chip 1's trailing edge, then the row surface on both sides.
        assert_eq!(buf[(overlay.x + 31, bar_y)].bg, row_bg);
        assert_eq!(buf[(overlay.right() - 1, bar_y)].bg, row_bg);
        // The content runs from one blank top row down to the hint row, with
        // no blank row between them.
        assert_eq!(inner.y, overlay.y + 1);
        assert_eq!(inner.height, overlay.height - 2);
    }

    #[test]
    fn chip_fill_rotation_repeats_past_three_hints() {
        let area = Rect::new(0, 0, 60, 8);
        let overlay = Rect::new(0, 0, 60, 8);
        let hints = ["A:1", "B:2", "C:3", "D:4"];
        let (buf, _) = draw(area, overlay, &hints);
        let bar_y = overlay.bottom() - 1;
        let fills = palette::HINT_PILL_FILLS;
        let foam = fills[0];
        let yellow = fills[1];
        let orange = fills[2];
        // Chip 0 is "◢A:1◤" = 5 columns and every later chip is "B:2◤" = 4,
        // so a 17-column group in a 60-column row leaves 21 columns to its
        // left; the chips start at 21, 26, 30, and 34, and each chip's first
        // fill column is its start (chip 0's is one further in, past ◢).
        // The rotation repeats past three.
        let starts = [22usize, 26, 30, 34];
        for (idx, expected) in [foam, yellow, orange, foam].into_iter().enumerate() {
            assert_eq!(buf[(starts[idx] as u16, bar_y)].bg, expected, "chip {idx}");
        }
    }

    /// The dim backdrop is confined to the supplied area: the reserved rows
    /// above the Library inset panel keep their own background, while the inset
    /// panel's remainder outside the overlay frame is dimmed.
    #[test]
    fn dim_backdrop_is_confined_to_the_supplied_area() {
        let area = Rect::new(0, 0, 48, 12);
        // The inset list box, with two reserved chrome rows above it (the pill
        // bar and its spacer band).
        let inset = Rect::new(0, 2, 48, 10);
        let overlay = Rect::new(4, 3, 40, 8);
        let chrome = palette::surface_colors(palette::Surface::PillRow, false).fill;
        let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
        terminal
            .draw(|f| {
                f.render_widget(Block::default().style(Style::default().bg(chrome)), area);
                paint_library_hero_overlay(f, inset, overlay, &["Esc:Dismiss"]);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        for y in 0..inset.y {
            for x in 0..area.width {
                assert_eq!(buf[(x, y)].bg, chrome, "chrome row {y} column {x}");
            }
        }
        assert_eq!(buf[(0, inset.y + 1)].bg, Color::Rgb(15, 17, 19)); // half of PillRow
        assert_eq!(buf[(overlay.x, overlay.y)].bg, chrome); // the sheet is not dimmed
    }

    #[test]
    fn top_row_is_not_the_hint() {
        let area = Rect::new(0, 0, 40, 12);
        let overlay = Rect::new(4, 2, 32, 8);
        let (buf, _) = draw(area, overlay, &["Esc:Dismiss"]);
        assert!(!row_text(&buf, overlay.y, (overlay.x, overlay.right())).contains("Dismiss"));
    }
}
