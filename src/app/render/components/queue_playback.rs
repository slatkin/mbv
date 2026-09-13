//! The Queue playback panel's header row (task 3.5, design D10): one
//! always-painted row at the top of the queue column's content in every
//! queue-visible layout, idle included. Status left, playback target right,
//! on the chrome band. While playing, the throbber and percent ride right
//! of the status word; they never show while idle or paused.

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

/// The status word's semantic colour: FOAM while playing, YELLOW while
/// paused, muted while idle.
fn status_color(status: NowPlayingStatus) -> ratatui::style::Color {
    match status {
        NowPlayingStatus::Playing => palette::TEXT_METADATA,
        NowPlayingStatus::Paused => palette::TEXT_FOCUS_ACCENT,
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

/// Paints the header row into `area`: ` PLAYING`/` PAUSED`/` IDLE` on the left
/// (one space of text indent), `on <host> ` right-aligned (one space of
/// trailing text padding). The `on ` prefix stays muted; the hostname paints
/// green when local, aqua when remote. While playing, the throbber and
/// percent paint one space right of the status word; they never show while
/// idle or paused, or when both arrive empty. The throbber keeps its own
/// foreground; the percent paints muted. Pure painter over projected state.
pub(in crate::app) fn render_playback_header(
    f: &mut Frame,
    area: Rect,
    status: NowPlayingStatus,
    host: &str,
    host_is_remote: bool,
    throbber: &Span<'static>,
    pct: &str,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let bg = palette::surface_colors(palette::Surface::QueueOnlyPlaybackPanel, false).fill;
    let status_color = status_color(status);
    let playing = status == NowPlayingStatus::Playing;
    let status = header_status_word(status);
    let status_w = unicode_width::UnicodeWidthStr::width(status) as u16;
    let target = format!("on {host}");
    let target_w = target.width() as u16;
    // One space of text padding on each side inside the header band. The
    // content budget is the band minus those two cells; when the band is
    // too narrow to hold even the padding, paint the background only.
    let content_w = area.width.saturating_sub(2);
    let status_fits = status_w <= content_w;
    let status_w = if status_fits { status_w } else { 0 };
    // The progress cluster (one leading space plus throbber and percent)
    // shows only while playing with something to state. It outranks the
    // target, which truncates with an ellipsis to make room for it.
    let throb_w = throbber.content.width() as u16;
    let pct_w = pct.width() as u16;
    let progress_w = 1 + throb_w + pct_w;
    let mut remaining = content_w.saturating_sub(status_w);
    let show_progress = playing
        && status_fits
        && (!throbber.content.is_empty() || !pct.is_empty())
        && progress_w <= remaining;
    if show_progress {
        remaining -= progress_w;
    }
    // The gap between the status word (plus the progress cluster while
    // playing) and the right-aligned target; when the row is too narrow,
    // the target truncates with an ellipsis before it can overlap them. It
    // never drops, so the header keeps stating `on <host>`
    // (the queue-playback-panel delta).
    let avail = remaining;
    let (target, inner) = if !status_fits {
        (String::new(), 0)
    } else if target_w <= avail {
        (target, avail - target_w)
    } else if avail > 1 {
        (ellipsize(&target, avail - 1), 1)
    } else {
        (String::new(), 0)
    };
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
    if show_progress {
        spans.push(Span::styled(" ", bg_style));
        spans.push(Span::styled(
            throbber.content.to_string(),
            throbber.style.bg(bg),
        ));
        spans.push(Span::styled(
            pct.to_owned(),
            Style::default().fg(palette::TEXT_METADATA).bg(bg),
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
    let mut target_w2 = 0u16;
    if inner > 0 {
        spans.push(Span::styled(
            " ".repeat(inner as usize),
            Style::default().bg(bg),
        ));
    }
    if !target.is_empty() {
        target_w2 = target.width() as u16;
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
    let progress_cells = if show_progress { progress_w } else { 0 };
    let used_w = 1 + status_w + progress_cells + inner + target_w2;
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
        term.draw(|f| {
            render_playback_header(
                f,
                Rect::new(0, 0, width, 1),
                status,
                host,
                false,
                &Span::raw(""),
                "",
            )
        })
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
    fn progress_rides_right_of_playing_and_hides_otherwise() {
        fn line(status: NowPlayingStatus) -> String {
            let mut term = Terminal::new(TestBackend::new(40, 1)).unwrap();
            term.draw(|f| {
                render_playback_header(
                    f,
                    Rect::new(0, 0, 40, 1),
                    status,
                    "music-box",
                    false,
                    &Span::raw("~"),
                    "50%",
                )
            })
            .unwrap();
            let buf = term.backend().buffer();
            (0..40).map(|x| buf[(x, 0)].symbol().to_owned()).collect()
        }
        let playing = line(NowPlayingStatus::Playing);
        assert!(
            playing.starts_with(" PLAYING ~50%"),
            "progress rides right of PLAYING: {playing:?}"
        );
        assert!(
            playing.ends_with("on music-box "),
            "target still right: {playing:?}"
        );
        for status in [NowPlayingStatus::Paused, NowPlayingStatus::Idle] {
            let hidden = line(status);
            assert!(
                !hidden.contains('~') && !hidden.contains('%'),
                "{status:?} shows no progress: {hidden:?}"
            );
        }
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
                    &Span::raw(""),
                    "",
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
