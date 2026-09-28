use super::{
    MediaList, MediaListOperation, MediaListTransition, WideMediaListPaintPolicy, WideViewport,
};
use crate::list::{
    Cursored, MarkSelection, MarkSelectionState, PaintRetained, PaintRetainedState, Viewported,
};
use mbv_render::components::media_list::{MediaListRow, MediaListTitleReveal, RowGeometry};
use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::Component;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

/// Embedded plain fixed-height, one-column media list: owns the display-row
/// list, the selectable index over it, the cursor, and the resting scroll
/// offset through its shared [`MediaList`] owner. It has no mouse hit-resolution API
/// and accepts no column-count or inline-detail options (design.md D1).
/// Painting is performed by its `Component::view` through the render adapter;
/// current-frame point resolution and geometry are retained by the shared
/// paint carrier.
#[derive(Debug)]
pub struct WideMediaList<Target> {
    core: MediaList<Target>,
    policy: WideMediaListPaintPolicy,
    configured_geometry: Option<(Rect, Rect)>,
    paint: PaintRetainedState<Target>,
}

impl<Target> Default for WideMediaList<Target> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Target> WideMediaList<Target> {
    #[must_use]
    pub fn new() -> Self {
        Self::from_media_list(MediaList::new())
    }

    /// Reconfigure this logical flow as a Wide presentation without copying
    /// its rows or interaction state.
    pub fn from_media_list(core: MediaList<Target>) -> Self {
        Self {
            core,
            policy: WideMediaListPaintPolicy::new(false),
            configured_geometry: None,
            paint: PaintRetainedState::new(),
        }
    }

