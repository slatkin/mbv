//! The Queue playback panel's header row (task 3.5, design D10): one
//! always-painted row at the top of the queue column's content in every
//! queue-visible layout, idle included. ` [mbv]` left, the status word
//! right (`[mbv] ... IDLE`), on the chrome band. It never
//! carries progress: the transport below owns the throbber, percent, and
//! seekbar.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::chrome_player::brand_row_spans;
use mbv_theme as palette;
use mbv_ui_model::playback_target::NowPlayingStatus;

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

/// Paints the header row into `area`: ` [mbv]` left-anchored (the shared
/// brand anchor), the status word right-anchored. The idle `IDLE` paints
/// muted grey; playing/paused keep their semantic colours. Pure painter over
/// projected state.
pub fn render_playback_header(f: &mut Frame, area: Rect, status: NowPlayingStatus) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let bg = palette::surface_colors(palette::Surface::QueueOnlyPlaybackPanel, false).fill;
    let right = vec![Span::styled(
        header_status_word(status),
        Style::default()
            .fg(status_color(status))
            .bg(bg)
            .add_modifier(Modifier::BOLD),
    )];
    let spans = brand_row_spans(bg, area.width as usize, right);
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)),
        area,
    );
}
