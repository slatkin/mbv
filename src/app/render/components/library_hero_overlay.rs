use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::palette;

/// Paint the Library-local overlay chrome and return the inset Hero rectangle.
/// Only `library_area` is dimmed; the Queue is outside this supplied rectangle.
pub(in crate::app) fn paint_library_hero_overlay(
    f: &mut Frame,
    library_area: Rect,
    overlay: Rect,
) -> Rect {
    let buffer = f.buffer_mut();
    for y in library_area.y..library_area.bottom() {
        for x in library_area.x..library_area.right() {
            if let Some(cell) = buffer.cell_mut((x, y)) {
                cell.fg = dim(cell.fg);
                cell.bg = dim(cell.bg);
            }
        }
    }
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
    inner
}

fn dim(color: ratatui::style::Color) -> ratatui::style::Color {
    match color {
        ratatui::style::Color::White => ratatui::style::Color::Rgb(127, 127, 127),
        ratatui::style::Color::Black | ratatui::style::Color::Reset => {
            ratatui::style::Color::Rgb(0, 0, 0)
        }
        ratatui::style::Color::Rgb(r, g, b) => ratatui::style::Color::Rgb(r / 2, g / 2, b / 2),
        other => other,
    }
}