    pub fn invalidate_paint(&mut self) {
        self.paint.invalidate();
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

    pub fn view_geometry(&self, area: Rect) -> (Rect, Rect) {
        self.configured_geometry.unwrap_or((area, area))
    }

    pub fn begin_view(&mut self) {
        self.invalidate_paint();
    }

    pub fn finish_view(
        &mut self,
        claim_rect: Rect,
        content_rect: Rect,
        row_geometry: &RowGeometry<Target>,
        selected_row_rect: Option<&Rect>,
    ) where
        Target: Clone,
    {
        let rows = row_geometry.target_rects(claim_rect, content_rect);
        PaintRetained::finish(
            self,
            claim_rect,
            content_rect,
            rows,
            selected_row_rect.copied(),
        );
    }

    /// The current frame's content rectangle, if `view` completed.
    pub fn current_content_rect(&self) -> Option<Rect> {
        self.paint.content_rect()
    }

    /// The current frame's selected-row rectangle, if it is visible.
    pub fn current_selected_row_rect(&self) -> Option<Rect> {
        self.paint.selected_row_rect()
    }

    /// Whether the current frame's painted list claims `point`.
    pub fn claims_current_point(&self, point: Position) -> bool {
        self.paint.claims_point(point)
    }

    /// Resolve `point` from the current frame's retained painted geometry.
    pub fn resolve_current_point(&self, point: Position) -> Option<&Target> {
        self.paint.resolve_point(point)
    }

    pub fn rows(&self) -> &[MediaListRow<Target>] {
        self.core.rows()
    }

    /// No selectable rows at all.
    pub fn is_empty(&self) -> bool {
        self.core.is_empty()
    }

    /// The cursor as an index into the selectable rows.
    pub fn cursor(&self) -> usize {
        self.core.cursor()
    }

    /// The stable identity under the cursor.
    pub fn selected_target(&self) -> Option<&Target> {
        self.core.selected_target()
    }

    pub fn multi_selection(&self) -> &[Target] {
        self.core.multi_selection()
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

    pub fn marquee_state(&mut self, text: &str) -> (String, std::time::Instant) {
        self.core.marquee_state(text)
    }

    #[cfg(any(test, feature = "test"))]
    pub fn title_reveal(&self) -> MediaListTitleReveal {
        self.core.title_reveal
    }

    pub fn set_title_reveal(&mut self, policy: MediaListTitleReveal) {
        self.core.set_title_reveal(policy);
    }
}

impl<Target: Clone + Eq> WideMediaList<Target> {
    /// The clamped viewport for a painted `viewport_height`, keeping the
    /// selected row on screen.
    pub fn resolve_viewport(&self, viewport_height: usize) -> WideViewport {
        self.core.resolve_viewport(viewport_height)
    }

    /// Export the fixed one-column flow used by the painter.
    pub fn row_geometry(&self, viewport_height: usize) -> RowGeometry<Target> {
        let viewport = self.core.resolve_viewport(viewport_height);
        RowGeometry::source(
            self.core.rows(),
            viewport.offset,
            self.core.selected_display_row(),
        )
    }
}

impl<Target: Clone + Eq> WideMediaList<Target> {
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
}

impl<Target: Clone + Eq> WideMediaList<Target> {
    pub fn enter_visual_mode(&mut self) {
        self.invalidate_paint();
        self.core.enter_visual_mode();
    }
    /// Replace the display rows, preserving the selected target where possible
    /// and locally clamping otherwise (design.md D3).
    pub fn set_content(&mut self, rows: Vec<MediaListRow<Target>>) {
        self.invalidate_paint();
        self.core.set_content(rows);
    }

    /// Replace one existing row by stable target, preserving selection and scroll.
    pub fn patch_row(&mut self, target: &Target, row: MediaListRow<Target>) -> bool {
        self.invalidate_paint();
        self.core.patch_row(target, row)
    }

    /// Replace the display rows from a letter-grouped projection: sort the
    /// Move the cursor to `target` when it is present; returns whether it was.
    /// Selection does not change row flow geometry, so a completed view remains
    /// valid for pointer gestures until the next view begins.
    pub fn select_target(&mut self, target: &Target) -> bool {
        self.core.select_target(target)
    }

    pub fn toggle_selection(&mut self, target: &Target) {
        self.core.toggle_selection(target);
    }

    pub fn select_all(&mut self) {
        self.invalidate_paint();
        self.core.select_all();
    }

    pub fn clear_selection(&mut self) {
        self.core.clear_selection();
    }

    /// Offer one already-normalized row-local input to the shared owner. Every
    /// delegate outcome is selection-only and changes no row-flow geometry, so
    /// the completed frame's retained facts stay valid for a continuing pointer
    /// gesture (matching `select_target`); the next `view` re-publishes them.
    pub fn delegate_operation(
        &mut self,
        operation: MediaListOperation<Target>,
    ) -> MediaListTransition<Target> {
        self.core.delegate_operation(operation)
    }
}

impl<Target: Clone + Eq> Cursored<Target> for WideMediaList<Target> {
    fn selected_target(&self) -> Option<&Target> {
        self.core.selected_target()
    }

    fn set_selected_target(&mut self, target: Option<&Target>) {
        Cursored::set_selected_target(&mut self.core, target);
    }
}

impl<Target: Clone + Eq> Viewported<Target> for WideMediaList<Target> {
    fn viewport_offset(&self) -> usize {
        self.core.scroll()
    }

    fn set_viewport_offset(&mut self, offset: usize) {
        self.set_scroll(offset);
    }
}

impl<Target: Clone + Eq> MarkSelection<Target> for WideMediaList<Target> {
    fn mark_selection(&self) -> &MarkSelectionState<Target> {
        self.core.mark_selection()
    }

    fn mark_selection_mut(&mut self) -> &mut MarkSelectionState<Target> {
        self.core.mark_selection_mut()
    }

    fn after_mark_mutation(&mut self) {
        self.core.after_mark_mutation();
    }
}

impl<Target> PaintRetained<Target> for WideMediaList<Target> {
    fn paint_retained(&self) -> &PaintRetainedState<Target> {
        &self.paint
    }

    fn paint_retained_mut(&mut self) -> &mut PaintRetainedState<Target> {
        &mut self.paint
    }
}

impl<Target: Clone + Eq> Component for WideMediaList<Target> {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        self.begin_view();
        let (claim_rect, content_rect) = self.view_geometry(area);
        if area.is_empty() || claim_rect.is_empty() || content_rect.is_empty() || self.is_empty() {
            return;
        }
        let policy = self.policy;
        let geometry = self.row_geometry(content_rect.height as usize);
        let key = policy
            .focused()
            .then_some(geometry.selected_row())
            .flatten()
            .and_then(|row| self.rows().get(row))
            .and_then(mbv_render::components::media_list::row_marquee_key);
        let marquee_key = key;
        if let Some(key) = marquee_key.as_deref() {
            self.marquee_state(key);
        }
        let crate::media_list::MediaList {
            rows: list_rows,
            multi_selection,
            title_reveal,
            marquee_text,
            marquee_started_at,
            ..
        } = &mut self.core;
        let rows = &**list_rows;
        let multi_selection = multi_selection.targets();
        let title_reveal = *title_reveal;
        let marquee_state = marquee_key
            .as_ref()
            .map(|_| (&mut *marquee_text, &mut *marquee_started_at));
        let marquee = marquee_key
            .as_deref()
            .zip(marquee_state)
            .map(|(key, (text, started_at))| (key, text, started_at));
        let paint = mbv_render::render_wide_media_list_component(
            frame,
            mbv_render::components::media_list::WideMediaListPaintInput {
                rows,
                row_geometry: geometry,
                multi_selection,
                title_reveal,
                marquee,
                policy,
                claim_rect,
                content_rect,
            },
        );
        self.set_scroll(paint.row_geometry.offset());
        self.finish_view(
            claim_rect,
            content_rect,
            &paint.row_geometry,
            paint.selected_row_rect.as_ref(),
        );
    }

    fn query(&self, _attr: Attribute) -> Option<QueryResult<'_>> {
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
