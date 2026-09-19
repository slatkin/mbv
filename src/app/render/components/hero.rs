//! Shared search-row painting.

use crate::app::palette;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

pub(in crate::app) fn render_search_box(f: &mut Frame, area: Rect, query: &str, loading: bool) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let style = Style::default().bg(palette::surface_colors(palette::Surface::PillRow, false).fill);
    f.render_widget(Block::default().style(style), area);
    let input = if loading {
        format!("{query}█ [loading…]")
    } else {
        format!("{query}█")
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" SEARCH: {input}"),
            Style::default().fg(palette::TEXT_PRIMARY.color()),
        ))),
        area,
    );
}
