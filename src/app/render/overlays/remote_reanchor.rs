use super::super::super::palette;
use super::super::super::App;
use super::modal_frame::render_modal_frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

impl App {
    pub(in crate::app::render) fn render_remote_reanchor_popup(&mut self, f: &mut Frame) {
        let Some(popup) = &self.remote_reanchor_popup else {
            return;
        };
        let title = " Re-anchor Remote Tracking ";
        let width = 46;
        let height = (popup.targets.len() as u16 + 3).min(12);
        let inner = render_modal_frame(
            f,
            &mut self.dim_backdrop_active,
            title,
            width,
            height + 2,
            palette::BG_GREEN,
        );
        f.render_widget(
            Paragraph::new(Span::styled(
                "Choose the observed occurrence",
                Style::default().fg(palette::MUTED),
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
        let cursor = popup.cursor;
        let list_h = list_area.height as usize;
        let scroll = cursor.saturating_sub(list_h.saturating_sub(1));
        let lines = popup
            .targets
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
                            palette::TEXT
                        } else {
                            palette::MUTED
                        }),
                    ),
                    Span::styled(
                        format!("Occurrence {}: {media_id}", occurrence + 1),
                        Style::default().fg(if focused {
                            palette::TEXT
                        } else {
                            palette::SUBTLE
                        }),
                    ),
                ])
            })
            .collect::<Vec<_>>();
        f.render_widget(Paragraph::new(lines), list_area);
        f.render_widget(
            Paragraph::new(Span::styled(
                "Enter select  ·  Esc cancel",
                Style::default().fg(palette::MUTED),
            )),
            Rect {
                x: inner.x,
                y: inner.y + inner.height.saturating_sub(1),
                width: inner.width,
                height: 1,
            },
        );
    }
}
