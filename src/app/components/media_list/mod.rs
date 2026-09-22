//! Provider-neutral embedded media-list controls (design.md D1/D2/D3).
//!
//! [`WideMediaList`] is the fixed-row presentation over one [`MediaList`]
//! owner. Painting lives in `crate::app::render::components::media_list`.

use crate::app::ui_util::move_cursor;
use ratatui::layout::Rect;
use std::time::Instant;

mod anchor;
mod carrier;
mod grouping;
mod selection;
#[cfg(test)]
mod tests;
mod types;
mod wide;

pub use anchor::ViewportAnchor;
pub use carrier::MediaListCarrier;
pub use grouping::letter_grouped_rows;
pub(crate) use types::row_marquee_key;
pub(crate) use types::{queue_row_background, queue_row_zebra, queue_row_zebra_stripe};
pub use types::{
    ActiveProgress, LibrarySelectionOrigin, MediaKind, MediaListDisposition, MediaListOperation,
    MediaListRow, MediaListSurfaceInput, MediaListTitleReveal, MediaListTrailing,
    MediaListTransition, MediaSemanticState, RowIntent, SelectedRowSurface, SelectionOrigin,
    SelectionSummary, WideMediaListPaintPolicy, WideViewport, ZebraStripe,
};
pub use wide::WideMediaList;

/// Flow-space geometry for a painted media-list control.
///
/// Rows contain the source-row lookup used by painters and an optional stable
/// target for hit maps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowGeometry<Target> {
    offset: usize,
    rows: Vec<FlowRow<Target>>,
    selected_row: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FlowRow<Target> {
    source_row: Option<usize>,
    target: Option<Target>,
}

impl<Target> RowGeometry<Target> {
    /// Display-row index at the viewport top.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Number of rows in the complete painted flow.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Display-row index of the selected row in flow space.
    pub fn selected_row(&self) -> Option<usize> {
        self.selected_row
    }

    /// The selected row's absolute one-line rectangle when it is visible.
    pub fn selected_row_rect(&self, area: Rect) -> Option<Rect> {
        let row = self.selected_row?;
        (self.offset..self.offset.saturating_add(area.height as usize))
            .contains(&row)
            .then(|| Rect {
                y: area.y + (row - self.offset) as u16,
                height: 1,
                ..area
            })
    }

    /// Resolve a flow row to its source row for canonical painting.
    pub(crate) fn source_row(&self, row: usize) -> Option<usize> {
        self.rows.get(row).and_then(|row| row.source_row)
    }
}

impl<Target: Clone> RowGeometry<Target> {
    fn source(rows: &[MediaListRow<Target>], offset: usize, selected_row: Option<usize>) -> Self {
        Self {
            offset,
            rows: rows
                .iter()
                .enumerate()
                .map(|(source_row, row)| FlowRow {
                    source_row: Some(source_row),
                    target: row.selectable_target().cloned(),
                })
                .collect(),
            selected_row,
        }
    }
}

/// The single canonical owner for one logical provider-neutral media-row flow.
///
/// Presentations embed or receive this owner; they never maintain a second
/// cursor, scroll, selectable index, or selected-target state.
pub struct MediaList<Target> {
    /// Every display row in paint order.
    rows: Vec<MediaListRow<Target>>,
    /// Indices into `rows` that are selectable `Item`s, ascending. Rebuilt
    /// only by `set_content`.
    selectable: Vec<usize>,
    /// Index into `selectable`; `0` when nothing is selectable.
    cursor: usize,
    /// Display-row index parked at the viewport top. Height-aware clamping
    /// happens in `resolve_viewport` at paint time.
    scroll: usize,
    /// Stable targets selected for a bulk action, in selection order.
    multi_selection: Vec<Target>,
    /// The stable target from which range selection is extended.
    selection_anchor: Option<Target>,
    /// Selection retained when a live range is re-anchored or extended.
    frozen_selection: Vec<Target>,
    /// Whether cursor movement currently recomputes the anchored range.
    live_range: bool,
    /// The list's title-reveal policy (see [`MediaListTitleReveal`]).
    title_reveal: MediaListTitleReveal,
    marquee_text: String,
    marquee_started_at: Instant,
}

