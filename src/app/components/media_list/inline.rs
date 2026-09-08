use super::{
    InlineMediaBrowserPaintPolicy, ListCore, MediaListRow, RowGeometry, ViewportAnchor,
    WideViewport,
};
use ratatui::layout::{Position, Rect};
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::Component;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

/// Resolved geometry for one painted frame of an [`InlineMediaBrowser`]
/// (design.md D1). `detail_rows == 0` means the detail block did not fit the
/// painted viewport and the browser fell back to painting the ordinary
/// selected row; otherwise the block replaces the selected row in the flow.
pub struct InlineLayout<Target> {
    /// The authoritative flow geometry used by the painter and consumers.
    pub row_geometry: RowGeometry<Target>,
    /// Admitted detail-block height, or `0` on fallback.
    pub detail_rows: usize,
}

/// The read-only facts retained by one completed `InlineMediaBrowser::view`.
struct InlinePaintResult<Target> {
    claim_rect: Rect,
    content_rect: Rect,
    row_geometry: RowGeometry<Target>,
    selected_target: Option<Target>,
    selected_row_rect: Option<Rect>,
    detail_rect: Option<Rect>,
}

/// Embedded plain media browser that replaces the selected row with a
/// variable-height detail block when it fits, and falls back to the ordinary
/// row when it does not (design.md D1). Shares
/// [`WideMediaList`](super::WideMediaList)'s list mechanics through the
/// private [`ListCore`]; the fit admission, fallback, and replacement paint
/// geometry live in [`resolve_inline_layout`](Self::resolve_inline_layout).
/// Painting is performed by its `Component::view` through the render adapter.
pub struct InlineMediaBrowser<Target> {
    core: ListCore<Target>,
    policy: InlineMediaBrowserPaintPolicy,
    paint: Option<InlinePaintResult<Target>>,
}

impl<Target> Default for InlineMediaBrowser<Target> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Target> InlineMediaBrowser<Target> {
    pub fn new() -> Self {
        Self {
            core: ListCore::new(),
            policy: InlineMediaBrowserPaintPolicy::new(
                false,
                super::SelectedRowSurface::ListBackdrop,
                0,
            ),
            paint: None,
        }
    }

    fn invalidate_paint(&mut self) {
        self.paint = None;
    }

    /// Configure the semantic policy used by the next `view`.
    pub fn set_paint_policy(&mut self, policy: InlineMediaBrowserPaintPolicy) {
        self.policy = policy;
        self.invalidate_paint();
    }

    pub(crate) fn begin_view(&mut self) {
        self.invalidate_paint();
    }

    pub(crate) fn finish_view(
        &mut self,
        claim_rect: Rect,
        content_rect: Rect,
        row_geometry: RowGeometry<Target>,
        selected_row_rect: Option<Rect>,
        detail_rect: Option<Rect>,
    ) where
        Target: Clone,
    {
        self.core.set_scroll(row_geometry.offset());
        self.paint = Some(InlinePaintResult {
            claim_rect,
            content_rect,
            row_geometry,
            selected_target: self.core.selected_target().cloned(),
            selected_row_rect,
            detail_rect,
        });
    }

    pub fn current_claim_rect(&self) -> Option<Rect> {
        self.paint.as_ref().map(|paint| paint.claim_rect)
    }

    pub fn current_content_rect(&self) -> Option<Rect> {
        self.paint.as_ref().map(|paint| paint.content_rect)
    }

    pub fn current_selected_target(&self) -> Option<&Target> {
        self.paint
            .as_ref()
            .and_then(|paint| paint.selected_target.as_ref())
    }

    pub fn current_selected_row_rect(&self) -> Option<Rect> {
        self.paint
            .as_ref()
            .and_then(|paint| paint.selected_row_rect)
    }

    pub fn current_detail_rect(&self) -> Option<Rect> {
        self.paint.as_ref().and_then(|paint| paint.detail_rect)
    }

    pub fn claims_current_point(&self, point: Position) -> bool {
        self.current_claim_rect()
            .is_some_and(|rect| rect.contains(point))
    }

