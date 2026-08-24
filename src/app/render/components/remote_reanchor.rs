use super::super::super::palette;
use crate::app::render::components::modal_frame::render_modal_frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Paint the remote-reanchor popup: centered frame with target list + hint.
///
/// Extracted from `impl App::render_remote_reanchor_popup` so the Interactive
/// Component (`src/app/components/remote_reanchor.rs`) can call it without an
/// `App` reference (design D9).
//
// `pub(in crate::app)` so the Interactive Component can call it.
pub(in crate::app) fn render_remote_reanchor_popup_content(
    f: &mut Frame,
    dim_flag: &mut bool,
    targets: &[(usize, String)],
    cursor: usize,
) {
    let title = " Re-anchor Remote Tracking ";
    let width = 46;
    let height = (targets.len() as u16 + 3).min(12);
    let inner = render_modal_frame(
        f,
        dim_flag,
        title,
        width,
        height + 2,
        palette::SURFACE_FOCUSED,
    );
    f.render_widget(
        Paragraph::new(Span::styled(
            "Choose the observed occurrence",
            Style::default().fg(palette::TEXT_MUTED),
        )),
        Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        },
    );
    let list_area = Rect {
        x: inner.x,
        y: inner.y + 1,
        width: inner.width,
        height: inner.height.saturating_sub(2),
    };
    let list_h = list_area.height as usize;
    let scroll = cursor.saturating_sub(list_h.saturating_sub(1));
    let lines = targets
        .iter()
        .enumerate()
        .skip(scroll)
        .take(list_h)
        .map(|(index, (occurrence, media_id))| {
            let focused = index == cursor;
            Line::from(vec![
                Span::styled(
                    if focused { "▸ " } else { "  " },
                    Style::default().fg(if focused {
                        palette::TEXT_PRIMARY
                    } else {
                        palette::TEXT_MUTED
                    }),
                ),
                Span::styled(
                    format!("Occurrence {}: {media_id}", occurrence + 1),
                    Style::default().fg(if focused {
                        palette::TEXT_PRIMARY
                    } else {
                        palette::TEXT_SECONDARY
                    }),
                ),
            ])
        })
        .collect::<Vec<_>>();
    f.render_widget(Paragraph::new(lines), list_area);
    f.render_widget(
        Paragraph::new(Span::styled(
            "Enter select  ·  Esc cancel",
            Style::default().fg(palette::TEXT_MUTED),
        )),
        Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        },
    );
}
