use crate::app::components::mouse::gesture::{MouseGesture, MouseGestureState};
use crate::app::components::msg::{Msg, QueueRequest};
use crate::app::components::UserEvent;
use crate::app::palette;
use crate::app::queue_column_width::normalize_queue_column_width;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, MouseButton, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

/// The one-column Queue-side root boundary. Its gesture state is deliberately
/// private: the shell only receives resolved semantic widths.
pub struct QueueBoundaryComponent {
    area: Rect,
    frame_left: u16,
    terminal_width: u16,
    focused: bool,
    enabled: bool,
    changed: bool,
    width: u16,
    gestures: MouseGestureState,
}

impl QueueBoundaryComponent {
    pub fn new() -> Self {
        Self {
            area: Rect::default(),
            frame_left: 0,
            terminal_width: 0,
            focused: false,
            enabled: false,
            changed: false,
            width: 0,
            gestures: MouseGestureState::new(),
        }
    }

    pub fn sync(
        &mut self,
        area: Rect,
        frame_left: u16,
        terminal_width: u16,
        width: u16,
        focused: bool,
        enabled: bool,
    ) {
        if !enabled {
            self.gestures = MouseGestureState::new();
            self.changed = false;
        }
        self.area = area;
        self.frame_left = frame_left;
        self.terminal_width = terminal_width;
        self.width = width;
        self.focused = focused;
        self.enabled = enabled;
    }

    fn inside(&self, at: Position) -> bool {
        self.enabled && self.area.contains(at)
    }

    fn resolve(&self, at: Position) -> u16 {
        normalize_queue_column_width(
            at.x.saturating_sub(self.frame_left).saturating_add(1),
            self.terminal_width,
        )
    }

    fn handle_mouse(&mut self, event: &tuirealm::event::MouseEvent) -> Option<Msg> {
        if matches!(event.kind, MouseEventKind::Down(MouseButton::Left))
            && !self.inside(Position {
                x: event.column,
                y: event.row,
            })
        {
            return None;
        }
        let gesture = self.gestures.recognize(event)?;
        match gesture {
            MouseGesture::Click(_)
                if self.inside(Position {
                    x: event.column,
                    y: event.row,
                }) =>
            {
                self.changed = false;
                None
            }
            MouseGesture::Drag { to, .. }
                if self.changed || self.inside(to) || self.area.width == 1 =>
            {
                let width = self.resolve(to);
                if width == self.width {
                    return None;
                }
                self.width = width;
                self.changed = true;
                Some(Msg::Queue(QueueRequest::ResizeColumnLive(width)))
            }
            MouseGesture::DragEnd if self.changed => {
                self.changed = false;
                Some(Msg::Queue(QueueRequest::ResizeColumnEnd(self.width)))
            }
            MouseGesture::DragEnd => {
                self.changed = false;
                None
            }
            _ => None,
        }
    }
}

impl Default for QueueBoundaryComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for QueueBoundaryComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        if self.enabled && area.width > 0 && area.height > 0 {
            frame.render_widget(
                Block::default()
                    .style(Style::default().bg(palette::resolve_surface_focus(self.focused))),
                area,
            );
        }
    }
    fn query<'a>(&'a self, _attr: Attribute) -> Option<QueryResult<'a>> {
        None
    }
    fn attr(&mut self, _attr: Attribute, _value: AttrValue) {}
    fn state(&self) -> State {
        State::None
    }
    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for QueueBoundaryComponent {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            _ => None,
        }
    }
}
