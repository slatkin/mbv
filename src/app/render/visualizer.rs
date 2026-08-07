use super::super::{palette, App};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use ratatui::Frame;

pub(super) const VISUALIZER_HEIGHT: u16 = 11;

impl App {
    pub(super) fn render_visualizer(&self, f: &mut Frame, area: Rect, bg: Color) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let bg_style = Style::default().bg(bg);
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = f.buffer_mut().cell_mut((x, y)) {
                    cell.set_style(bg_style);
                }
            }
        }
        let inner = Rect {
            x: area.x + 2,
            y: area.y + 1,
            width: area.width.saturating_sub(4),
            height: area.height.saturating_sub(2),
        };
        if inner.width == 0 || inner.height == 0 {
            return;
        }

        let bars = &self.visualizer_frame;
        let rows = (0..inner.height)
            .map(|row| {
                let text = (0..inner.width)
                    .map(|column| {
                        if column % 2 == 1 {
                            ' '
                        } else {
                            let value = if bars.is_empty() {
                                0.0
                            } else {
                                let bar_index = (column / 2) as usize * bars.len()
                                    / inner.width.div_ceil(2) as usize;
                                let raw = bars[bar_index.min(bars.len() - 1)];
                                raw.min(1.0)
                            };
                            let height = (value * inner.height as f32).round() as u16;
                            if height >= inner.height.saturating_sub(row) {
                                '█'
                            } else {
                                ' '
                            }
                        }
                    })
                    .collect::<String>();
                Line::from(Span::styled(
                    text,
                    Style::default().fg(palette::AQUA).bg(bg),
                ))
            })
            .collect::<Vec<_>>();
        Paragraph::new(rows).render(inner, f.buffer_mut());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::make_app_stub;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn render_visualizer_is_noop_on_empty_area() {
        let mut app = make_app_stub();
        app.visualizer_enabled = true;
        app.visualizer_frame = vec![1.0; 64];

        let backend = TestBackend::new(20, 10);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            app.render_visualizer(f, Rect::default(), crate::app::palette::LIBRARY_SIDE_BG);
        })
        .unwrap();
    }
}
