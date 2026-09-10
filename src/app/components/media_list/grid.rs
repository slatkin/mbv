use super::{MediaList, MediaListRow, RowLocalInput, RowLocalOutcome, ViewportAnchor};
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
            // One column until the arrangement configures its policy before
            // the first paint; a pre-paint input stride must not claim a
            // two-column policy the parent has not supplied.
            columns: 1,
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

    /// Store the shared owner's display-line scroll offset.
    pub fn set_scroll(&mut self, offset: usize) {
        self.paint = None;
        let max = self.display_lines().len().saturating_sub(1);
        self.core.set_scroll(offset.min(max));
    }

    /// The painted display lines of the grid: each line holds up to
    /// `columns` selectable source rows; a `Heading`/`Spacer` row flushes the
    /// current line and occupies a full line alone (the established bucketed
    /// two-column catalog policy: a bucket always starts a fresh item row).
    pub(crate) fn display_lines(&self) -> Vec<Vec<usize>> {
        let mut lines: Vec<Vec<usize>> = Vec::new();
        let mut current: Vec<usize> = Vec::new();
        for (index, row) in self.core.rows().iter().enumerate() {
            if row.selectable_target().is_some() {
                if current.len() == self.columns {
                    lines.push(std::mem::take(&mut current));
                }
                current.push(index);
            } else {
                if !current.is_empty() {
                    lines.push(std::mem::take(&mut current));
                }
                lines.push(vec![index]);
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
        lines
    }

    /// The display line holding the selected source row and the selected
    /// column within that line.
    fn selected_line(&self, lines: &[Vec<usize>]) -> Option<(usize, usize)> {
        let row = self.core.selected_display_row()?;
        let line = lines.iter().position(|line| line.contains(&row))?;
        let col = lines[line].iter().position(|&r| r == row).unwrap_or(0);
        Some((line, col))
    }

    /// Resolve the shared display-line viewport for a grid's line capacity.
    pub fn resolve_viewport(&self, viewport_height: usize) -> super::WideViewport {
        let height = viewport_height.max(1);
        let lines = self.display_lines();
        let total_rows = lines.len();
        let mut offset = self.core.scroll().min(total_rows.saturating_sub(height));
        if let Some((line, _)) = self.selected_line(&lines) {
            if line < offset {
                offset = line;
            } else if line >= offset + height {
                offset = line + 1 - height;
            }
        }
        super::WideViewport {
            offset,
            height,
            total_rows,
        }
    }

    /// Zero-based screen-row offset from the viewport top to the selected
    /// row, for the responsive [`ViewportAnchor`] hand-off (design.md D3).
    pub fn selected_row_offset(&self, viewport_height: usize) -> Option<usize> {
        let lines = self.display_lines();
        let (line, _) = self.selected_line(&lines)?;
        Some(line.saturating_sub(self.resolve_viewport(viewport_height).offset))
    }

    /// Restore a [`ViewportAnchor`] at a painted viewport height: select the
    /// target when present, then place its line at the requested offset where
    /// the geometry allows, clamping otherwise (design.md D3).
    pub fn apply_viewport_anchor(&mut self, anchor: &ViewportAnchor<Target>, viewport_height: usize)
    where
        Target: PartialEq,
    {
        self.paint = None;
        self.core.select_target(&anchor.selected_target);
        let height = viewport_height.max(1);
        let lines = self.display_lines();
        let Some((line, _)) = self.selected_line(&lines) else {
            return;
        };
        let max = lines.len().saturating_sub(height);
        let offset = line.saturating_sub(anchor.selected_row_offset).min(max);
        self.core.set_scroll(offset);
    }

    /// Move the selection by `item_rows` painted item rows, preserving the
    /// selected column and falling back to the nearest cell of the target row
    /// (the established two-column catalog traversal policy, including its
    /// ragged trailing-row clamp). Header/spacer lines do not participate.
    pub fn move_item_rows(&mut self, item_rows: i64)
    where
        Target: Clone + PartialEq,
    {
        let rows = self.core.rows();
        let item_lines: Vec<Vec<usize>> = self
            .display_lines()
            .into_iter()
            .filter(|line| {
                line.iter()
                    .any(|&source| rows[source].selectable_target().is_some())
            })
            .collect();
        let Some((line, col)) = self.selected_line(&item_lines) else {
            return;
        };
        let target_line = if item_rows < 0 {
            line.saturating_sub(item_rows.unsigned_abs() as usize)
        } else {
            (line + item_rows as usize).min(item_lines.len().saturating_sub(1))
        };
        let target_row = item_lines
            .get(target_line)
            .and_then(|cells| cells.get(col).or(cells.last()))
            .copied();
        if let Some(target_row) = target_row {
            if let Some(target) = rows[target_row].selectable_target().cloned() {
                self.core.select_target(&target);
            }
        }
    }

    pub(crate) fn columns(&self) -> usize {
        self.columns
    }

    /// No selectable rows at all.
    pub fn is_empty(&self) -> bool {
        self.core.is_empty()
    }

    /// Move the cursor by `delta` selectable rows, clamped to the ends.
    pub fn move_selection(&mut self, delta: i64) {
        self.core.move_selection(delta);
    }

    pub fn select_first(&mut self) {
        self.core.select_first();
    }

    pub fn select_last(&mut self) {
        self.core.select_last();
    }

    /// Place the cursor at selectable index `index`, clamped to the last row.
    pub fn select_index(&mut self, index: usize) {
        self.core.select_index(index);
    }
    pub fn set_content(&mut self, rows: Vec<MediaListRow<Target>>)
    where
        Target: Clone + PartialEq,
    {
        self.paint = None;
        self.core.set_content(rows);
    }
    /// Move the cursor to `target` when it is present; returns whether it was.
    /// Selection does not change row flow geometry, so a completed view
    /// remains valid for pointer gestures until the next view begins (matching
    /// the Wide presentation's retained-result contract).
    pub fn select_target(&mut self, target: &Target) -> bool
    where
        Target: PartialEq,
    {
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

    pub(crate) fn cells(&mut self, content: Rect) -> Vec<(GridCell<Target>, usize)>
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
        // The shared owner stores display-line offsets. Grid resolves that
        // offset in line space and skips whole lines while laying out cells.
        let offset = self.resolve_viewport(viewport_rows).offset;
        self.core.set_scroll(offset);
        let mut cells = Vec::new();
        for (line_index, line) in self
            .display_lines()
            .iter()
            .skip(offset)
            .take(viewport_rows)
            .enumerate()
        {
            let y = content.y + line_index as u16;
            for (col, &source) in line.iter().enumerate() {
                if self.core.rows()[source].selectable_target().is_none() {
                    // Header/spacer lines occupy the full display line and
                    // resolve no cell.
                    continue;
                }
                let x = content.x + (width + self.gap) * col as u16;
                cells.push((
                    GridCell {
                        rect: Rect {
                            x,
                            y,
                            width,
                            height: 1,
                        },
                        target: self.core.rows()[source].selectable_target().cloned(),
                    },
                    source,
                ));
            }
        }
        cells
    }

    pub(crate) fn finish_view(
        &mut self,
        claim_rect: Rect,
        content_rect: Rect,
        cells: Vec<(GridCell<Target>, usize)>,
    ) {
        let total_lines = self.display_lines().len();
        let viewport_rows = content_rect.height as usize;
        self.paint = Some(GridPaint {
            claim_rect,
            content_rect,
            offset: self.core.scroll(),
            overflows: total_lines > viewport_rows,
            cells: cells.into_iter().map(|(cell, _)| cell).collect(),
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
