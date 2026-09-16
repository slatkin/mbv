//! The Library playback panel (task 4.1, design D10): the right-column
//! playback strip, mounted only when the queue column is hidden — there it
//! is the frame's one transport. It owns its placement around the shared
//! width-driven transport arrangement (a `PLAYER_BOX_HEIGHT` band between
//! the tab bar and the library: no header row, no visual slot) while the
//! arrangement decides what the transport shows at the panel's width, the
//! same arrangement the Queue playback panel's queue-column transport calls.
//! Transport hit geometry is the panel's own retained paint state.
//!
//! `PlaybackProjection` — the shared transport projection both playback
//! panels consume — is also defined here.

use std::time::{Duration, Instant};

use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::text::Span;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, Props, QueryResult};
use tuirealm::state::State;

use super::msg::{LeafKeyResult, Msg, PlaybackRequest};
use super::user_event::UserEvent;
use crate::app::palette;
use crate::app::render::arrangements::chrome::PLAYER_BOX_HEIGHT;
use crate::app::render::PlaybackStripAreas;
use crate::app::render::{render_player_panel, PlaybackRenderContext};
use crate::app::types_playback::PlaybackState;
use mbv_core::playback_queue::PlaybackTitleParts;

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct PlaybackProjection {
    pub state: PlaybackState,
    pub show_controls: bool,
    /// The panel surface plus the site's own focus bit; the painter resolves
    /// the fill through the surface table instead of carrying a bare colour.
    pub panel: palette::Surface,
    pub panel_focused: bool,
    pub now_playing_title: Option<(String, Color)>,
    /// The typed now-playing title parts with their closed roles (D6); the
    /// painter resolves a role to a colour. `None` when the attached target
    /// is not addressable as a local queue item — `now_playing_title` then
    /// carries the plain fallback title.
    pub title_parts: Option<PlaybackTitleParts>,
    pub status_indicators: Option<Vec<Span<'static>>>,
    pub throbber: Span<'static>,
    pub idle_feed_title: Option<(String, bool)>,
    pub use_nerd_fonts: bool,
    pub stop_available: bool,
    pub next_available: bool,
}

pub struct LibraryPlaybackPanel {
    projection: PlaybackProjection,
    props: Props,
    last_space: Option<Instant>,
    last_escape: Option<Instant>,
    play_pause_area: Rect,
    stop_area: Rect,
    next_area: Rect,
    seekbar_area: Rect,
    marquee_text: String,
    marquee_started_at: Instant,
}

impl LibraryPlaybackPanel {
    pub fn new() -> Self {
        Self {
            projection: PlaybackProjection {
                state: PlaybackState::default(),
                show_controls: false,
                // The pre-sync default: the panel's resting fill (the table's
                // `PlaybackPanel` row, bool false).
                panel: palette::Surface::PlaybackPanel,
                panel_focused: false,
                now_playing_title: None,
                title_parts: None,
                status_indicators: None,
                throbber: Span::raw(""),
                idle_feed_title: None,
                use_nerd_fonts: false,
                stop_available: false,
                next_available: false,
            },
            props: Props::default(),
            last_space: None,
            last_escape: None,
            play_pause_area: Rect::default(),
            stop_area: Rect::default(),
            next_area: Rect::default(),
            seekbar_area: Rect::default(),
            marquee_text: String::new(),
            marquee_started_at: Instant::now(),
        }
    }

    pub(in crate::app) fn set_projection(&mut self, projection: PlaybackProjection) {
        self.projection = projection;
    }

    /// Test-only: the retained transport hit geometry.
    #[cfg(test)]
    pub(in crate::app) fn transport_hits(&self) -> (Rect, Rect) {
        (self.play_pause_area, self.seekbar_area)
    }

    fn double_tap(last: &mut Option<Instant>) -> bool {
        let now = Instant::now();
        let result =
            last.is_some_and(|previous| now.duration_since(previous) < Duration::from_millis(300));
        *last = (!result).then_some(now);
        result
    }

    fn key_result(&mut self, key: &KeyEvent) -> LeafKeyResult {
        match self.key(key) {
            Some(message) => LeafKeyResult::Consumed(Some(message)),
            None if matches!(
                key.code,
                Key::Char(' ')
                    | Key::Esc
                    | Key::Left
                    | Key::Right
                    | Key::Char('m' | '[' | ']' | '<' | '>')
            ) =>
            {
                LeafKeyResult::Consumed(None)
            }
            None => LeafKeyResult::Unhandled,
        }
    }

    fn key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if key.modifiers != KeyModifiers::NONE {
            return None;
        }
        let request = match key.code {
            Key::Char(' ') if Self::double_tap(&mut self.last_space) => {
                PlaybackRequest::TogglePlayPause
            }
            Key::Esc if Self::double_tap(&mut self.last_escape) => PlaybackRequest::Stop,
            Key::Left => PlaybackRequest::Previous,
            Key::Right => PlaybackRequest::Next,
            Key::Char('m') => PlaybackRequest::ToggleMute,
            Key::Char('[') => PlaybackRequest::VolumeDelta(-5),
            Key::Char(']') => PlaybackRequest::VolumeDelta(5),
            Key::Char('<') => PlaybackRequest::CycleAudio,
            Key::Char('>') => PlaybackRequest::CycleSubtitle,
            _ => return None,
        };
        Some(Msg::Playback(request))
    }

    fn mouse(&self, event: &tuirealm::event::MouseEvent) -> Option<Msg> {
        let point = (event.column, event.row).into();
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) if self.play_pause_area.contains(point) => {
                Some(Msg::Playback(PlaybackRequest::TogglePlayPause))
            }
            MouseEventKind::Down(MouseButton::Left)
                if self.stop_area.contains(point) && self.projection.stop_available =>
            {
                Some(Msg::Playback(PlaybackRequest::Stop))
            }
            MouseEventKind::Down(MouseButton::Left)
                if self.next_area.contains(point) && self.projection.next_available =>
            {
                Some(Msg::Playback(PlaybackRequest::Next))
            }
            MouseEventKind::Down(MouseButton::Left)
                if self.seekbar_area.contains(point) && self.seekbar_area.width > 0 =>
            {
                let fraction = event.column.saturating_sub(self.seekbar_area.x) as f64
                    / self.seekbar_area.width as f64;
                Some(Msg::Playback(PlaybackRequest::SeekTo(fraction)))
            }
            _ => None,
        }
    }
}