impl<Target> MediaList<Target> {
    /// Creates an empty canonical owner for one logical row flow.
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            selectable: Vec::new(),
            cursor: 0,
            scroll: 0,
            multi_selection: Vec::new(),
            selection_anchor: None,
            frozen_selection: Vec::new(),
            live_range: false,
            title_reveal: MediaListTitleReveal::Always,
            marquee_text: String::new(),
            marquee_started_at: Instant::now(),
        }
    }

    /// Declare how this list's rows reveal their titles.
    pub(crate) fn set_title_reveal(&mut self, policy: MediaListTitleReveal) {
        self.title_reveal = policy;
    }

    pub(crate) fn title_reveal(&self) -> MediaListTitleReveal {
        self.title_reveal
    }

    pub(crate) fn marquee_state(&mut self, text: &str) -> (String, Instant) {
        if self.marquee_text != text {
            self.marquee_text.clear();
            self.marquee_text.push_str(text);
            self.marquee_started_at = Instant::now();
        }
        (self.marquee_text.clone(), self.marquee_started_at)
    }

    /// Test-only clock injection: advances the marquee clock without a real
    /// sleep. `text` must match the currently marqueed title so the injected
    /// time isn't immediately reset by the next `marquee_state` call.
    #[cfg(test)]
    pub(crate) fn set_marquee_started_at(&mut self, text: &str, at: Instant) {
        self.marquee_text.clear();
        self.marquee_text.push_str(text);
        self.marquee_started_at = at;
    }

    fn rows(&self) -> &[MediaListRow<Target>] {
        &self.rows
    }

    /// No selectable rows at all.
    fn is_empty(&self) -> bool {
        self.selectable.is_empty()
    }

    /// The cursor as an index into the selectable rows.
    fn cursor(&self) -> usize {
        self.cursor
    }

    /// The display-row index the cursor currently points at.
    pub(crate) fn selected_display_row(&self) -> Option<usize> {
        self.selectable.get(self.cursor).copied()
    }

    /// The stable identity under the cursor.
    fn selected_target(&self) -> Option<&Target> {
        self.selectable
            .get(self.cursor)
            .and_then(|&row| self.rows[row].selectable_target())
    }

    /// The resting scroll offset (pre height-aware clamp).
    fn scroll(&self) -> usize {
        self.scroll
    }

    /// Store the offset a painter resolved, so the next frame resumes from it.
    fn set_scroll(&mut self, offset: usize) {
        self.scroll = offset.min(self.rows.len().saturating_sub(1));
    }

    /// Stable targets currently selected for a bulk action.
    pub fn multi_selection(&self) -> &[Target] {
        &self.multi_selection
    }

    /// A non-empty multi-selection is Visual mode.
    pub fn is_visual_mode(&self) -> bool {
        !self.multi_selection.is_empty()
    }

    /// Move the cursor by `delta` selectable rows, clamped to the ends.
    fn move_selection(&mut self, delta: i64) {
        if !self.selectable.is_empty() {
            self.cursor = move_cursor(self.cursor, delta, self.selectable.len());
        }
    }

    fn select_first(&mut self) {
        self.cursor = 0;
    }

    fn select_last(&mut self) {
        self.cursor = self.selectable.len().saturating_sub(1);
    }

    /// Place the cursor at selectable index `index`, clamped to the last row.
    fn select_index(&mut self, index: usize) {
        self.cursor = index.min(self.selectable.len().saturating_sub(1));
    }

    /// The clamped viewport for a painted `viewport_height`, keeping the
    /// selected row on screen.
    fn resolve_viewport(&self, viewport_height: usize) -> WideViewport {
        let total_rows = self.rows.len();
        let height = viewport_height.max(1);
        let mut offset = self.scroll.min(total_rows.saturating_sub(height));
        if let Some(row) = self.selected_display_row() {
            if row < offset {
                offset = row;
                // Keep the label of the selection's group visible: the raise
                // continues over the contiguous Heading/Spacer rows directly
                // above it and stops at the previous selectable row (#731).
                while offset > 0 && self.rows[offset - 1].selectable_target().is_none() {
                    offset -= 1;
                }
            } else if row >= offset + height {
                offset = row + 1 - height;
            }
        }
        WideViewport {
            offset,
            height,
            total_rows,
        }
    }

    /// Zero-based screen-row offset from the viewport top to the selected
    /// row (design.md D3). `None` when nothing is selectable.
    #[cfg_attr(not(test), allow(dead_code))]
    fn selected_row_offset(&self, viewport_height: usize) -> Option<usize> {
        let row = self.selected_display_row()?;
        Some(row.saturating_sub(self.resolve_viewport(viewport_height).offset))
    }

    /// Apply a target-resolved operation and report all independent effects.
    pub fn delegate_operation(
        &mut self,
        operation: MediaListOperation<Target>,
    ) -> MediaListTransition<Target>
    where
        Target: Clone + PartialEq,
    {
        let before = self.selected_target().cloned();
        let before_count = self.multi_selection.len();
        let extends_range = matches!(
            operation,
            MediaListOperation::Move(_)
                | MediaListOperation::Page(_)
                | MediaListOperation::First
                | MediaListOperation::Last
        );
        let external_intent = match operation {
            MediaListOperation::Move(delta) => {
                self.move_selection(delta);
                None
            }
            MediaListOperation::Page(delta) => {
                self.move_selection(delta.saturating_mul(5));
                None
            }
            MediaListOperation::First => {
                self.select_first();
                None
            }
            MediaListOperation::Last => {
                self.select_last();
                None
            }
            MediaListOperation::ActivateCurrent => {
                self.selected_target().cloned().map(RowIntent::Activate)
            }
            MediaListOperation::ContextCurrent => self
                .selected_target()
                .cloned()
                .map(|target| self.context_intent(target)),
            MediaListOperation::Select(target) => {
                self.clear_selection();
                self.select_target(&target);
                None
            }
            MediaListOperation::Toggle(target) => {
                self.toggle_selection(&target);
                self.select_target(&target);
                None
            }
            MediaListOperation::Range(target) => {
                self.extend_selection_to(&target);
                self.select_target(&target);
                None
            }
            MediaListOperation::Activate(target) => {
                self.select_target(&target);
                Some(RowIntent::Activate(target))
            }
            MediaListOperation::Context(target) => Some(self.context_intent(target)),
        };
        if extends_range && self.live_range {
            if let Some(target) = self.selected_target().cloned() {
                self.extend_selection_to(&target);
            }
        }
        let after = self.selected_target().cloned();
        let disposition = if before.is_some()
            || after.is_some()
            || external_intent.is_some()
            || before_count != self.multi_selection.len()
        {
            MediaListDisposition::Consumed
        } else {
            MediaListDisposition::Unhandled
        };
        MediaListTransition {
            disposition,
            selected_target: (before != after).then_some(after).flatten(),
            selection_summary: (before_count != self.multi_selection.len()).then_some(
                SelectionSummary {
                    count: self.multi_selection.len(),
                    origin: SelectionOrigin::Queue,
                },
            ),
            external_intent,
        }
    }
}

