use super::super::super::palette;
use crate::app::render::components::modal_frame::render_modal_frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Paint the daemon-lost modal: centered 64×10 frame with diagnostics + hint.
///
/// Extracted from `impl App::render_daemon_lost_modal` so the Interactive
/// Component (`src/app/components/daemon_lost.rs`) can call it without an
/// `App` reference (design D9).
//
// `pub(in crate::app)` so the Interactive Component can call it.
pub(in crate::app) fn render_daemon_lost_modal_content(
    f: &mut Frame,
    dim_flag: &mut bool,
    last_playing_title: Option<&str>,
    daemon_log_path: &str,
    restart_error: Option<&str>,
) {
    let inner = render_modal_frame(
        f,
        dim_flag,
        " Daemon Lost ",
        64,
        10,
        palette::SURFACE_FOCUSED,
    );

    let mut lines = vec![Line::from(Span::styled(
        "The local daemon connection was lost unexpectedly.",
        Style::default().fg(palette::TEXT_STRONG),
    ))];
    if let Some(title) = last_playing_title {
        lines.push(Line::from(Span::styled(
            format!("Was playing: {title}"),
            Style::default().fg(palette::TEXT_SECONDARY),
        )));
    }
    lines.push(Line::from(Span::styled(
        format!("Daemon log: {daemon_log_path}"),
        Style::default().fg(palette::TEXT_SECONDARY),
    )));
    if let Some(error) = restart_error {
        lines.push(Line::from(Span::styled(
            format!("Restart failed: {error}"),
            Style::default().fg(palette::STATUS_ERROR),
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
            Style::default().fg(palette::TEXT_EMPHASIS),
        )),
        Rect {
            x: inner.x + 1,
            y: inner.y + inner.height.saturating_sub(2),
            width: inner.width.saturating_sub(2),
            height: 1,
        },
    );
}
