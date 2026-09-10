use super::super::super::palette;
use super::backdrop::dim_backdrop;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Clear};
use ratatui::Frame;

pub fn render_modal_frame(f: &mut Frame, dim_flag: &mut bool, title: &str, w: u16, h: u16) -> Rect {
    *dim_flag = true;
    render_modal_frame_inner(f, title, w, h)
}

fn render_modal_frame_inner(f: &mut Frame, title: &str, w: u16, h: u16) -> Rect {
    dim_backdrop(f);

    // The popup frame names its own surface: callers pass no colour.
    let bg = palette::surface_colors_for_column_focus(palette::Surface::PopupFrame, false).fill;

    let full = f.area();
    let w = w.min(full.width.saturating_sub(2));
    let h = h.min(full.height);
    let x = full.x + full.width.saturating_sub(w) / 2;
    let y = full.y + full.height.saturating_sub(h) / 2;
    let rect = Rect {
        x,
        y,
        width: w,
        height: h,
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
