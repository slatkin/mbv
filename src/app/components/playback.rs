use std::time::{Duration, Instant};

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};
use tuirealm::props::{AttrValue, Attribute, Props, QueryResult};
use tuirealm::state::State;

use super::legacy_input::to_crossterm_mouse_event;
use super::msg::{Msg, PlaybackRequest};
use super::user_event::UserEvent;
use crate::app::render::render_playback_chrome_content;
use crate::app::types_playback::PlaybackState;

/// Set when `App.skip_intro_end_ticks.is_some()`.
pub const ATTR_SKIP_INTRO_PROMPT_VISIBLE: Attribute =
    Attribute::Custom("skip_intro_prompt_visible");
/// Set when `App.next_up_item.is_some()`.
pub const ATTR_NEXT_UP_PROMPT_VISIBLE: Attribute = Attribute::Custom("next_up_prompt_visible");

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct PlaybackProjection {
    pub state: PlaybackState,
    pub title: Option<String>,
    pub player_area: Rect,
    pub status_area: Rect,
    pub show_controls: bool,
    pub focused: bool,
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
}

impl PlaybackComponent {
    pub fn new() -> Self {
        let mut props = Props::default();
        props.set(ATTR_SKIP_INTRO_PROMPT_VISIBLE, AttrValue::Flag(false));
        props.set(ATTR_NEXT_UP_PROMPT_VISIBLE, AttrValue::Flag(false));
        Self {
            projection: PlaybackProjection {
                state: PlaybackState::default(),
                title: None,
                player_area: Rect::default(),
                status_area: Rect::default(),
                show_controls: false,
                focused: false,
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
        let mouse = to_crossterm_mouse_event(event);
        let point = (mouse.column, mouse.row).into();
        match mouse.kind {
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
                if self.play_pause_area.contains(point) =>
            {
                Some(Msg::Playback(PlaybackRequest::TogglePlayPause))
            }
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
                if self.stop_area.contains(point) && self.projection.stop_available =>
            {
                Some(Msg::Playback(PlaybackRequest::Stop))
            }
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
                if self.next_area.contains(point) && self.projection.next_available =>
            {
                Some(Msg::Playback(PlaybackRequest::Next))
            }
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
                if self.seekbar_area.contains(point) =>
            {
                Some(Msg::Playback(PlaybackRequest::SeekTo(mouse.column)))
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
        let geometry = render_playback_chrome_content(frame, &self.projection);
        self.play_pause_area = geometry.play_pause_area;
        self.stop_area = geometry.stop_area;
        self.next_area = geometry.next_area;
        self.seekbar_area = geometry.seekbar_area;
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
