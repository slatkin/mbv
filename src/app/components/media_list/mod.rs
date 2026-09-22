//! Provider-neutral embedded media-list controls (design.md D1/D2/D3).
//!
//! [`WideMediaList`] is the fixed-row presentation over one [`MediaList`]
//! owner. Painting lives in `crate::app::render::components::media_list`.

use crate::app::components::list::{
    Cursored, MarkSelection, MarkSelectionState, Row, RowFlow, Viewported,
};
use std::time::Instant;

/// The flat shape's fixed selectable-row page distance (design D4). Flat
/// lists keep a fixed row count per page; the tree owns its own policy.
const PAGE_DISTANCE: usize = 5;

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
pub use types::{
    ActiveProgress, LibrarySelectionOrigin, MediaKind, MediaListDisposition, MediaListOperation,
    MediaListRow, MediaListSurfaceInput, MediaListTitleReveal, MediaListTrailing,
    MediaListTransition, MediaSemanticState, RowGeometry, RowIntent, SelectedRowSurface,
    SelectionOrigin, SelectionSummary, WideMediaListPaintPolicy, WideViewport, ZebraStripe,
};
pub use wide::WideMediaList;
pub(crate) use wide::{
    queue_row_background, queue_row_zebra, queue_row_zebra_stripe, row_marquee_key,
};

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
    multi_selection: MarkSelectionState<Target>,
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
            multi_selection: MarkSelectionState::new(),
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
        self.multi_selection.targets()
    }

    /// A non-empty multi-selection is Visual mode.
    pub fn is_visual_mode(&self) -> bool {
        !self.multi_selection.is_empty()
    }

    /// Move the cursor by `delta` selectable rows, clamped to the ends.
    fn move_selection(&mut self, delta: i64)
    where
        Target: Clone + Eq,
    {
        let flow = self.row_flow();
        Cursored::move_by(self, &flow, delta as isize);
    }

    fn select_first(&mut self)
    where
        Target: Clone + Eq,
    {
        let flow = self.row_flow();
        Cursored::first(self, &flow);
    }

    fn select_last(&mut self)
    where
        Target: Clone + Eq,
    {
        let flow = self.row_flow();
        Cursored::last(self, &flow);
    }

    /// Place the cursor at selectable index `index`, clamped to the last row.
    fn select_index(&mut self, index: usize)
    where
        Target: Clone + Eq,
    {
        let flow = self.row_flow();
        Cursored::select_selectable_ordinal(self, &flow, index);
    }

    /// The display rows as the seam's ordered row flow.
    pub(crate) fn row_flow(&self) -> RowFlow<Target>
    where
        Target: Clone,
    {
        RowFlow::new(
            self.rows
                .iter()
                .map(|row| {
                    row.selectable_target()
                        .cloned()
                        .map_or_else(Row::structural, Row::selectable)
                })
                .collect(),
        )
    }

    /// The clamped viewport for a painted `viewport_height`, keeping the
    /// selected row on screen.
    fn resolve_viewport(&self, viewport_height: usize) -> WideViewport
    where
        Target: Clone + Eq,
    {
        let flow = self.row_flow();
        let offset = Viewported::resolved_viewport_offset(
            self,
            &flow,
            viewport_height,
            self.selected_display_row(),
        );
        WideViewport {
            offset,
            height: viewport_height.max(1),
            total_rows: self.rows.len(),
        }
    }

    /// Zero-based screen-row offset from the viewport top to the selected
    /// row (design.md D3). `None` when nothing is selectable.
    #[cfg_attr(not(test), allow(dead_code))]
    fn selected_row_offset(&self, viewport_height: usize) -> Option<usize>
    where
        Target: Clone + Eq,
    {
        let row = self.selected_display_row()?;
        Some(row.saturating_sub(self.resolve_viewport(viewport_height).offset))
    }

    /// Apply a target-resolved operation and report all independent effects.
    pub fn delegate_operation(
        &mut self,
        operation: MediaListOperation<Target>,
    ) -> MediaListTransition<Target>
    where
        Target: Clone + Eq,
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
                self.move_selection(delta.saturating_mul(PAGE_DISTANCE as i64));
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

impl<Target: Clone + Eq> MediaList<Target> {
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

impl<Target: Clone + Eq> MediaList<Target> {
    fn context_intent(&mut self, target: Target) -> RowIntent<Target> {
        if self.is_visual_mode() {
            if self
                .multi_selection
                .targets()
                .iter()
                .any(|selected| selected == &target)
            {
                let flow = self.row_flow();
                let targets = MarkSelection::action_targets(self, &flow);
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
                self.multi_selection.clear();
                self.multi_selection.add(target.clone());
                self.frozen_selection.clear();
            } else {
                self.frozen_selection = self.multi_selection.targets().to_vec();
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
    /// still present; a vanished target resolves through the shared cursor seam
    /// to the first selectable row, never from a carried-over numeric position
    /// (`shared-list-components`: "Content replacement preserves selection by
    /// stable identity"). Structural rows are filtered out of the selectable
    /// index here so they can never become selected.
    fn set_content(&mut self, rows: Vec<MediaListRow<Target>>) {
        let previous = self.selected_target().cloned();
        self.rows = rows;
        self.selectable = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.selectable_target().is_some())
            .map(|(index, _)| index)
            .collect();
        if let Some(target) = previous {
            let flow = self.row_flow();
            Cursored::select_target(self, &flow, &target);
        } else {
            self.cursor = self.cursor.min(self.selectable.len().saturating_sub(1));
        }
        if self.selectable.is_empty() {
            self.cursor = 0;
        }
        let present: Vec<Target> = self
            .selectable
            .iter()
            .filter_map(|&row| self.rows[row].selectable_target().cloned())
            .collect();
        self.multi_selection
            .retain(|target| present.contains(target));
        self.frozen_selection
            .retain(|target| present.contains(target));
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
        let flow = self.row_flow();
        Viewported::clamp_viewport(self, &flow, 1);
    }
}

impl<Target: Clone + Eq> Cursored<Target> for MediaList<Target> {
    fn selected_target(&self) -> Option<&Target> {
        self.selected_target()
    }

    fn set_selected_target(&mut self, target: Option<&Target>) {
        if let Some(target) = target {
            self.select_target(target);
        }
    }
}

impl<Target: Clone + Eq> Viewported<Target> for MediaList<Target> {
    fn viewport_offset(&self) -> usize {
        self.scroll()
    }

    fn set_viewport_offset(&mut self, offset: usize) {
        self.set_scroll(offset);
    }

    /// A grouped flat list keeps the selection's group heading or spacer
    /// visible when it scrolls the selection back into view from above (#731).
    fn raise_over_leading_structural_rows(&self) -> bool {
        true
    }
}

impl<Target: Clone + Eq> MarkSelection<Target> for MediaList<Target> {
    fn mark_selection(&self) -> &MarkSelectionState<Target> {
        &self.multi_selection
    }

    fn mark_selection_mut(&mut self) -> &mut MarkSelectionState<Target> {
        &mut self.multi_selection
    }

    fn after_mark_mutation(&mut self) {
        self.frozen_selection.clear();
        self.selection_anchor = None;
        self.live_range = false;
    }
}
