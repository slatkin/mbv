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
