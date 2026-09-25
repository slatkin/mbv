use super::{Cursored, PaintRetained, PaintRetainedState, Row, RowFlow, Viewported};
use ratatui::layout::{Position, Rect};
use ratatui::style::Color;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::Component;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

/// A closed semantic text role for one span in a three-line item.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThreeLineRole {
    #[default]
    Name,
    Kind,
    Detail,
    Status,
    Accent,
    /// Explicit-color badge (e.g. a nerd-font service glyph): the component
    /// resolves the color shell-side, and the painter preserves it on the
    /// selected row like `Accent`.
    Badge(Color),
}

/// One styled text span in a three-line item's presentation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreeLineSpan {
    pub text: String,
    pub role: ThreeLineRole,
}

impl ThreeLineSpan {
    pub fn new(text: impl Into<String>, role: ThreeLineRole) -> Self {
        Self {
            text: text.into(),
            role,
        }
    }
}

/// Target identity and presentation are independent of Service objects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreeLineItem<Target> {
    pub target: Target,
    pub lines: [Vec<ThreeLineSpan>; 3],
}

impl<Target> ThreeLineItem<Target> {
    pub fn new(target: Target, lines: [Vec<ThreeLineSpan>; 3]) -> Self {
        Self { target, lines }
    }
}

/// Embedded selectable three-line item flow. It is not independently mounted.
pub struct ThreeLineFlatList<Target> {
    items: Vec<ThreeLineItem<Target>>,
    selected: Option<Target>,
    offset: usize,
    gap: u16,
    focused: bool,
    paint: PaintRetainedState<Target>,
}

impl<Target> ThreeLineFlatList<Target> {
    pub fn new(gap: u16) -> Self {
        Self {
            items: Vec::new(),
            selected: None,
            offset: 0,
            gap,
            focused: false,
            paint: PaintRetainedState::new(),
        }
    }

    pub fn gap(&self) -> u16 {
        self.gap
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub(in crate::app) fn focused(&self) -> bool {
        self.focused
    }

    pub fn items(&self) -> &[ThreeLineItem<Target>] {
        &self.items
    }

    pub fn selected_target(&self) -> Option<&Target> {
        self.selected.as_ref()
    }

    pub fn set_content(&mut self, items: Vec<ThreeLineItem<Target>>)
    where
        Target: Clone + Eq,
    {
        let target = self.selected.clone();
        self.items = items;
        let flow = self.flow();
        if let Some(target) = target {
            if flow.position_of(&target).is_none() {
                self.first(&flow);
            }
        } else {
            self.first(&flow);
        }
        self.invalidate();
    }

    pub fn move_selection(&mut self, delta: isize)
    where
        Target: Clone + Eq,
    {
        let flow = self.flow();
        self.move_by(&flow, delta);
        self.invalidate();
    }

    pub fn select_target(&mut self, target: &Target) -> bool
    where
        Target: Clone + Eq,
    {
        let flow = self.flow();
        let found = Cursored::select_target(self, &flow, target);
        self.invalidate();
        found
    }

    pub fn resolve_point(&self, point: Position) -> Option<&Target> {
        PaintRetained::resolve_point(self, point)
    }

    pub fn invalidate_paint(&mut self) {
        self.invalidate();
    }

    fn flow(&self) -> RowFlow<Target>
    where
        Target: Clone,
    {
        RowFlow::new(
            self.items
                .iter()
                .map(|item| Row::selectable(item.target.clone()))
                .collect(),
        )
    }

    fn visible_items(&self, height: u16) -> usize {
        if height < 3 {
            0
        } else {
            1 + (height - 3) as usize / (3 + self.gap as usize)
        }
    }
}

impl<Target: Clone + Eq> Cursored<Target> for ThreeLineFlatList<Target> {
    fn selected_target(&self) -> Option<&Target> {
        self.selected.as_ref()
    }
    fn set_selected_target(&mut self, target: Option<&Target>) {
        self.selected = target.cloned();
    }
}

impl<Target: Clone + Eq> Viewported<Target> for ThreeLineFlatList<Target> {
    fn viewport_offset(&self) -> usize {
        self.offset
    }
    fn set_viewport_offset(&mut self, offset: usize) {
        self.offset = offset;
    }
}

impl<Target> PaintRetained<Target> for ThreeLineFlatList<Target> {
    fn paint_retained(&self) -> &PaintRetainedState<Target> {
        &self.paint
    }
    fn paint_retained_mut(&mut self) -> &mut PaintRetainedState<Target> {
        &mut self.paint
    }
}

impl<Target: Clone + Eq> Component for ThreeLineFlatList<Target> {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        crate::app::render::render_three_line_flat_list(frame, area, self);
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

impl<Target> ThreeLineFlatList<Target> {
    pub(in crate::app) fn painting_parts(&mut self, area: Rect) -> (usize, usize, usize)
    where
        Target: Clone + Eq,
    {
        let count = self.visible_items(area.height);
        let flow = self.flow();
        self.reconcile_viewport(&flow, count);
        (
            self.offset,
            count.min(self.items.len().saturating_sub(self.offset)),
            self.gap as usize,
        )
    }

    pub(in crate::app) fn publish(
        &mut self,
        area: Rect,
        rows: Vec<(Rect, Target)>,
        selected: Option<Rect>,
    ) {
        PaintRetained::finish(self, area, area, rows, selected);
    }

    pub(in crate::app) fn selected(&self) -> Option<&Target> {
        self.selected.as_ref()
    }
    pub(in crate::app) fn item(&self, index: usize) -> &ThreeLineItem<Target> {
        &self.items[index]
    }
    pub(in crate::app) fn begin_paint(&mut self) {
        self.begin();
    }
}

#[cfg(test)]
mod tests;
