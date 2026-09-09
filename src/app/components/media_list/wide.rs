use super::{
    letter_grouped_rows, ListCore, MediaListRow, RowGeometry, ViewportAnchor,
    WideMediaListPaintPolicy, WideViewport,
};
use ratatui::layout::{Position, Rect};
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::Component;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

/// The read-only facts retained by one completed `WideMediaList::view`.
struct WidePaintResult<Target> {
    claim_rect: Rect,
    content_rect: Rect,
    row_geometry: RowGeometry<Target>,
    selected_target: Option<Target>,
    selected_row_rect: Option<Rect>,
}

/// Embedded plain fixed-height, one-column media list: owns the display-row
/// list, the selectable index over it, the cursor, and the resting scroll
/// offset through the shared [`ListCore`]. It has no mouse hit-resolution API
/// and accepts no column-count or inline-detail options (design.md D1).
/// Painting is performed by its `Component::view` through the render adapter;
/// current-frame point resolution is retained alongside the painted flow.
pub struct WideMediaList<Target> {
    core: ListCore<Target>,
    policy: WideMediaListPaintPolicy,
    configured_geometry: Option<(Rect, Rect)>,
    paint: Option<WidePaintResult<Target>>,
}

impl<Target> Default for WideMediaList<Target> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Target> WideMediaList<Target> {
    pub fn new() -> Self {
        Self {
            core: ListCore::new(),
            policy: WideMediaListPaintPolicy::new(
                false,
                super::SelectedRowSurface::ListBackdrop,
                None,
            ),
            configured_geometry: None,
            paint: None,
        }
    }

    pub fn invalidate_paint(&mut self) {
        self.paint = None;
    }

    /// Configure the semantic policy used by the next `view`.
    pub fn set_paint_policy(&mut self, policy: WideMediaListPaintPolicy) {
        self.policy = policy;
        self.invalidate_paint();
    }

    /// Configure the parent-owned claim and row-flow rectangles for the next
    /// `view`. The rectangles may differ in width, as when a framed list
    /// claims its full panel while rows use an inset flow.
    pub fn set_geometry(&mut self, claim_rect: Rect, content_rect: Rect) {
        self.configured_geometry = Some((claim_rect, content_rect));
        self.invalidate_paint();
    }

    pub(crate) fn view_geometry(&self, area: Rect) -> (Rect, Rect) {
        self.configured_geometry.unwrap_or((area, area))
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
    ) where
        Target: Clone,
    {
        self.paint = Some(WidePaintResult {
            claim_rect,
            content_rect,
            row_geometry,
            selected_target: self.core.selected_target().cloned(),
            selected_row_rect,
        });
    }

    /// The current frame's claimed list rectangle, if `view` completed.
    pub fn current_claim_rect(&self) -> Option<Rect> {
        self.paint.as_ref().map(|paint| paint.claim_rect)
    }

    /// The current frame's content rectangle, if `view` completed.
    pub fn current_content_rect(&self) -> Option<Rect> {
        self.paint.as_ref().map(|paint| paint.content_rect)
    }

    /// The current frame's selected target, if `view` completed.
    pub fn current_selected_target(&self) -> Option<&Target> {
        self.paint
            .as_ref()
            .and_then(|paint| paint.selected_target.as_ref())
    }

    /// The current frame's selected-row rectangle, if it is visible.
    pub fn current_selected_row_rect(&self) -> Option<Rect> {
        self.paint
            .as_ref()
            .and_then(|paint| paint.selected_row_rect)
    }

    /// Number of rows in the complete flow retained by the current view.
    pub fn current_flow_len(&self) -> Option<usize> {
        self.paint.as_ref().map(|paint| paint.row_geometry.len())
    }

    /// Display-row offset and target retained by the current view.
    pub fn current_flow_target_at(&self, row: usize) -> Option<Option<&Target>> {
        self.paint
            .as_ref()
            .map(|paint| paint.row_geometry.targets().nth(row).flatten())
    }

    pub fn current_flow_offset(&self) -> Option<usize> {
        self.paint.as_ref().map(|paint| paint.row_geometry.offset())
    }

    /// Whether the current frame's painted list claims `point`.
    pub fn claims_current_point(&self, point: Position) -> bool {
        self.current_claim_rect()
            .is_some_and(|rect| rect.contains(point))
    }

    /// Resolve `point` from the current frame's retained painted geometry.
    pub fn resolve_current_point(&self, point: Position) -> Option<&Target> {
        let paint = self.paint.as_ref()?;
        if !paint.claim_rect.contains(point)
            || point.y < paint.content_rect.y
            || point.y >= paint.content_rect.bottom()
        {
            return None;
        }
        let row = (point.y - paint.content_rect.y) as usize + paint.row_geometry.offset();
        paint
            .row_geometry
            .source_row(row)
            .and_then(|source_row| self.core.rows().get(source_row))
            .and_then(MediaListRow::selectable_target)
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

    /// The clamped viewport for a painted `viewport_height`, keeping the
    /// selected row on screen.
    pub fn resolve_viewport(&self, viewport_height: usize) -> WideViewport {
        self.core.resolve_viewport(viewport_height)
    }

    /// Zero-based screen-row offset from the viewport top to the selected
    /// row, for the responsive [`ViewportAnchor`] hand-off (design.md D3).
    pub fn selected_row_offset(&self, viewport_height: usize) -> Option<usize> {
        self.core.selected_row_offset(viewport_height)
    }
}

impl<Target: Clone> WideMediaList<Target> {
    /// Export the fixed one-column flow used by the painter.
    pub fn row_geometry(&self, viewport_height: usize) -> RowGeometry<Target> {
        let viewport = self.core.resolve_viewport(viewport_height);
        RowGeometry::source(
            self.core.rows(),
            viewport.offset,
            self.core.selected_display_row(),
        )
    }

