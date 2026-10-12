//! The queue header row's status vocabulary: the status word for one
//! now-playing status and its semantic colour. The row itself paints
//! through `chrome_player::title::render_header_title` (the target icon,
//! the bracketed host, and this status word left, the title riding the
//! remaining space). It never carries progress: the transport below owns
//! the throbber, percent, and seekbar.

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

/// The status word's semantic colour: AQUA while playing, YELLOW while
/// paused, muted while idle.
pub(crate) fn status_color(status: NowPlayingStatus) -> ratatui::style::Color {
    match status {
        NowPlayingStatus::Playing => palette::Role::Accent.color(),
        NowPlayingStatus::Paused => palette::Role::TextFocusAccent.color(),
        NowPlayingStatus::Idle => palette::Role::TextMuted.color(),
    }
}
