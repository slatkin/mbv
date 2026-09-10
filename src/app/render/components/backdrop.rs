use crate::app::render::palette;
use ratatui::Frame;

/// Darkens every cell already rendered into the frame, across the full
/// terminal area. Call this before drawing any blocking popup overlay so
/// the background reads as dimmed while the popup itself stays at full
/// brightness.
///
/// The shade is the table's popup-level dim row: `Surface::PopupDimBackdrop`,
/// named here and resolved through the production resolver. Each cell is
/// blended halfway toward that surface's fill; the raw channel arithmetic
/// lives in the theme (`palette::dim_backdrop_color`) because raw colour
/// construction is private to it.
pub fn dim_backdrop(f: &mut Frame) {
    let base =
        palette::surface_colors_for_column_focus(palette::Surface::PopupDimBackdrop, false).fill;
    let area = f.area();
    let buf = f.buffer_mut();
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.fg = palette::dim_backdrop_color(cell.fg, base);
                cell.bg = palette::dim_backdrop_color(cell.bg, base);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::app::render::palette;
    use ratatui::style::Color;

    /// Resolve the dim through the production surface, so the test exercises
    /// the same path `dim_backdrop` does.
    fn dim(color: Color) -> Color {
        let base =
            palette::surface_colors_for_column_focus(palette::Surface::PopupDimBackdrop, false)
                .fill;
        palette::dim_backdrop_color(color, base)
    }

    #[test]
    fn dim_white_becomes_half_bright_gray() {
        assert_eq!(dim(Color::White), Color::Rgb(127, 127, 127));
    }

    #[test]
    fn dim_black_stays_black() {
        assert_eq!(dim(Color::Black), Color::Rgb(0, 0, 0));
    }

    #[test]
    fn dim_reset_stays_black() {
        assert_eq!(dim(Color::Reset), Color::Rgb(0, 0, 0));
    }

    #[test]
    fn dim_rgb_halves_each_channel() {
        assert_eq!(dim(Color::Rgb(200, 100, 50)), Color::Rgb(100, 50, 25));
    }

    #[test]
    fn dim_rgb_zero_stays_zero() {
        assert_eq!(dim(Color::Rgb(0, 0, 0)), Color::Rgb(0, 0, 0));
    }

    #[test]
    fn dim_rgb_max_becomes_half() {
        assert_eq!(dim(Color::Rgb(255, 255, 255)), Color::Rgb(127, 127, 127));
    }

    #[test]
    fn dim_indexed_passthrough() {
        let c = Color::Indexed(196);
        assert_eq!(dim(c), c);
    }
}
