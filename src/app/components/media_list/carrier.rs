//! Destination-side carrier for one logical media-row flow.
//!
//! The carrier keeps one canonical fixed-row presentation over one
//! [`MediaList`] owner.  The Library panel changes only the rectangles and
//! paint policy as geometry changes; it never swaps list presentations.

use ratatui::layout::{Position, Rect};
use tuirealm::event::{Key, KeyEvent, KeyModifiers};

use super::{
    MediaListOperation, MediaListRow, MediaListTransition, SelectionOrigin, SelectionSummary,
    ViewportAnchor, WideMediaList,
};

/// The retained presentation identity for a carrier.  There is deliberately
/// only one presentation: fixed rows are used in every panel geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presentation {
    Wide,
}

/// One logical row flow's destination-side carrier over one canonical owner.
pub struct MediaListCarrier<Target> {
    wide: WideMediaList<Target>,
    selection_origin: SelectionOrigin,
}

impl<Target> MediaListCarrier<Target> {
    pub fn new(_presentation: Presentation) -> Self {
        Self::new_with_origin(_presentation, SelectionOrigin::Queue)
    }

    pub fn new_with_origin(_presentation: Presentation, selection_origin: SelectionOrigin) -> Self {
        Self {
            wide: WideMediaList::new(),
            selection_origin,
        }
    }

    pub fn active(&self) -> Presentation {
        Presentation::Wide
    }

    pub fn wide(&self) -> &WideMediaList<Target> {
        &self.wide
    }

    pub fn wide_mut(&mut self) -> &mut WideMediaList<Target> {
        &mut self.wide
    }

    pub fn selected_target(&self) -> Option<&Target> {
        self.wide.selected_target()
    }

    pub fn multi_selection(&self) -> &[Target] {
        self.wide.multi_selection()
    }

    pub fn set_selection_origin(&mut self, origin: SelectionOrigin) {
        self.selection_origin = origin;
    }

    pub fn selection_summary(&self) -> SelectionSummary {
        SelectionSummary {
            count: self.multi_selection().len(),
            origin: self.selection_origin.clone(),
        }
    }

    pub fn is_visual_mode(&self) -> bool {
        !self.multi_selection().is_empty()
    }

    pub fn cursor(&self) -> usize {
        self.wide.cursor()
    }

    pub fn scroll(&self) -> usize {
        self.wide.scroll()
    }

    pub fn rows(&self) -> &[MediaListRow<Target>] {
        self.wide.rows()
    }

    pub fn current_content_rect(&self) -> Option<Rect> {
        self.wide.current_content_rect()
    }

    pub fn current_selected_row_rect(&self) -> Option<Rect> {
        self.wide.current_selected_row_rect()
    }

    pub fn current_flow_len(&self) -> Option<usize> {
        self.wide.current_flow_len()
    }

    pub fn current_flow_target_at(&self, row: usize) -> Option<Option<&Target>> {
        self.wide.current_flow_target_at(row)
    }

    pub fn current_flow_offset(&self) -> Option<usize> {
        self.wide.current_flow_offset()
    }

    pub fn invalidate_paint(&mut self) {
        self.wide.invalidate_paint();
    }

    pub fn is_empty(&self) -> bool {
        self.wide.is_empty()
    }
}

impl<Target: Clone + PartialEq> MediaListCarrier<Target> {
    /// Keep the canonical fixed-row presentation active and clamp its
    /// viewport to the current geometry without transferring owner state.
    pub fn set_presentation(&mut self, _presentation: Presentation, viewport_height: usize) {
        let offset = self.wide.resolve_viewport(viewport_height.max(1)).offset;
        if self.wide.scroll() != offset {
            self.wide.set_scroll(offset);
        }
    }

    pub fn set_content(&mut self, rows: Vec<MediaListRow<Target>>) {
        self.wide.set_content(rows);
    }

    pub fn select_target(&mut self, target: &Target) -> bool {
        self.wide.select_target(target)
    }

    pub fn enter_visual_mode(&mut self) {
        self.wide.enter_visual_mode();
    }

    pub fn handle_visual_key(&mut self, key: &KeyEvent) -> Option<usize> {
        if matches!(key.code, Key::Char('v') | Key::Char('V'))
            && key.modifiers == KeyModifiers::SHIFT
        {
            self.enter_visual_mode();
            return Some(self.multi_selection().len());
        }
        if !self.is_visual_mode() || !key.modifiers.is_empty() {
            return None;
        }
        match key.code {
            Key::Esc => {
                self.clear_selection();
                Some(0)
            }
            Key::Char(' ') => {
                let target = self.selected_target()?.clone();
                self.toggle_selection(&target);
                Some(self.multi_selection().len())
            }
            _ => None,
        }
    }

    pub fn toggle_selection(&mut self, target: &Target) {
        self.wide.toggle_selection(target);
    }

    pub fn extend_selection_to(&mut self, target: &Target) {
        self.wide.extend_selection_to(target);
    }

    pub fn clear_selection(&mut self) {
        self.wide.clear_selection();
    }

    pub fn patch_row(&mut self, target: &Target, row: MediaListRow<Target>) -> bool {
        self.wide.patch_row(target, row)
    }

    pub fn select_first(&mut self) {
        self.wide.select_first();
    }

    pub fn select_last(&mut self) {
        self.wide.select_last();
    }

    pub fn select_index(&mut self, index: usize) {
        self.wide.select_index(index);
    }

    pub fn move_selection(&mut self, delta: i64) {
        self.wide.move_selection(delta);
    }

    pub fn set_scroll(&mut self, offset: usize) {
        self.wide.set_scroll(offset);
    }

    pub fn sync_viewport(&mut self, viewport_height: usize) {
        let offset = self.wide.resolve_viewport(viewport_height.max(1)).offset;
        self.wide.set_scroll(offset);
    }

    pub fn delegate_operation(
        &mut self,
        operation: MediaListOperation<Target>,
    ) -> MediaListTransition<Target> {
        let mut transition = self.wide.delegate_operation(operation);
        if transition.selection_summary.is_some() {
            transition.selection_summary = Some(self.selection_summary());
        }
        transition
    }

    pub fn claims_current_point(&self, point: Position) -> bool {
        self.wide.claims_current_point(point)
    }

    pub fn resolve_current_point(&self, point: Position) -> Option<&Target> {
        self.wide.resolve_current_point(point)
    }

    pub fn viewport_anchor(&self, viewport_height: usize) -> Option<ViewportAnchor<Target>> {
        self.wide.viewport_anchor(viewport_height)
    }

    pub fn apply_viewport_anchor(
        &mut self,
        anchor: &ViewportAnchor<Target>,
        viewport_height: usize,
    ) {
        self.wide.apply_viewport_anchor(anchor, viewport_height);
    }
}
