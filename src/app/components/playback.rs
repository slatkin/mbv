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

use super::msg::{Msg, PlaybackRequest};
use super::user_event::UserEvent;
use crate::app::layout::LayoutPlayback;
use crate::app::palette;
use crate::app::render::{render_player_panel, PlaybackRenderContext};
use crate::app::types_playback::PlaybackState;

/// Set while any blocking overlay is mounted.
pub const ATTR_BLOCKING_OVERLAY_ACTIVE: Attribute = Attribute::Custom("blocking_overlay_active");
/// Set while the active Emby library has inline Search open.
pub const ATTR_LIB_SEARCH_ACTIVE: Attribute = Attribute::Custom("lib_search_active");

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct PlaybackProjection {
    pub state: PlaybackState,
    pub title: Option<String>,
    pub player_area: Rect,
    pub status_area: Rect,
    pub show_controls: bool,
    pub focused: bool,
    pub player_h: u16,
    pub panel_bg: Color,
    pub narrow_player: bool,
    pub now_playing_title: Option<(String, Color)>,
    pub title_parts: Vec<(String, Color)>,
    pub status_indicators: Option<Vec<Span<'static>>>,
    pub throbber: Span<'static>,
    pub idle_feed_title: Option<(String, bool)>,
    pub use_nerd_fonts: bool,
    pub stop_available: bool,
    pub next_available: bool,
    pub volume: String,
    pub muted: bool,
}

pub struct PlaybackComponent {
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

impl PlaybackComponent {
    pub fn new() -> Self {
        let mut props = Props::default();
        props.set(ATTR_BLOCKING_OVERLAY_ACTIVE, AttrValue::Flag(false));
        props.set(ATTR_LIB_SEARCH_ACTIVE, AttrValue::Flag(false));
        Self {
            projection: PlaybackProjection {
                state: PlaybackState::default(),
                title: None,
                player_area: Rect::default(),
                status_area: Rect::default(),
                show_controls: false,
                focused: false,
                player_h: 0,
                panel_bg: palette::SURFACE_PLAYBACK,
                narrow_player: false,
                now_playing_title: None,
                title_parts: Vec::new(),
                status_indicators: None,
                throbber: Span::raw(""),
                idle_feed_title: None,
                use_nerd_fonts: false,
                stop_available: false,
                next_available: false,
                volume: String::new(),
                muted: false,
            },
            props,
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

    fn double_tap(last: &mut Option<Instant>) -> bool {
        let now = Instant::now();
        let result =
            last.is_some_and(|previous| now.duration_since(previous) < Duration::from_millis(300));
        *last = (!result).then_some(now);
        result
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
            MouseEventKind::Down(MouseButton::Left) if self.seekbar_area.contains(point) => {
                Some(Msg::Playback(PlaybackRequest::SeekTo(event.column)))
            }
            _ => None,
        }
    }
}

impl Default for PlaybackComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for PlaybackComponent {
    fn view(&mut self, frame: &mut Frame, _area: Rect) {
        let mut playback = LayoutPlayback {
            player_area: self.projection.player_area,
            ..LayoutPlayback::default()
        };
        render_player_panel(
            frame,
            PlaybackRenderContext {
                area: self.projection.player_area,
                playback: &mut playback,
                player_h: self.projection.player_h,
                show_controls: self.projection.show_controls,
                now_playing_title: self.projection.now_playing_title.clone(),
                panel_bg: self.projection.panel_bg,
                narrow_player: self.projection.narrow_player,
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

impl AppComponent<Msg, UserEvent> for PlaybackComponent {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            Event::Keyboard(key) => self.key(key),
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

    #[test]
    fn playback_chrome_transport_intent_is_typed_and_player_free() {
        let mut component = PlaybackComponent::new();
        assert!(component.on(&key(Key::Char('m'))).is_some());
        assert!(matches!(
            component.on(&key(Key::Right)),
            Some(Msg::Playback(PlaybackRequest::Next))
        ));
    }

    #[test]
    fn playback_chrome_projection_renders_without_player_authority() {
        let mut component = PlaybackComponent::new();
        component.set_projection(PlaybackProjection {
            state: PlaybackState::default(),
            title: Some("Example".into()),
            player_area: Rect::new(0, 0, 40, 3),
            status_area: Rect::new(0, 3, 40, 1),
            show_controls: true,
            focused: false,
            player_h: 3,
            panel_bg: palette::SURFACE_PLAYBACK,
            narrow_player: false,
            now_playing_title: Some(("Example".into(), palette::PLAYBACK_VALUE_FG)),
            title_parts: vec![("Example".into(), palette::PLAYBACK_VALUE_FG)],
            status_indicators: None,
            throbber: Span::raw(" "),
            idle_feed_title: None,
            use_nerd_fonts: false,
            stop_available: false,
            next_available: false,
            volume: "50%".into(),
            muted: false,
        });
        let mut terminal = Terminal::new(TestBackend::new(40, 4)).unwrap();
        terminal
            .draw(|frame| component.view(frame, frame.area()))
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
