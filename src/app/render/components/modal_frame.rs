use super::backdrop::dim_backdrop;
use crate::app::infra::palette;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Clear};
use ratatui::Frame;

pub fn render_modal_frame(
    f: &mut Frame,
    dim_flag: &mut bool,
    title: &str,
    w: u16,
    h: u16,
    bg: Color,
) -> Rect {
    *dim_flag = true;
    render_modal_frame_inner(f, title, w, h, bg)
}

fn render_modal_frame_inner(
    f: &mut Frame,
    title: &str,
    width: u16,
    height: u16,
    bg: Color,
) -> Rect {
    dim_backdrop(f);

    let full = f.area();
    let width = width.min(full.width.saturating_sub(2));
    let height = height.min(full.height);
    let x = full.x + full.width.saturating_sub(width) / 2;
    let y = full.y + full.height.saturating_sub(height) / 2;
    let rect = Rect {
        x,
        y,
        width,
        height,
    };

    f.render_widget(Clear, rect);
    let block = Block::default()
        .title(Span::styled(
            title.to_string(),
            Style::default()
                .fg(palette::TEXT_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center)
        .borders(Borders::NONE)
        .style(Style::default().bg(bg));
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    inner
}