impl<Target: PartialEq> MediaList<Target> {
    /// The selectable-index position of `target`, if it is present.
    fn position_of(&self, target: &Target) -> Option<usize> {
        self.selectable
            .iter()
            .position(|&row| self.rows[row].selectable_target() == Some(target))
    }

    /// Move the cursor to `target` when it is present; returns whether it was.
    fn select_target(&mut self, target: &Target) -> bool {
        match self.position_of(target) {
            Some(index) => {
                self.cursor = index;
                true
            }
            None => false,
        }
    }
}

impl<Target: Clone + PartialEq> MediaList<Target> {
    fn context_intent(&mut self, target: Target) -> RowIntent<Target> {
        if self.is_visual_mode() {
            if self
                .multi_selection
                .iter()
                .any(|selected| selected == &target)
            {
                let targets = self
                    .selectable
                    .iter()
                    .filter_map(|&row| self.rows[row].selectable_target())
                    .filter(|candidate| {
                        self.multi_selection
                            .iter()
                            .any(|selected| selected == *candidate)
                    })
                    .cloned()
                    .collect();
                RowIntent::ContextSelection(targets)
            } else {
                self.clear_selection();
                RowIntent::Context(target)
            }
        } else {
            RowIntent::Context(target)
        }
    }

    /// Begin keyboard Visual mode, anchored at the current cursor target.
    pub fn enter_visual_mode(&mut self) {
        if let Some(target) = self.selected_target().cloned() {
            if self.multi_selection.is_empty() {
                self.multi_selection = vec![target.clone()];
                self.frozen_selection.clear();
            } else {
                self.frozen_selection = self.multi_selection.clone();
            }
            self.selection_anchor = Some(target);
            self.live_range = true;
        }
    }

    /// Replace one existing row by stable target without rebuilding indexes or
    /// disturbing selection/scroll. This is for live presentation patches.
    fn patch_row(&mut self, target: &Target, row: MediaListRow<Target>) -> bool {
        let Some(index) = self
            .rows
            .iter()
            .position(|existing| existing.selectable_target() == Some(target))
        else {
            return false;
        };
        self.rows[index] = row;
        true
    }

    /// Replace the display rows. The selected target is preserved when it is
    /// still present; otherwise the cursor and scroll are locally clamped
    /// (design.md D3). Structural rows are filtered out of the selectable
    /// index here so they can never become selected.
    fn set_content(&mut self, rows: Vec<MediaListRow<Target>>) {
        let previous = self.selected_target().cloned();
        let selectable: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.selectable_target().is_some())
            .map(|(index, _)| index)
            .collect();
        let cursor = previous
            .and_then(|target| {
                selectable
                    .iter()
                    .position(|&row| rows[row].selectable_target() == Some(&target))
            })
            .unwrap_or_else(|| self.cursor.min(selectable.len().saturating_sub(1)));
        self.rows = rows;
        self.selectable = selectable;
        let present: Vec<Target> = self
            .selectable
            .iter()
            .filter_map(|&row| self.rows[row].selectable_target().cloned())
            .collect();
        self.multi_selection
            .retain(|target| present.contains(target));
        self.frozen_selection
            .retain(|target| present.contains(target));
        self.cursor = if self.selectable.is_empty() {
            0
        } else {
            cursor.min(self.selectable.len() - 1)
        };
        if self
            .selection_anchor
            .as_ref()
            .is_some_and(|target| !present.contains(target))
        {
            self.selection_anchor = self.selected_target().cloned();
        }
        if self.multi_selection.is_empty() {
            self.frozen_selection.clear();
            self.selection_anchor = None;
            self.live_range = false;
        }
        self.scroll = self.scroll.min(self.rows.len().saturating_sub(1));
    }
}