impl Default for LibraryPlaybackPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for LibraryPlaybackPanel {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        // The strip's placement is the `RootFrame.library_playback` band
        // (`PLAYER_BOX_HEIGHT` tall); it carries no header row and no visual
        // slot — the shared width-driven transport arrangement decides which
        // transport rows and indicators the band shows at its width (the same
        // arrangement the queue-column transport calls, D10), and
        // `render_player_panel` is its leaf painter.
        let player_h = area.height.min(PLAYER_BOX_HEIGHT);
        let mut playback = PlaybackStripAreas::default();
        render_player_panel(
            frame,
            PlaybackRenderContext {
                area,
                playback: &mut playback,
                player_h,
                show_controls: self.projection.show_controls,
                now_playing_title: self.projection.now_playing_title.clone(),
                panel: self.projection.panel,
                panel_focused: self.projection.panel_focused,
                progress: (
                    self.projection.state.position_ticks,
                    self.projection.state.runtime_ticks,
                    self.projection.state.paused,
                ),
                use_nerd_fonts: self.projection.use_nerd_fonts,
                stop_available: self.projection.stop_available,
                next_available: self.projection.next_available,
                status_indicators: self.projection.status_indicators.clone(),
                throbber: self.projection.throbber.clone(),
                title_parts: self.projection.title_parts.clone(),
                idle_feed_title: self.projection.idle_feed_title.clone(),
                marquee_text: &mut self.marquee_text,
                marquee_started_at: &mut self.marquee_started_at,
            },
        );
        self.play_pause_area = playback.play_pause_area;
        self.stop_area = playback.stop_area;
        self.next_area = playback.next_area;
        self.seekbar_area = playback.seekbar_area;
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        self.props.get_for_query(attr)
    }
    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        self.props.set(attr, value);
    }
    fn state(&self) -> State {
        State::None
    }
    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for LibraryPlaybackPanel {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            Event::Keyboard(key) => self.key_result(key).into_option(),
            Event::Mouse(mouse) => self.mouse(mouse),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn key(code: Key) -> Event<UserEvent> {
        Event::Keyboard(KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
        })
    }

    fn click(column: u16, row: u16) -> Event<UserEvent> {
        Event::Mouse(tuirealm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        })
    }

    fn painted_panel() -> LibraryPlaybackPanel {
        let mut panel = LibraryPlaybackPanel::new();
        panel.set_projection(PlaybackProjection {
            state: PlaybackState::default(),
            show_controls: true,
            panel: palette::Surface::PlaybackPanel,
            panel_focused: false,
            now_playing_title: Some(("Example".into(), palette::PLAYBACK_VALUE_FG)),
            title_parts: None,
            status_indicators: None,
            throbber: Span::raw(" "),
            idle_feed_title: None,
            use_nerd_fonts: false,
            stop_available: true,
            next_available: true,
        });
        let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
        terminal
            .draw(|frame| panel.view(frame, Rect::new(10, 5, 40, 4)))
            .unwrap();
        panel
    }

    #[test]
    fn seekbar_click_resolves_a_fraction_against_the_painted_seekbar_area() {
        let mut panel = painted_panel();
        // seekbar_area == panel row: x 10, width 40. Column 30 -> 0.5.
        let message = panel.on(&click(30, 5));
        assert!(matches!(
            message,
            Some(Msg::Playback(PlaybackRequest::SeekTo(f))) if (f - 0.5).abs() < 1e-6
        ));
    }

    #[test]
    fn transport_button_click_emits_its_typed_intent() {
        let mut panel = painted_panel();
        // Play/pause glyph starts at the panel's x + 1 on the title row.
        assert!(matches!(
            panel.on(&click(11, 6)),
            Some(Msg::Playback(PlaybackRequest::TogglePlayPause))
        ));
    }

    #[test]
    fn playback_chrome_transport_intent_is_typed_and_player_free() {
        let mut panel = LibraryPlaybackPanel::new();
        assert!(panel.on(&key(Key::Char('m'))).is_some());
        assert!(matches!(
            panel.on(&key(Key::Right)),
            Some(Msg::Playback(PlaybackRequest::Next))
        ));
    }

    #[test]
    fn playback_chrome_projection_renders_without_player_authority() {
        let mut panel = LibraryPlaybackPanel::new();
        panel.set_projection(PlaybackProjection {
            state: PlaybackState::default(),
            show_controls: true,
            panel: palette::Surface::PlaybackPanel,
            panel_focused: false,
            now_playing_title: Some(("Example".into(), palette::PLAYBACK_VALUE_FG)),
            title_parts: None,
            status_indicators: None,
            throbber: Span::raw(" "),
            idle_feed_title: None,
            use_nerd_fonts: false,
            stop_available: false,
            next_available: false,
        });
        let mut terminal = Terminal::new(TestBackend::new(40, 4)).unwrap();
        terminal
            .draw(|frame| panel.view(frame, frame.area()))
            .unwrap();
        let output: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol().to_owned())
            .collect();
        assert!(output.contains("Example"));
    }
}
