use super::super::super::palette;
use super::backdrop::dim_backdrop;
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

fn render_modal_frame_inner(f: &mut Frame, title: &str, w: u16, h: u16, bg: Color) -> Rect {
    dim_backdrop(f);

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

    let frame_rect = Rect {
        x: rect.x.saturating_sub(2),
        y: rect.y.saturating_sub(1),
        width: rect.width + 4,
        height: rect.height + 2,
    };
    let frame_style = Style::default().bg(palette::LIBRARY_SIDE_BG);

    f.render_widget(
        Block::default().borders(Borders::NONE).style(frame_style),
        Rect {
            x: frame_rect.x,
            y: frame_rect.y,
            width: frame_rect.width,
            height: 1,
        },
    );
    f.render_widget(
        Block::default().borders(Borders::NONE).style(frame_style),
        Rect {
            x: frame_rect.x,
            y: frame_rect.y + frame_rect.height - 1,
            width: frame_rect.width,
            height: 1,
        },
    );
    f.render_widget(
        Block::default().borders(Borders::NONE).style(frame_style),
        Rect {
            x: frame_rect.x,
            y: frame_rect.y + 1,
            width: 2,
            height: frame_rect.height - 2,
        },
    );
    f.render_widget(
        Block::default().borders(Borders::NONE).style(frame_style),
        Rect {
            x: frame_rect.x + frame_rect.width - 2,
            y: frame_rect.y + 1,
            width: 2,
            height: frame_rect.height - 2,
        },
    );

    f.render_widget(Clear, rect);
    let block = Block::default()
        .title(Span::styled(
            title.to_string(),
            Style::default()
                .fg(palette::TEXT)
                .add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center)
        .borders(Borders::NONE)
        .style(Style::default().bg(bg));
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    inner
}
