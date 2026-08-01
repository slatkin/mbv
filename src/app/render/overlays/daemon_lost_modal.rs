use super::super::super::palette;
use super::super::super::App;
use super::modal_frame::render_modal_frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

impl App {
    /// Blocking modal raised when a local daemon's connection is lost with
    /// no announced shutdown (a crash) -- see `player_event.rs`'s
    /// `PlayerEvent::Stopped` handling. Taller than the confirm modal: it
    /// shows diagnostics (last playing title, daemon log path, and an
    /// optional restart-failure line) above a 3-choice hint instead of a
    /// yes/no one.
    pub(in crate::app::render) fn render_daemon_lost_modal(&self, f: &mut Frame) {
        let Some(ref modal) = self.daemon_lost_modal else {
            return;
        };
        let inner = render_modal_frame(f, " Daemon Lost ", 64, 10);

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
