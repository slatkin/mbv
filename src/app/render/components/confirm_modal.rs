use super::super::super::palette;
use super::super::super::App;
use crate::app::render::components::modal_frame::render_modal_frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

impl App {
    /// Shared confirmation-modal overlay used by every blocking confirmation
    /// prompt in the app (clear queue, remove now-playing item, rescan
    /// library, save-playlist overwrite/discard).
    pub(in crate::app::render) fn render_confirm_modal(&mut self, f: &mut Frame) {
        let (title, message, hint) = match &self.confirm_modal {
            Some(m) => (m.title.clone(), m.message.clone(), m.hint.clone()),
            None => return,
        };
        let inner = render_modal_frame(
            f,
            &mut self.dim_backdrop_active,
            &title,
            60,
            7,
            palette::SURFACE_FOCUSED,
        );
        let base_y = inner.y + (inner.height.saturating_sub(3)) / 2;
        f.render_widget(
            Paragraph::new(Span::styled(
                message,
                Style::default().fg(palette::TEXT_STRONG),
            )),
            Rect {
                x: inner.x + 1,
                y: base_y,
                width: inner.width.saturating_sub(2),
                height: 1,
            },
        );
        f.render_widget(
            Paragraph::new(Span::styled(
                hint,
                Style::default().fg(palette::TEXT_SECONDARY),
            )),
            Rect {
                x: inner.x + 1,
                y: base_y + 2,
                width: inner.width.saturating_sub(2),
                height: 1,
            },
        );
    }
}
