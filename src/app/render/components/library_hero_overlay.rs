use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use super::backdrop::dim_backdrop_in;
use crate::app::palette;

/// Paint the Library-local overlay chrome and return the inset Hero rectangle.
/// Only `library_area` is dimmed; the Queue is outside this supplied rectangle.
pub(in crate::app) fn paint_library_hero_overlay(
    f: &mut Frame,
    library_area: Rect,
    overlay: Rect,
) -> Rect {
    dim_backdrop_in(f, library_area);
    f.render_widget(Clear, overlay);
    let block = Block::default()
        .title(Span::styled(
            " Library Hero ",
            Style::default()
                .fg(palette::TEXT_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(palette::TEXT_PRIMARY))
        .style(
            Style::default().bg(palette::surface_colors(palette::Surface::PopupFrame, false).fill),
        );
    let inner = block.inner(overlay);
    f.render_widget(block, overlay);
    if inner.height > 0 {
        f.render_widget(
            Paragraph::new(" Esc dismiss ").style(Style::default().fg(palette::TEXT_SECONDARY)),
            Rect {
                x: inner.x,
                y: inner.bottom().saturating_sub(1),
                width: inner.width,
                height: 1,
            },
        );
    }
    // The hint owns the final inner row; shared Hero content must not repaint it.
    Rect {
        height: inner.height.saturating_sub(1),
        ..inner
    }
}
