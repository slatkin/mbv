//! The Queue playback panel's header row (task 3.5, design D10): one
//! always-painted row at the top of the queue column's content in every
//! queue-visible layout, idle included. Status left, `[host]` right
//! (`PLAYING [music]`), on the chrome band. It never
//! carries progress: the transport below owns the throbber, percent, and
//! seekbar.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

use mbv_theme as palette;
use mbv_ui_model::playback_target::NowPlayingStatus;
use mbv_ui_model::ui_util::trunc_str;

/// The header's status word for one now-playing status.
#[must_use]
pub fn header_status_word(status: NowPlayingStatus) -> &'static str {
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

/// Paints the header row into `area`: `PLAYING` left, `[host]` right
/// (` PLAYING [music] `, one space of text indent plus one space of
/// trailing text padding). The status word keeps its semantic colour; the
/// `[host]` chunk: the brackets always paint cream, the name inside paints
/// muted when local, aqua when remote. Pure painter over projected state.
pub fn render_playback_header(
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
    let status_w = u16::try_from(unicode_width::UnicodeWidthStr::width(status)).unwrap_or(u16::MAX);
    let host_inner_w = u16::try_from(host.width()).unwrap_or(u16::MAX);
    let full_host_w = host_inner_w.saturating_add(2);
    // One space of text padding on each side inside the header band. The
    // content budget is the band minus those two cells; when the band is
    // too narrow to hold even the status word, paint the background only.
    let content_w = area.width.saturating_sub(2);
    let status_fits = status_w <= content_w;
    let status_w = if status_fits { status_w } else { 0 };
    // The gap between the left-aligned status word and the right-aligned
    // host chunk; when the row is too narrow the hostname truncates with
    // an ellipsis (brackets kept) before it can overlap the status. An
    // empty host paints no chunk at all.
    let avail = content_w.saturating_sub(status_w);
    let (host_text, inner) = if !status_fits || host.is_empty() {
        (String::new(), 0)
    } else if full_host_w <= avail {
        (host.to_string(), avail - full_host_w)
    } else if avail >= 4 {
        (trunc_str(host, (avail - 3) as usize), 1)
    } else {
        (String::new(), 0)
    };
    let host_w = u16::try_from(host_text.width()).unwrap_or(u16::MAX);
    let chunk_w = if host_text.is_empty() {
        0
    } else {
        host_w.saturating_add(2)
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
    let host_style = Style::default()
        .fg(if host_is_remote {
            palette::ACCENT
        } else {
            palette::TEXT_MUTED
        })
        .bg(bg);
    if inner > 0 {
        spans.push(Span::styled(
            " ".repeat(inner as usize),
            Style::default().bg(bg),
        ));
    }
    if !host_text.is_empty() {
        let bracket_style = Style::default().fg(palette::TEXT_EMPHASIS).bg(bg);
        spans.push(Span::styled("[", bracket_style));
        spans.push(Span::styled(host_text, host_style));
        spans.push(Span::styled("]", bracket_style));
    }
    // Trailing text padding: fill whatever cells remain (at least the one
    // reserved pad cell, more when the host truncated short) with the
    // band background so the text never touches the right edge.
    let used_w = 1 + status_w + inner + chunk_w;
    let trailing = area.width.saturating_sub(used_w);
    if trailing > 0 {
        spans.push(Span::styled(" ".repeat(trailing as usize), bg_style));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)),
        area,
    );
}
