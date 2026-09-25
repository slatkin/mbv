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

/// Darkens every cell already rendered into the frame, across the full
/// terminal area. Call this before drawing any blocking modal overlay so
/// the background reads as dimmed while the modal itself stays at full
/// brightness.
pub fn dim_backdrop(f: &mut Frame) {
    dim_backdrop_in(f, f.area());
}

/// Darkens cells within `area`, leaving the rest of the frame unchanged.
pub fn dim_backdrop_in(f: &mut Frame, area: ratatui::layout::Rect) {
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
