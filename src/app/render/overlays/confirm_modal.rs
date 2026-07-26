use super::super::super::palette;
use super::super::super::App;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

impl App {
    /// Shared confirmation-modal overlay used by every blocking yes/no
    /// prompt in the app (clear queue, remove now-playing item, rescan
    /// library, save-playlist overwrite/discard). Styled like the other
    /// modern overlays (`context_menu.rs`, `multiselect.rs`): rounded border,
    /// `palette::IRIS`, rather than the older `palette::YELLOW` look.
    pub(in crate::app::render) fn render_confirm_modal(&self, f: &mut Frame) {
        let Some(ref modal) = self.confirm_modal else {
            return;
        };
        let full = f.area();
        let w: u16 = 60.min(full.width.saturating_sub(2));
        let h: u16 = 7.min(full.height);
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
                modal.title.clone(),
                Style::default()
                    .fg(palette::TEXT)
                    .add_modifier(Modifier::BOLD),
            ))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::IRIS));
        let inner = block.inner(rect);
        f.render_widget(block, rect);
        let base_y = inner.y + (inner.height.saturating_sub(3)) / 2;
        f.render_widget(
            Paragraph::new(Span::styled(
                modal.message.clone(),
                Style::default().fg(palette::WHITE),
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
                modal.hint.clone(),
                Style::default().fg(palette::SUBTLE),
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