    pub fn resolve_current_point(&self, point: Position) -> Option<&Target> {
        let paint = self.paint.as_ref()?;
        if !paint.claim_rect.contains(point) {
            return None;
        }
        let flow_row = (point.y - paint.content_rect.y) as usize + paint.row_geometry.offset();
        match paint.row_geometry.source_row(flow_row) {
            Some(source_row) => self.core.rows().get(source_row)?.selectable_target(),
            None => match paint.row_geometry.selected_row() {
                Some(block_start) if paint.detail_rect.is_some() && flow_row == block_start => {
                    self.core.selected_target()
                }
                _ => None,
            },
        }
    }

    pub fn rows(&self) -> &[MediaListRow<Target>] {
        self.core.rows()
    }

    /// Number of selectable rows.
    pub fn selectable_len(&self) -> usize {
        self.core.selectable_len()
    }

    /// No selectable rows at all.
    pub fn is_empty(&self) -> bool {
        self.core.is_empty()
    }

    /// The cursor as an index into the selectable rows.
    pub fn cursor(&self) -> usize {
        self.core.cursor()
    }

    /// The display-row index the cursor currently points at.
    pub fn selected_display_row(&self) -> Option<usize> {
        self.core.selected_display_row()
    }

    /// The stable identity under the cursor.
    pub fn selected_target(&self) -> Option<&Target> {
        self.core.selected_target()
    }

    /// The resting scroll offset (pre height-aware clamp).
    pub fn scroll(&self) -> usize {
        self.core.scroll()
    }

    /// Store the offset a painter resolved, so the next frame resumes from it.
    pub fn set_scroll(&mut self, offset: usize) {
        self.invalidate_paint();
        self.core.set_scroll(offset);
    }

    /// Move the cursor by `delta` selectable rows, clamped to the ends.
    pub fn move_selection(&mut self, delta: i64) {
        self.invalidate_paint();
        self.core.move_selection(delta);
    }

    pub fn select_first(&mut self) {
        self.invalidate_paint();
        self.core.select_first();
    }

    pub fn select_last(&mut self) {
        self.invalidate_paint();
        self.core.select_last();
    }

    /// Place the cursor at selectable index `index`, clamped to the last row.
    pub fn select_index(&mut self, index: usize) {
        self.invalidate_paint();
        self.core.select_index(index);
    }

    /// The clamped ordinary-row viewport for a painted `viewport_height`,
    /// keeping the selected row on screen. This is the fallback flow and the
    /// geometry the [`ViewportAnchor`] is measured against.
    pub fn resolve_viewport(&self, viewport_height: usize) -> WideViewport {
        self.core.resolve_viewport(viewport_height)
    }

    /// Zero-based screen-row offset from the viewport top to the selected
    /// ordinary row, for the responsive [`ViewportAnchor`] hand-off
    /// (design.md D3).
    pub fn selected_row_offset(&self, viewport_height: usize) -> Option<usize> {
        self.core.selected_row_offset(viewport_height)
    }

    /// Resolve the replacement paint geometry for a painted `viewport_height`
    /// and a `desired_detail_rows` detail block. The block is admitted only
    /// when it is shorter than the viewport, leaving room for at least one
    /// ordinary row (mirrors `hero::inline_detail_flow`'s admission test in
    /// the render layer); otherwise the browser falls back to the ordinary
    /// selected row and this returns `detail_rows == 0`.
    pub fn resolve_inline_layout(
        &self,
        viewport_height: usize,
        desired_detail_rows: usize,
    ) -> InlineLayout<Target>
    where
        Target: Clone,
    {
        let height = viewport_height.max(1);
        let admit = match self.core.selected_display_row() {
            Some(row) if desired_detail_rows > 0 && desired_detail_rows < height => Some(row),
            _ => None,
        };

        match admit {
            Some(row) => {
                let lower_bound = (row + desired_detail_rows).saturating_sub(height).min(row);
                let mut offset = self.core.scroll().clamp(lower_bound, row);
                if matches!(
                    self.core.rows().get(offset.saturating_sub(1)),
                    Some(MediaListRow::Heading { .. })
                ) && offset > 0
                {
                    let header_offset = offset - 1;
                    let detail_end = row.saturating_sub(header_offset) + desired_detail_rows;
                    if detail_end <= height {
                        offset = header_offset;
                    }
                }
                InlineLayout {
                    row_geometry: RowGeometry::replacement(
                        self.core.rows(),
                        row,
                        desired_detail_rows,
                        offset,
                    ),
                    detail_rows: desired_detail_rows,
                }
            }
            None => {
                let viewport = self.core.resolve_viewport(height);
                InlineLayout {
                    row_geometry: RowGeometry::source(
                        self.core.rows(),
                        viewport.offset,
                        self.core.selected_display_row(),
                    ),
                    detail_rows: 0,
                }
            }
        }
    }

