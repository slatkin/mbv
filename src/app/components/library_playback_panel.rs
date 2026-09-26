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

use std::time::Instant;

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
use crate::app::state::types::playback::PlaybackState;
use mbv_core::playback_queue::PlaybackTitleParts;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::app) struct TransportAvailability {
    pub stop: bool,
    pub next: bool,
    pub previous: bool,
}

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
    pub idle_feed_title: Option<(String, bool)>,
    pub use_nerd_fonts: bool,
    pub availability: TransportAvailability,
}

pub struct LibraryPlaybackPanel {
    projection: PlaybackProjection,
    props: Props,
    play_pause_area: Rect,
    stop_area: Rect,
    next_area: Rect,
    prev_area: Rect,
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
                use_nerd_fonts: false,
                idle_feed_title: None,
                availability: TransportAvailability::default(),
            },
            props: Props::default(),
            play_pause_area: Rect::default(),
            stop_area: Rect::default(),
            next_area: Rect::default(),
            prev_area: Rect::default(),
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

    /// Test-only: the projected now-playing title parts the sync pass
    /// delivered (task 5.2).
    #[cfg(test)]
    pub(in crate::app) fn title_parts_for_test(&self) -> Option<PlaybackTitleParts> {
        self.projection.title_parts.clone()
    }

    fn key_result(key: &KeyEvent) -> LeafKeyResult {
        match Self::key(key) {
            Some(message) => LeafKeyResult::Consumed(Some(Box::new(message))),
            None if matches!(
                key.code,
                Key::Char(' ' | 'm' | '[' | ']' | '<' | '>') | Key::Esc | Key::Left | Key::Right
            ) =>
            {
                LeafKeyResult::Consumed(None)
            }
            None => LeafKeyResult::Unhandled,
        }
    }

    /// One semantic intent per press: `Space` and `Esc` fire their transport
    /// request immediately when the panel holds focus; the shell's deferred
    /// candidate only covers presses the leaf did not consume.
    fn key(key: &KeyEvent) -> Option<Msg> {
        if key.modifiers != KeyModifiers::NONE {
            return None;
        }
        let request = match key.code {
            Key::Char(' ') => PlaybackRequest::TogglePlayPause,
            Key::Esc => PlaybackRequest::Stop,
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

    fn mouse(&self, event: tuirealm::event::MouseEvent) -> Option<Msg> {
        let point = (event.column, event.row).into();
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) if self.play_pause_area.contains(point) => {
                Some(Msg::Playback(PlaybackRequest::TogglePlayPause))
            }
            MouseEventKind::Down(MouseButton::Left)
                if self.stop_area.contains(point) && self.projection.availability.stop =>
            {
                Some(Msg::Playback(PlaybackRequest::Stop))
            }
            MouseEventKind::Down(MouseButton::Left)
                if self.prev_area.contains(point) && self.projection.availability.previous =>
            {
                Some(Msg::Playback(PlaybackRequest::Previous))
            }
            MouseEventKind::Down(MouseButton::Left)
                if self.next_area.contains(point) && self.projection.availability.next =>
            {
                Some(Msg::Playback(PlaybackRequest::Next))
            }
            MouseEventKind::Down(MouseButton::Left)
                if self.seekbar_area.contains(point) && self.seekbar_area.width > 0 =>
            {
                let fraction = f64::from(event.column.saturating_sub(self.seekbar_area.x))
                    / f64::from(self.seekbar_area.width);
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
                controls: crate::app::render::PlaybackControls {
                    show: self.projection.show_controls,
                    use_nerd_fonts: self.projection.use_nerd_fonts,
                    availability: self.projection.availability,
                    panel_focused: self.projection.panel_focused,
                    progress: (
                        self.projection.state.position_ticks,
                        self.projection.state.runtime_ticks,
                        self.projection.state.paused,
                    ),
                    idle_feed_title: self.projection.idle_feed_title.clone(),
                },
                now_playing_title: self.projection.now_playing_title.clone(),
                panel: self.projection.panel,
                status_indicators: self.projection.status_indicators.clone(),
                title_parts: self.projection.title_parts.clone(),
                marquee_text: &mut self.marquee_text,
                marquee_started_at: &mut self.marquee_started_at,
            },
        );
        self.play_pause_area = playback.play_pause;
        self.stop_area = playback.stop;
        self.next_area = playback.next;
        self.prev_area = playback.prev;
        self.seekbar_area = playback.seekbar;
    }

    fn query(&self, attr: Attribute) -> Option<QueryResult<'_>> {
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
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            Event::Keyboard(key) => Self::key_result(key).into_option(),
            Event::Mouse(mouse) => self.mouse(*mouse),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mbv_core::playback_queue::{PlaybackTitlePart, PlaybackTitlePartRole};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use rstest::rstest;

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
            idle_feed_title: None,
            use_nerd_fonts: false,
            availability: super::TransportAvailability {
                stop: true,
                next: true,
                previous: true,
            },
        });
        let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
        terminal
            .draw(|frame| panel.view(frame, Rect::new(10, 5, 40, 4)))
            .unwrap();
        panel
    }

    /// The media-type families of the requirements table, as the projection
    /// carries them (title part, optional context part). The mapping itself is
    /// core's table (task 1.1); these fixtures pin the painted behaviour per
    /// media type.
    fn parts_for(title: &str, context: Option<&str>) -> PlaybackTitleParts {
        PlaybackTitleParts {
            title: PlaybackTitlePart {
                role: PlaybackTitlePartRole::Title,
                text: title.to_string(),
            },
            context: context.map(|text| PlaybackTitlePart {
                role: PlaybackTitlePartRole::Context,
                text: text.to_string(),
            }),
        }
    }

    fn projection_with_parts(parts: PlaybackTitleParts) -> PlaybackProjection {
        PlaybackProjection {
            state: PlaybackState::default(),
            show_controls: true,
            panel: palette::Surface::PlaybackPanel,
            panel_focused: false,
            // The painter paints the typed parts only over an attached
            // target's plain title; the parts replace it when present.
            now_playing_title: Some(("Fallback".into(), palette::PLAYBACK_VALUE_FG)),
            title_parts: Some(parts),
            status_indicators: None,
            idle_feed_title: None,
            use_nerd_fonts: false,
            availability: super::TransportAvailability {
                stop: true,
                next: true,
                previous: true,
            },
        }
    }

    /// Paint the panel and return the transport title row's text plus each
    /// cell's foreground (the strip's title row is placement row 1 of the
    /// band at y 5). The strip is 50 columns wide so every media type in the
    /// table fits its slot without the marquee; the marquee window itself is
    /// the painter test's proof (task 4.3).
    fn painted_title_row(parts: PlaybackTitleParts) -> (String, Vec<Color>) {
        let mut panel = LibraryPlaybackPanel::new();
        panel.set_projection(projection_with_parts(parts));
        let mut terminal = Terminal::new(TestBackend::new(70, 12)).unwrap();
        terminal
            .draw(|frame| panel.view(frame, Rect::new(10, 5, 50, 4)))
            .unwrap();
        let buf = terminal.backend().buffer();
        (
            (0..70).map(|x| buf[(x, 6)].symbol().to_string()).collect(),
            (0..70).map(|x| buf[(x, 6)].fg).collect(),
        )
    }

    fn assert_cells_carry(text: &str, fgs: &[Color], needle: &str, expected: Color, label: &str) {
        let start = text
            .find(needle)
            .unwrap_or_else(|| panic!("{label} not painted in the row: {text:?}"));
        for (i, _) in needle.char_indices() {
            assert_eq!(
                fgs[start + i],
                expected,
                "cell {i} of {label} must carry its role's fg: {text:?}"
            );
        }
    }

    /// The painted media-type table (tasks 4.1, 4.2, 4.4): two-part rows
    /// paint the context part — with its trailing space — in the yellow
    /// context role, then the title part in the aqua title role, delineated
    /// by exactly one space and no separator glyph; single-part rows paint
    /// wholly in the title role with no context part before or after them.
    #[rstest]
    #[case::emby_episode("Pilot", Some("Series"))]
    fn title_row_paints_each_media_types_parts_in_their_roles(
        #[case] title: &str,
        #[case] context: Option<&str>,
    ) {
        let (text, fgs) = painted_title_row(parts_for(title, context));
        assert_cells_carry(
            &text,
            &fgs,
            title,
            palette::PLAYBACK_TITLE_FG,
            "the title part",
        );
        if let Some(context) = context {
            // D3: exactly one space between the parts and no separator
            // glyph of any form. The context part paints first, the
            // title part after it.
            let joined = format!("{context} {title}");
            // The painted run must be exactly the one-space join, not
            // merely contain it: a wider delineation (e.g. a doubled
            // space) must fail here.
            let start = text.find(context).unwrap();
            let painted: String = text[start..].chars().take(joined.chars().count()).collect();
            assert_eq!(
                painted, joined,
                "exactly one space between the parts: {text:?}"
            );
            for separator in [" - ", " \u{2013} ", " \u{2014} ", " | ", " \u{2022} "] {
                assert!(
                    !text.contains(&format!("{context}{separator}{title}")),
                    "no separator glyph between the parts: {text:?}"
                );
            }
            // The context span owns its trailing space, so the context
            // text and the space paint in the context role.
            for i in 0..=context.chars().count() {
                assert_eq!(
                    fgs[start + i],
                    palette::PLAYBACK_CONTEXT_FG,
                    "the context part and the space paint in the context role: {text:?}"
                );
            }
        } else {
            // A single-part row paints no context part: nothing in the
            // context role precedes or follows the title run (the
            // audiobook case is task 4.4, design D5).
            let title_start = text.find(title).unwrap();
            if title_start > 0 {
                assert_ne!(
                    fgs[title_start - 1],
                    palette::PLAYBACK_CONTEXT_FG,
                    "no context part before the title: {text:?}"
                );
            }
            let after = title_start + title.chars().count();
            assert_ne!(
                fgs[after],
                palette::PLAYBACK_CONTEXT_FG,
                "no context part after the title: {text:?}"
            );
        }
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
    fn prev_and_next_clicks_resolve_against_their_own_painted_areas() {
        let mut panel = painted_panel();
        let (prev, next) = (panel.prev_area, panel.next_area);
        // Both glyphs paint and keep distinct hit rects: the prev control was
        // painted without one for as long as it has existed.
        assert!(prev.width > 0 && next.width > 0);
        assert!(prev.right() <= next.x, "prev={prev:?} next={next:?}");
        assert!(matches!(
            panel.on(&click(prev.x, prev.y)),
            Some(Msg::Playback(PlaybackRequest::Previous))
        ));
        assert!(matches!(
            panel.on(&click(next.x, next.y)),
            Some(Msg::Playback(PlaybackRequest::Next))
        ));
    }

    /// One press fires the transport intent — no repeated-press window
    /// remains (task 4.2 / semantic-input-arbitration).
    #[test]
    fn space_and_escape_fire_their_transport_intent_on_one_press() {
        let mut panel = LibraryPlaybackPanel::new();
        assert!(matches!(
            panel.on(&key(Key::Char(' '))),
            Some(Msg::Playback(PlaybackRequest::TogglePlayPause))
        ));
        assert!(matches!(
            panel.on(&key(Key::Char(' '))),
            Some(Msg::Playback(PlaybackRequest::TogglePlayPause))
        ));
        assert!(matches!(
            panel.on(&key(Key::Esc)),
            Some(Msg::Playback(PlaybackRequest::Stop))
        ));
    }
}