    /// Resolve a screen `point` inside the painter-supplied `list_area` to the
    /// target under it (design.md D6). Built on the same `row_geometry` the
    /// painter consumes, so the hit flow can never drift from the painted one.
    /// Returns `None` for a point outside `list_area` (horizontally too), a
    /// heading/spacer row, or a point past the last row.
    /// Claim the painted list region, including blank and non-selectable rows.
    /// Parents use this to distinguish an in-region no-op from outside input.
    pub fn claims_point(&self, list_area: Rect, point: Position) -> bool {
        list_area.contains(point)
    }

    pub fn resolve_point(&self, list_area: Rect, point: Position) -> Option<&Target> {
        if !self.claims_point(list_area, point) {
            return None;
        }
        let row_in_view = (point.y - list_area.y) as usize;
        let display_row = row_in_view + self.row_geometry(list_area.height as usize).offset();
        self.core.rows().get(display_row)?.selectable_target()
    }

    /// Resolve a screen row `y` inside the painter-supplied `list_area` to the
    /// selectable-item ordinal painted on that row (design.md D6). Unlike
    /// [`Self::resolve_point`] this returns a **positional** ordinal (safe when
    /// targets are not unique) and keys on the row only, matching the legacy
    /// `left_row_map` row-hit flow the TV workspace feeds to
    /// `NavLevel::set_resting_cursor`. Returns `None` for a `y` outside the
    /// vertical span of `list_area`, a heading/spacer row, or a row past the
    /// last item.
    pub fn resolve_ordinal_at_y(&self, list_area: Rect, y: u16) -> Option<usize> {
        if y < list_area.y || y >= list_area.y.saturating_add(list_area.height) {
            return None;
        }
        let geometry = self.row_geometry(list_area.height as usize);
        let display_row = (y - list_area.y) as usize + geometry.offset();
        let mut ordinal = 0usize;
        for (row, target) in geometry.targets().enumerate() {
            if row == display_row {
                return target.map(|_| ordinal);
            }
            if target.is_some() {
                ordinal += 1;
            }
        }
        None
    }
}

impl<Target: Clone + PartialEq> WideMediaList<Target> {
    /// Replace the display rows, preserving the selected target where possible
    /// and locally clamping otherwise (design.md D3).
    pub fn set_content(&mut self, rows: Vec<MediaListRow<Target>>) {
        if self.core.rows() != rows.as_slice() {
            self.invalidate_paint();
            self.core.set_content(rows);
        }
    }

    /// Replace one existing row by stable target, preserving selection and scroll.
    pub fn patch_row(&mut self, target: &Target, row: MediaListRow<Target>) -> bool {
        self.invalidate_paint();
        self.core.patch_row(target, row)
    }

    /// Replace the display rows from a letter-grouped projection: sort the
    /// `(sort_str, Item)` pairs by natural key and inject `Heading`/`Spacer`
    /// rows per bucket, matching `render_letter_grouped_rows`. `total_count`
    /// selects range vs per-letter buckets; `letter_filter_active` forces
    /// per-letter mode for an already-filtered slice.
    pub fn set_letter_grouped_content(
        &mut self,
        items: Vec<(String, MediaListRow<Target>)>,
        total_count: usize,
        letter_filter_active: bool,
    ) {
        self.invalidate_paint();
        self.core.set_content(letter_grouped_rows(
            items,
            total_count,
            letter_filter_active,
        ));
    }

    /// Move the cursor to `target` when it is present; returns whether it was.
    /// Selection does not change row flow geometry, so a completed view remains
    /// valid for pointer gestures until the next view begins.
    pub fn select_target(&mut self, target: &Target) -> bool {
        self.core.select_target(target)
    }

    /// Produce a [`ViewportAnchor`] from the current selection for a painted
    /// viewport height (design.md D3). `None` when nothing is selectable.
    pub fn viewport_anchor(&self, viewport_height: usize) -> Option<ViewportAnchor<Target>> {
        self.core.viewport_anchor(viewport_height)
    }

    /// Restore a [`ViewportAnchor`] at a painted viewport height: select the
    /// target if present, then place it at the requested offset where the
    /// geometry allows, clamping otherwise (design.md D3).
    pub fn apply_viewport_anchor(
        &mut self,
        anchor: &ViewportAnchor<Target>,
        viewport_height: usize,
    ) {
        self.invalidate_paint();
        self.core.apply_viewport_anchor(anchor, viewport_height);
    }
}

impl<Target: Clone> Component for WideMediaList<Target> {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        crate::app::render::render_wide_media_list_component(frame, area, self, self.policy);
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
