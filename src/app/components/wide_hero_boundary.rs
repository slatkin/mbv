use crate::app::components::mouse::gesture::{MouseGesture, MouseGestureState};
use crate::app::components::msg::{Msg, ShellRequest};
use crate::app::components::UserEvent;
use crate::app::list_pane_width::normalize_list_pane_width;
use crate::app::palette;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, MouseButton, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

/// The Wide hero split's gap-columns boundary. Its gesture state is
/// deliberately private: the shell only receives resolved semantic widths.
///
/// The gap is the shared arrangement's existing `WIDE_HERO_PANE_GAP` gutter,
/// so this component paints it with the backdrop it already showed
/// (`palette::SURFACE_BACKDROP`, the right-panel backdrop), adding no divider,
/// gutter, hover treatment, or wider hit region. It is the sole painter and
/// gesture owner of those columns; the panes' own hit geometry excludes them.
pub struct WideHeroBoundaryComponent {
    area: Rect,
    pane_origin_x: u16,
    content_width: u16,
    width: u16,
    enabled: bool,
    gestures: MouseGestureState,
}

impl WideHeroBoundaryComponent {
    pub fn new() -> Self {
        Self {
            area: Rect::default(),
            pane_origin_x: 0,
            content_width: 0,
            width: 0,
            enabled: false,
            gestures: MouseGestureState::new(),
        }
    }

    pub fn sync(
        &mut self,
        area: Rect,
        pane_origin_x: u16,
        content_width: u16,
        width: u16,
        enabled: bool,
    ) {
        // Losing eligibility mid-drag (overlay mount, mode change, empty
        // content) clears the gesture state before the next delivery, so no
        // stale width can be emitted after eligibility ends.
        if !enabled {
            self.gestures = MouseGestureState::new();
        }
        self.area = area;
        self.pane_origin_x = pane_origin_x;
        self.content_width = content_width;
        self.width = width;
        self.enabled = enabled;
    }

    fn inside(&self, at: Position) -> bool {
        self.enabled && self.area.contains(at)
    }

    /// The pinned resolution: the list-pane width is the pointer column minus
    /// the pane origin, clamped to the shared arrangement's valid range. The
    /// near gap column is the exact edge (grabbing without motion resolves to
    /// the current width, so nothing changes); the outer gap column resolves
    /// one column wider.
    fn resolve(&self, at: Position) -> u16 {
        normalize_list_pane_width(
            Some(at.x.saturating_sub(self.pane_origin_x)),
            self.content_width,
        )
        .unwrap_or(self.width)
    }

    fn handle_mouse(&mut self, event: &tuirealm::event::MouseEvent) -> Option<Msg> {
        let at = Position::new(event.column, event.row);
        // A press outside the gap never arms a drag (the gesture state's drag
        // anchor stays unset), so pane gestures are untouched.
        if matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) && !self.inside(at) {
            return None;
        }
        let gesture = self.gestures.recognize(event)?;
        match gesture {
            // Press-and-release without motion changes nothing.
            MouseGesture::Click(_) if self.inside(at) => None,
            // A recognized `Drag` implies an armed press inside the gap, so
            // every drag resolves -- tracking necessarily continues outside
            // the gap once the pointer leaves it.
            MouseGesture::Drag { to, .. } => {
                let width = self.resolve(to);
                if width == self.width {
                    return None;
                }
                self.width = width;
                Some(Msg::Shell(ShellRequest::ResizeListPaneLive(width)))
            }
            // Live-only: there is nothing to persist, so `DragEnd` is a no-op.
            _ => None,
        }
    }
}

impl Default for WideHeroBoundaryComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for WideHeroBoundaryComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        if self.enabled && area.width > 0 && area.height > 0 {
            frame.render_widget(
                Block::default().style(Style::default().bg(palette::SURFACE_BACKDROP)),
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

impl AppComponent<Msg, UserEvent> for WideHeroBoundaryComponent {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            _ => None,
        }
    }
}
