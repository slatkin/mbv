use super::{MediaList, MediaListRow, RowLocalInput, RowLocalOutcome};
use ratatui::layout::{Position, Rect};
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::Component;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridPaintPolicy {
    focused: bool,
}

impl GridPaintPolicy {
    pub const fn new(focused: bool) -> Self {
        Self { focused }
    }
    pub(crate) const fn focused(self) -> bool {
        self.focused
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GridCell<Target> {
    pub rect: Rect,
    pub target: Option<Target>,
}

struct GridPaint<Target> {
    claim_rect: Rect,
    content_rect: Rect,
    offset: usize,
    overflows: bool,
    cells: Vec<GridCell<Target>>,
}

/// Embedded two-column presentation over the canonical media-list owner.
/// Column count and cell width are arrangement policy supplied by the parent;
/// this presentation only executes that policy and retains the painted cells.
pub struct GridMediaList<Target> {
    core: MediaList<Target>,
    columns: usize,
    cell_width: u16,
    gap: u16,
    policy: GridPaintPolicy,
    configured_geometry: Option<(Rect, Rect)>,
    paint: Option<GridPaint<Target>>,
}

impl<Target> Default for GridMediaList<Target> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Target> GridMediaList<Target> {
    pub fn new() -> Self {
        Self::from_media_list(MediaList::new())
    }

    pub fn from_media_list(core: MediaList<Target>) -> Self {
        Self {
            core,
            columns: 2,
            cell_width: 0,
            gap: 2,
            policy: GridPaintPolicy::new(false),
            configured_geometry: None,
            paint: None,
        }
    }

    pub fn into_media_list(self) -> MediaList<Target> {
        self.core
    }

    pub fn set_columns(&mut self, columns: usize, cell_width: u16, gap: u16) {
        self.columns = columns.max(1);
        self.cell_width = cell_width;
        self.gap = gap;
        self.paint = None;
    }

    pub fn set_geometry(&mut self, claim_rect: Rect, content_rect: Rect) {
        self.configured_geometry = Some((claim_rect, content_rect));
        self.paint = None;
    }

    pub fn set_paint_policy(&mut self, policy: GridPaintPolicy) {
        self.policy = policy;
        self.paint = None;
    }

    pub fn rows(&self) -> &[MediaListRow<Target>] {
        self.core.rows()
    }
    pub fn selected_target(&self) -> Option<&Target> {
        self.core.selected_target()
    }
    pub fn cursor(&self) -> usize {
        self.core.cursor()
    }
    pub fn scroll(&self) -> usize {
        self.core.scroll()
    }
    pub(crate) fn columns(&self) -> usize {
        self.columns
    }
    pub fn set_content(&mut self, rows: Vec<MediaListRow<Target>>)
    where
        Target: Clone + PartialEq,
    {
        self.paint = None;
        self.core.set_content(rows);
    }
    pub fn select_target(&mut self, target: &Target) -> bool
    where
        Target: PartialEq,
    {
        self.paint = None;
        self.core.select_target(target)
    }
    pub fn delegate(
        &mut self,
        input: RowLocalInput,
        target: Option<Target>,
    ) -> RowLocalOutcome<Target>
    where
        Target: Clone + PartialEq,
    {
        let outcome = self.core.delegate(input, target);
        if !matches!(outcome, RowLocalOutcome::Unhandled) {
            self.paint = None;
        }
        outcome
    }

    pub fn current_claim_rect(&self) -> Option<Rect> {
        self.paint.as_ref().map(|p| p.claim_rect)
    }
    pub fn current_content_rect(&self) -> Option<Rect> {
        self.paint.as_ref().map(|p| p.content_rect)
    }
    pub fn current_cells(&self) -> Option<&[GridCell<Target>]> {
        self.paint.as_ref().map(|p| p.cells.as_slice())
    }

    pub fn current_flow_offset(&self) -> Option<usize> {
        self.paint.as_ref().map(|p| p.offset)
    }

    pub fn current_overflows(&self) -> Option<bool> {
        self.paint.as_ref().map(|p| p.overflows)
    }

    pub fn claims_current_point(&self, point: Position) -> bool {
        self.paint
            .as_ref()
            .is_some_and(|p| p.claim_rect.contains(point))
    }

    pub fn resolve_current_point(&self, point: Position) -> Option<&Target> {
        let paint = self.paint.as_ref()?;
        paint
            .cells
            .iter()
            .find(|cell| cell.rect.contains(point))
            .and_then(|cell| cell.target.as_ref())
    }

    pub fn begin_view(&mut self) {
        self.paint = None;
    }

    pub(crate) fn geometry(&self, area: Rect) -> (Rect, Rect) {
        self.configured_geometry.unwrap_or((area, area))
    }

    pub(crate) fn cells(&mut self, content: Rect) -> Vec<GridCell<Target>>
    where
        Target: Clone,
    {
        if content.is_empty() {
            return Vec::new();
        }
        let width = if self.cell_width == 0 {
            content
                .width
                .saturating_sub(self.gap * self.columns.saturating_sub(1) as u16)
                / self.columns as u16
        } else {
            self.cell_width
        };
        let viewport_rows = content.height as usize;
        let offset = self.core.scroll().min(
            self.core
                .rows()
                .len()
                .saturating_sub(viewport_rows * self.columns),
        );
        self.core.set_scroll(offset);
        self.core
            .rows()
            .iter()
            .enumerate()
            .skip(offset * self.columns)
            .take(viewport_rows * self.columns)
            .map(|(index, row)| {
                let slot = index - offset * self.columns;
                let col = slot % self.columns;
                let line = slot / self.columns;
                let x = content.x + (width + self.gap) * col as u16;
                GridCell {
                    rect: Rect {
                        x,
                        y: content.y + line as u16,
                        width,
                        height: 1,
                    },
                    target: row.selectable_target().cloned(),
                }
            })
            .collect()
    }

    pub(crate) fn finish_view(
        &mut self,
        claim_rect: Rect,
        content_rect: Rect,
        cells: Vec<GridCell<Target>>,
    ) {
        let total_rows = self.core.rows().len();
        let viewport_rows = content_rect.height as usize;
        self.paint = Some(GridPaint {
            claim_rect,
            content_rect,
            offset: self.core.scroll(),
            overflows: total_rows > viewport_rows * self.columns,
            cells,
        });
    }
}

impl<Target: Clone + PartialEq> Component for GridMediaList<Target> {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        crate::app::render::render_grid_media_list_component(frame, area, self, self.policy);
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