    /// Export the replacement flow geometry used for a painted frame.
    pub fn row_geometry(
        &self,
        viewport_height: usize,
        desired_detail_rows: usize,
    ) -> RowGeometry<Target>
    where
        Target: Clone,
    {
        self.resolve_inline_layout(viewport_height, desired_detail_rows)
            .row_geometry
    }

    /// Resolve a screen `point` inside the painter-supplied `list_area` to the
    /// target under it (design.md D6). `detail_rows` is the block height the
    /// parent painted; the control resolves against the same replacement flow.
    /// Returns `None` for a point outside `list_area` (horizontally too), a
    /// heading/spacer row, an inline detail-block continuation row, or a point
    /// past the last row.
    /// Claim the painted list region, including blank, heading, and detail rows.
    /// Parents use this to distinguish an in-region no-op from outside input.
    pub fn claims_point(&self, list_area: Rect, point: Position) -> bool {
        list_area.contains(point)
    }

    pub fn resolve_point(
        &self,
        list_area: Rect,
        detail_rows: usize,
        point: Position,
    ) -> Option<&Target>
    where
        Target: Clone,
    {
        if !self.claims_point(list_area, point) {
            return None;
        }
        let layout = self.resolve_inline_layout(list_area.height as usize, detail_rows);
        let geom = &layout.row_geometry;
        let flow_row = (point.y - list_area.y) as usize + geom.offset();
        match geom.source_row(flow_row) {
            Some(source_row) => self.core.rows().get(source_row)?.selectable_target(),
            None => match geom.selected_row() {
                // The detail block replaces the selected row; only its first
                // flow row carries the target, continuation rows resolve None.
                Some(block_start) if layout.detail_rows > 0 && flow_row == block_start => {
                    self.core.selected_target()
                }
                _ => None,
            },
        }
    }
}

impl<Target: Clone + PartialEq> InlineMediaBrowser<Target> {
    /// Replace the display rows, preserving the selected target where possible
    /// and locally clamping otherwise (design.md D3).
    pub fn set_content(&mut self, rows: Vec<MediaListRow<Target>>) {
        self.invalidate_paint();
        self.core.set_content(rows);
    }

    /// Move the cursor to `target` when it is present; returns whether it was.
    pub fn select_target(&mut self, target: &Target) -> bool {
        self.invalidate_paint();
        self.core.select_target(target)
    }

    /// Produce a [`ViewportAnchor`] from the current selection for a painted
    /// viewport height (design.md D3). `None` when nothing is selectable.
    pub fn viewport_anchor(&self, viewport_height: usize) -> Option<ViewportAnchor<Target>> {
        self.core.viewport_anchor(viewport_height)
    }

    /// Restore a [`ViewportAnchor`] at a painted viewport height: select the
    /// target if present, then place it at the requested offset where the
    /// ordinary-row geometry allows, clamping otherwise (design.md D3).
    pub fn apply_viewport_anchor(
        &mut self,
        anchor: &ViewportAnchor<Target>,
        viewport_height: usize,
    ) {
        self.invalidate_paint();
        self.core.apply_viewport_anchor(anchor, viewport_height);
    }
}

impl<Target: Clone> Component for InlineMediaBrowser<Target> {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        crate::app::render::render_inline_media_browser_component(frame, area, self, self.policy);
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
