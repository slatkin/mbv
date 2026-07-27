use super::super::super::App;
use ratatui::style::Color;
use ratatui::Frame;

/// Fraction of each color channel kept when dimming the backdrop behind a
/// blocking modal (the rest is blended toward black).
const DIM_FACTOR: f32 = 0.5;

fn dim(color: Color) -> Color {
    match color {
        Color::White => Color::Rgb(127, 127, 127),
        Color::Black | Color::Reset => Color::Rgb(0, 0, 0),
        Color::Rgb(r, g, b) => Color::Rgb(
            (r as f32 * DIM_FACTOR) as u8,
            (g as f32 * DIM_FACTOR) as u8,
            (b as f32 * DIM_FACTOR) as u8,
        ),
        // Indexed (256-color) and Named variants pass through undimmed because
        // we have no portable way to look up their actual RGB values — the
        // mapping is defined by the running terminal, not by ratatui.
        other => other,
    }
}

impl App {
    /// Darkens every cell already rendered into the frame, across the full
    /// terminal area. Blocking modal overlays (confirm modal, save-playlist
    /// dialog, multiselect popup, library-routes popup) call this first,
    /// then draw their own `Clear` + bordered content on top -- so the
    /// background reads as dimmed while the modal itself stays at full
    /// brightness.
    pub(in crate::app::render) fn render_backdrop_dim(&self, f: &mut Frame) {
        let area = f.area();
        let buf = f.buffer_mut();
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.fg = dim(cell.fg);
                    cell.bg = dim(cell.bg);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
