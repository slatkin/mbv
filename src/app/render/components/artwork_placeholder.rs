use crate::app::palette;
use ratatui::widgets::Block;
use ratatui::{layout::Rect, Frame};

/// Paints the shared surface used when an item has no artwork.
pub(in crate::app) fn render_artwork_placeholder(f: &mut Frame, area: Rect) {
    f.render_widget(
        Block::default()
            .style(ratatui::style::Style::default().bg(palette::SURFACE_ARTWORK_PLACEHOLDER)),
        area,
    );
}
