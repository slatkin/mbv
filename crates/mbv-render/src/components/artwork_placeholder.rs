use mbv_theme as palette;
use ratatui::widgets::Block;
use ratatui::{Frame, layout::Rect};

/// Paints the shared surface used when an item has no artwork.
pub fn render_artwork_placeholder(f: &mut Frame, area: Rect) {
    f.render_widget(
        Block::default().style(
            ratatui::style::Style::default().bg(palette::surface_colors(
                palette::Surface::ArtworkPlaceholder,
                false,
            )
            .fill),
        ),
        area,
    );
}
