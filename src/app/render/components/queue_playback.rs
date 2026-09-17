//! The Queue playback panel's header row (task 3.5, design D10): one
//! always-painted row at the top of the queue column's content in every
//! queue-visible layout, idle included. Status left, playback target right,
//! on the chrome band. It never carries progress: the transport below owns
//! the throbber, percent, and seekbar.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::app::palette;
use crate::app::ui_util::trunc_str;
use crate::app::NowPlayingStatus;

/// The header's status word for one now-playing status.
pub(in crate::app) fn header_status_word(status: NowPlayingStatus) -> &'static str {
    match status {
        NowPlayingStatus::Playing => "PLAYING",
        NowPlayingStatus::Paused => "PAUSED",
        NowPlayingStatus::Idle => "IDLE",
    }
}

/// The status word's semantic colour: FOAM while playing, YELLOW while
/// paused, muted while idle.
fn status_color(status: NowPlayingStatus) -> ratatui::style::Color {
    match status {
        NowPlayingStatus::Playing => palette::TEXT_METADATA,
        NowPlayingStatus::Paused => palette::TEXT_FOCUS_ACCENT,
        NowPlayingStatus::Idle => palette::TEXT_MUTED,
    }
}

/// Paints the header row into `area`: ` PLAYING`/` PAUSED`/` IDLE` on the left
/// (one space of text indent), `on <host> ` right-aligned (one space of
/// trailing text padding). The `on ` prefix stays muted; the hostname paints
/// green when local, aqua when remote. Pure painter over projected state.
pub(in crate::app) fn render_playback_header(
    f: &mut Frame,
    area: Rect,
    status: NowPlayingStatus,
    host: &str,
    host_is_remote: bool,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let bg = palette::surface_colors(palette::Surface::QueueOnlyPlaybackPanel, false).fill;
    let status_color = status_color(status);
    let status = header_status_word(status);
    let status_w = unicode_width::UnicodeWidthStr::width(status) as u16;
    let full_target = format!("on {host}");
    let full_target_w = full_target.width() as u16;
    // One space of text padding on each side inside the header band. The
    // content budget is the band minus those two cells; when the band is
    // too narrow to hold even the padding, paint the background only.
    let content_w = area.width.saturating_sub(2);
    let status_fits = status_w <= content_w;
    let status_w = if status_fits { status_w } else { 0 };
    let avail = content_w.saturating_sub(status_w);
    // The gap between the status word and the right-aligned target; when
    // the row is too narrow, the target truncates with an ellipsis before
    // it can overlap the status. It never drops, so the header keeps
    // stating `on <host>` (the queue-playback-panel delta).
    let (target, inner) = if !status_fits {
        (String::new(), 0)
    } else if full_target_w <= avail {
        (full_target, avail - full_target_w)
    } else if avail > 1 {
        (trunc_str(&full_target, (avail - 1) as usize), 1)
    } else {
        (String::new(), 0)
    };
    let target_w = target.width() as u16;
    let bg_style = Style::default().bg(bg);
    let mut spans = vec![Span::styled(" ", bg_style)];
    if status_fits {
        spans.push(Span::styled(
            status,
            Style::default()
                .fg(status_color)
                .bg(bg)
                .add_modifier(Modifier::BOLD),
        ));
    }
    let host_style = Style::default().fg(palette::TEXT_MUTED).bg(bg);
    let hostname_style = Style::default()
        .fg(if host_is_remote {
            palette::ACCENT
        } else {
            palette::STATUS_AVAILABLE
        })
        .bg(bg);
    if inner > 0 {
        spans.push(Span::styled(
            " ".repeat(inner as usize),
            Style::default().bg(bg),
        ));
    }
    if !target.is_empty() {
        // The hostname carries the local/remote colour; the `on ` prefix
        // stays muted. A target truncated short of the full prefix keeps
        // the muted style rather than colouring a fragment.
        if let Some(host_part) = target.strip_prefix("on ").filter(|s| !s.is_empty()) {
            let host_part = host_part.to_owned();
            spans.push(Span::styled("on ", host_style));
            spans.push(Span::styled(host_part, hostname_style));
        } else {
            spans.push(Span::styled(target, host_style));
        }
    }
    // Trailing text padding: fill whatever cells remain (at least the one
    // reserved pad cell, more when the target truncated short) with the
    // band background so the text never touches the right edge.
    let used_w = 1 + status_w + inner + target_w;
    let trailing = area.width.saturating_sub(used_w);
    if trailing > 0 {
        spans.push(Span::styled(" ".repeat(trailing as usize), bg_style));
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
        term.draw(|f| render_playback_header(f, Rect::new(0, 0, width, 1), status, host, false))
            .unwrap();
        let buf = term.backend().buffer();
        (0..width)
            .map(|x| buf[(x, 0)].symbol().to_owned())
            .collect::<String>()
    }

    #[test]
    fn header_states_the_status_left_and_the_target_right() {
        let line = header_line(40, NowPlayingStatus::Playing, "music-box");
        assert!(
            line.starts_with(" PLAYING"),
            "status indented one space: {line:?}"
        );
        assert!(
            line.ends_with("on music-box "),
            "target right with one trailing space: {line:?}"
        );
        assert!(!line.contains("PAUSED"));

        let paused = header_line(40, NowPlayingStatus::Paused, "music-box");
        assert!(paused.starts_with(" PAUSED"));
        assert!(paused.ends_with(' '));

        let idle = header_line(40, NowPlayingStatus::Idle, "music-box");
        assert!(idle.starts_with(" IDLE"));
        assert!(idle.ends_with(' '));
    }

    #[test]
    fn header_truncates_the_target_instead_of_dropping_it_on_a_narrow_row() {
        // 1 pad + 7 status cells + 1 gap + a 4-cell truncated target + 1 pad
        // fill a 14-cell row: the target keeps its `on <host>` head and the
        // ellipsis.
        let line = header_line(14, NowPlayingStatus::Playing, "music-box");
        assert!(line.starts_with(" PLAYING"), "status indented: {line:?}");
        assert!(line.contains("on "), "target present: {line:?}");
        assert!(
            line.ends_with("… "),
            "target truncated with trailing pad: {line:?}"
        );
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

    #[test]
    fn playing_carries_no_progress_cluster() {
        // The progress cluster (throbber + percent) is deleted from the
        // header: while playing it states only the status and the target.
        // Width 40 lays out as: 1 pad + `PLAYING` (x1-7) + 19 gap +
        // `on ` (x8-10 of the target run) + `music-box` + 1 pad.
        let line = header_line(40, NowPlayingStatus::Playing, "music-box");
        assert_eq!(
            line,
            format!(" PLAYING{}on music-box ", " ".repeat(19)),
            "status left, target right, nothing between: {line:?}"
        );
        assert!(!line.contains('%'));
    }

    #[test]
    fn hostname_is_green_when_local_and_aqua_when_remote() {
        // The hostname starts one cell left of the trailing pad; the `on `
        // prefix keeps the muted target style in both cases.
        for (host_is_remote, expected) in
            [(false, palette::STATUS_AVAILABLE), (true, palette::ACCENT)]
        {
            let mut term = Terminal::new(TestBackend::new(40, 1)).unwrap();
            term.draw(|f| {
                render_playback_header(
                    f,
                    Rect::new(0, 0, 40, 1),
                    NowPlayingStatus::Playing,
                    "music-box",
                    host_is_remote,
                )
            })
            .unwrap();
            let buf = term.backend().buffer();
            // Width 40 lays out as: 1 pad + `PLAYING` (x1-7) + 19 gap +
            // `on ` (x27-29) + `music-box` (x30-38) + 1 pad.
            assert_eq!(
                buf[(30, 0)].style().fg,
                Some(expected),
                "remote={host_is_remote}: the hostname carries the local/remote colour"
            );
            assert_eq!(
                buf[(27, 0)].style().fg,
                Some(palette::TEXT_MUTED),
                "remote={host_is_remote}: the `on ` prefix stays muted"
            );
        }
    }
}
