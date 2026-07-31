use super::super::super::palette;
use super::super::super::App;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
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
        self.render_backdrop_dim(f);
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

        // Draw frame around modal
        let frame_rect = Rect {
            x: rect.x.saturating_sub(2),
            y: rect.y.saturating_sub(1),
            width: rect.width + 4,
            height: rect.height + 2,
        };
        let frame_style = Style::default().bg(palette::LIBRARY_SIDE_BG);

        // Top row
        f.render_widget(
            Block::default().borders(Borders::NONE).style(frame_style),
            Rect {
                x: frame_rect.x,
                y: frame_rect.y,
                width: frame_rect.width,
                height: 1,
            },
        );

        // Bottom row
        f.render_widget(
            Block::default().borders(Borders::NONE).style(frame_style),
            Rect {
                x: frame_rect.x,
                y: frame_rect.y + frame_rect.height - 1,
                width: frame_rect.width,
                height: 1,
            },
        );

        // Left column
        f.render_widget(
            Block::default().borders(Borders::NONE).style(frame_style),
            Rect {
                x: frame_rect.x,
                y: frame_rect.y + 1,
                width: 2,
                height: frame_rect.height - 2,
            },
        );

        // Right column
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
                modal.title.clone(),
                Style::default()
                    .fg(palette::TEXT)
                    .add_modifier(Modifier::BOLD),
            ))
            .title_alignment(Alignment::Center)
            .borders(Borders::NONE)
            .style(Style::default().bg(palette::BG_GREEN));
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
