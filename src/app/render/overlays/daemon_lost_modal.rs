use super::super::super::palette;
use super::super::super::App;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

impl App {
    /// Blocking modal raised when a local daemon's connection is lost with
    /// no announced shutdown (a crash) -- see `player_event.rs`'s
    /// `PlayerEvent::Stopped` handling. Styled like `render_confirm_modal`,
    /// but taller: it shows diagnostics (last playing title, daemon log
    /// path, and an optional restart-failure line) above a 3-choice hint
    /// instead of a yes/no one.
    pub(in crate::app::render) fn render_daemon_lost_modal(&self, f: &mut Frame) {
        let Some(ref modal) = self.daemon_lost_modal else {
            return;
        };
        self.render_backdrop_dim(f);
        let full = f.area();
        let w: u16 = 64.min(full.width.saturating_sub(2));
        let h: u16 = 10.min(full.height);
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
                " Daemon Lost ",
                Style::default()
                    .fg(palette::TEXT)
                    .add_modifier(Modifier::BOLD),
            ))
            .title_alignment(Alignment::Center)
            .borders(Borders::NONE)
            .style(Style::default().bg(palette::GREEN));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let mut lines = vec![Line::from(Span::styled(
            "The local daemon connection was lost unexpectedly.",
            Style::default().fg(palette::WHITE),
        ))];
        if let Some(title) = &modal.last_playing_title {
            lines.push(Line::from(Span::styled(
                format!("Was playing: {title}"),
                Style::default().fg(palette::SUBTLE),
            )));
        }
        lines.push(Line::from(Span::styled(
            format!("Daemon log: {}", modal.daemon_log_path),
            Style::default().fg(palette::SUBTLE),
        )));
        if let Some(error) = &modal.restart_error {
            lines.push(Line::from(Span::styled(
                format!("Restart failed: {error}"),
                Style::default().fg(palette::RED),
            )));
        }
        f.render_widget(
            Paragraph::new(lines),
            Rect {
                x: inner.x + 1,
                y: inner.y + 1,
                width: inner.width.saturating_sub(2),
                height: inner.height.saturating_sub(3),
            },
        );
        f.render_widget(
            Paragraph::new(Span::styled(
                "[R] Restart and resume    [S] Restart, don't resume    [Q] Quit",
                Style::default().fg(palette::SOFT_WHITE),
            )),
            Rect {
                x: inner.x + 1,
                y: inner.y + inner.height.saturating_sub(2),
                width: inner.width.saturating_sub(2),
                height: 1,
            },
        );
    }
}
