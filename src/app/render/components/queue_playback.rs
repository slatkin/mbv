//! The Queue playback panel's header row (task 3.5, design D10): one
//! always-painted row at the top of the queue column's content in every
//! queue-visible layout, idle included. Status left, playback target right,
//! on the chrome band.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::app::palette;
use crate::app::NowPlayingStatus;

/// The header's status word for one now-playing status.
pub(in crate::app) fn header_status_word(status: NowPlayingStatus) -> &'static str {
    match status {
        NowPlayingStatus::Playing => "PLAYING",
        NowPlayingStatus::Paused => "PAUSED",
        NowPlayingStatus::Idle => "IDLE",
    }
}

/// The status word's semantic colour: the confirmed "playing" value role
/// while playing, metadata while paused, muted while idle.
fn status_color(status: NowPlayingStatus) -> ratatui::style::Color {
    match status {
        NowPlayingStatus::Playing => palette::TEXT_ACCENT_MUTED,
        NowPlayingStatus::Paused => palette::TEXT_METADATA,
        NowPlayingStatus::Idle => palette::TEXT_MUTED,
    }
}

/// Tail-truncates `text` to at most `max_width` cells, ending in an
/// ellipsis when any character was dropped.
fn ellipsize(text: &str, max_width: u16) -> String {
    if max_width == 0 {
        return String::new();
    }
    // Reserve one cell for the ellipsis; zero-width characters ride along.
    let budget = max_width - 1;
    let mut out = String::new();
    let mut used = 0u16;
    let mut truncated = false;
    for ch in text.chars() {
        let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0) as u16;
        if used + w > budget {
            truncated = true;
            break;
        }
        out.push(ch);
        used += w;
    }
    if truncated {
        out.push('\u{2026}');
    }
    out
}

/// Paints the header row into `area`: `PLAYING`/`PAUSED`/`IDLE` on the left,
/// `on <host>` right-aligned. Pure painter over projected state.
pub(in crate::app) fn render_playback_header(
    f: &mut Frame,
    area: Rect,
    status: NowPlayingStatus,
    host: &str,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let bg = palette::surface_colors(palette::Surface::QueueOnlyPlaybackPanel, false).fill;
    let status_color = status_color(status);
    let status = header_status_word(status);
    let status_w = unicode_width::UnicodeWidthStr::width(status) as u16;
    let target = format!("on {host}");
    let target_w = target.width() as u16;
    // The gap between the status word and the right-aligned target; when the
    // row is too narrow for both, the target truncates with an ellipsis
    // before it can overlap the status word. It never drops, so the header
    // keeps stating `on <host>` (the queue-playback-panel delta).
    let avail = area.width.saturating_sub(status_w);
    let (target, inner) = if target_w <= avail {
        (target, avail - target_w)
    } else if avail > 1 {
        (ellipsize(&target, avail - 1), 1)
    } else {
        (String::new(), 0)
    };
    let mut spans = vec![Span::styled(
        status,
        Style::default()
            .fg(status_color)
            .add_modifier(Modifier::BOLD),
    )];
    let host_style = Style::default().fg(palette::TEXT_MUTED).bg(bg);
    if inner > 0 {
        spans.push(Span::styled(
            " ".repeat(inner as usize),
            Style::default().bg(bg),
        ));
    }
    if !target.is_empty() {
        spans.push(Span::styled(target, host_style));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn header_line(width: u16, status: NowPlayingStatus, host: &str) -> String {
        let mut term = Terminal::new(TestBackend::new(width, 1)).unwrap();
        term.draw(|f| render_playback_header(f, Rect::new(0, 0, width, 1), status, host))
            .unwrap();
        let buf = term.backend().buffer();
        (0..width)
            .map(|x| buf[(x, 0)].symbol().to_owned())
            .collect::<String>()
    }

    #[test]
    fn header_states_the_status_left_and_the_target_right() {
        let line = header_line(40, NowPlayingStatus::Playing, "music-box");
        assert!(line.starts_with("PLAYING"), "status left: {line:?}");
        assert!(
            line.trim_end().ends_with("on music-box"),
            "target right: {line:?}"
        );
        assert!(!line.contains("PAUSED"));

        let paused = header_line(40, NowPlayingStatus::Paused, "music-box");
        assert!(paused.starts_with("PAUSED"));

        let idle = header_line(40, NowPlayingStatus::Idle, "music-box");
        assert!(idle.starts_with("IDLE"));
    }

    #[test]
    fn header_truncates_the_target_instead_of_dropping_it_on_a_narrow_row() {
        // 7 status cells + 1 gap + a 6-cell truncated target fill a 14-cell
        // row: the target keeps its `on <host>` head and the ellipsis.
        let line = header_line(14, NowPlayingStatus::Playing, "music-box");
        assert!(line.starts_with("PLAYING"), "status left: {line:?}");
        assert!(line.contains("on mu"), "target present: {line:?}");
        assert!(line.ends_with('\u{2026}'), "target truncated: {line:?}");
        assert!(
            !line.contains("music-box"),
            "the full target cannot fit: {line:?}"
        );
    }

    #[test]
    fn header_is_blank_on_a_zero_width_area() {
        let line = header_line(0, NowPlayingStatus::Playing, "host");
        assert!(line.is_empty());
    }
}
