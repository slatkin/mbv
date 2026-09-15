//! Provider-neutral embedded media-list controls (design.md D1/D2/D3).
//!
//! [`WideMediaList`] and [`InlineMediaBrowser`] are closed presentations over
//! one [`MediaList`] owner. [`ViewportAnchor`] is retained only for explicit
//! transitions between genuinely different owners. Painting
//! lives in `crate::app::render::components::media_list`.

use crate::app::components::component_id::BrowserKey;
use crate::app::ui_util::move_cursor;
use ratatui::layout::{Position, Rect};
use ratatui::style::Color;
use std::time::Instant;

mod anchor;
mod carrier;
mod grouping;
mod inline;
mod selection;
#[cfg(test)]
mod tests;
mod wide;

pub use anchor::ViewportAnchor;
pub use carrier::{MediaListCarrier, Presentation};
pub use grouping::letter_grouped_rows;
pub use inline::{InlineLayout, InlineMediaBrowser};
pub use wide::WideMediaList;

/// Flow-space geometry for a painted media-list control.
///
/// Rows contain the source-row lookup used by painters and an optional stable
/// target for compatibility hit maps. Replacement continuation rows contain
/// neither; the selected replacement row contains only the selected target.
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

    /// Stable targets parallel to the flow rows.
    pub fn targets(&self) -> impl Iterator<Item = Option<&Target>> {
        self.rows.iter().map(|row| row.target.as_ref())
    }

    /// Absolute one-line rectangles for rows visible in `area`.
    pub fn visible_rows(&self, area: Rect) -> Vec<Rect> {
        (self.offset..self.rows.len())
            .take(area.height as usize)
            .map(|row| Rect {
                y: area.y + (row - self.offset) as u16,
                height: 1,
                ..area
            })
            .collect()
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

    fn replacement(
        rows: &[MediaListRow<Target>],
        selected_row: usize,
        detail_rows: usize,
        offset: usize,
    ) -> Self {
        let mut flow = Vec::with_capacity(rows.len() - 1 + detail_rows);
        let mut selected_flow_row = None;
        for (source_row, row) in rows.iter().enumerate() {
            if source_row == selected_row {
                selected_flow_row = Some(flow.len());
                flow.extend((0..detail_rows).map(|detail_row| FlowRow {
                    source_row: None,
                    target: if detail_row == 0 {
                        row.selectable_target().cloned()
                    } else {
                        None
                    },
                }));
            } else {
                flow.push(FlowRow {
                    source_row: Some(source_row),
                    target: row.selectable_target().cloned(),
                });
            }
        }
        Self {
            offset,
            rows: flow,
            selected_row: selected_flow_row,
        }
    }
}

/// A bounded percentage used by active canonical media-list rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveProgress(u8);

impl ActiveProgress {
    /// Clamps a percentage into the permitted `0..=100` range.
    pub fn new(percent: u16) -> Self {
        Self(percent.min(100) as u8)
    }

    pub fn percent(self) -> u8 {
        self.0
    }
}

/// Whether a canonical media-list item drills into a container or plays a leaf.
///
/// `Collection` rows are navigable containers (movie/series folder, album,
/// podcast show, book title); `Media` rows are playable leaves (queue entries,
/// episodes, tracks, chapters, feed items). The painter suppresses the duration
/// slot for `Collection` rows even when one is projected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaKind {
    Collection,
    Media,
}

/// Provider-neutral semantic state used by canonical media-list rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaSemanticState {
    Ordinary,
    Played,
    Active { progress: Option<ActiveProgress> },
    NowPlaying { progress: Option<ActiveProgress> },
    Starting,
    Disabled,
}

impl MediaSemanticState {
    /// Constructs active state, clamping prepared progress to the permitted range.
    pub fn active(progress: Option<u16>) -> Self {
        Self::Active {
            progress: progress.map(ActiveProgress::new),
        }
    }
}

/// Which semantic surface should receive the selected-row treatment.
///
/// The policy is deliberately closed: callers choose a named surface identity,
/// never a raw Ratatui style or colour. An owning-surface selected row
/// resolves its containing column's focus pair, so the caller names which
/// column it sits in; the painter maps each variant to the table's declared
/// selected-row identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectedRowSurface {
    ListBackdrop,
    OwningQueueColumn,
    OwningLibraryPane,
}

/// Semantic paint policy for one `WideMediaList` view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZebraStripe {
    pub focused: Color,
    pub unfocused: Color,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WideMediaListPaintPolicy {
    focused: bool,
    selected_surface: SelectedRowSurface,
    zebra: Option<ZebraStripe>,
}

impl WideMediaListPaintPolicy {
    pub const fn new(focused: bool) -> Self {
        Self {
            focused,
            selected_surface: SelectedRowSurface::ListBackdrop,
            zebra: None,
        }
    }

    pub const fn for_queue(focused: bool) -> Self {
        Self {
            focused,
            selected_surface: SelectedRowSurface::OwningQueueColumn,
            zebra: None,
        }
    }

    pub const fn for_library_workspace(focused: bool) -> Self {
        Self {
            focused,
            selected_surface: SelectedRowSurface::OwningLibraryPane,
            zebra: None,
        }
    }

    pub const fn with_zebra(mut self, zebra: ZebraStripe) -> Self {
        self.zebra = Some(zebra);
        self
    }

    pub(crate) fn zebra_bg(self) -> Option<Color> {
        self.zebra.map(|zebra| {
            if self.focused {
                zebra.focused
            } else {
                zebra.unfocused
            }
        })
    }

    pub(crate) const fn focused(self) -> bool {
        self.focused
    }

    pub(crate) const fn selected_surface(self) -> SelectedRowSurface {
        self.selected_surface
    }
}

/// Semantic paint policy for one `InlineMediaBrowser` view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InlineMediaBrowserPaintPolicy {
    focused: bool,
    selected_surface: SelectedRowSurface,
    desired_detail_rows: usize,
}

impl InlineMediaBrowserPaintPolicy {
    pub const fn new(focused: bool, desired_detail_rows: usize) -> Self {
        Self {
            focused,
            selected_surface: SelectedRowSurface::ListBackdrop,
            desired_detail_rows,
        }
    }

    pub(crate) const fn focused(self) -> bool {
        self.focused
    }

    pub(crate) const fn selected_surface(self) -> SelectedRowSurface {
        self.selected_surface
    }

    pub(crate) const fn desired_detail_rows(self) -> usize {
        self.desired_detail_rows
    }
}

/// Pointer surface input resolved by a mounted presentation. Convert this to
/// a target-bearing operation before delegating to the canonical owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaListSurfaceInput {
    Move(i64),
    Page(i64),
    First,
    Last,
    Activate,
    Context,
    Click(Position),
    ToggleClick(Position),
    RangeClick(Position),
    DoubleClick(Position),
    ContextClick(Position),
    Wheel { at: Position, delta: i64 },
}

/// Target-resolved operation accepted by the canonical media-list owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaListOperation<Target> {
    Move(i64),
    Page(i64),
    First,
    Last,
    ActivateCurrent,
    ContextCurrent,
    Select(Target),
    Toggle(Target),
    Range(Target),
    Activate(Target),
    Context(Target),
    ContextSelection,
}

impl MediaListSurfaceInput {
    pub fn into_operation<Target>(
        self,
        target: Option<Target>,
    ) -> Option<MediaListOperation<Target>> {
        Some(match self {
            Self::Move(delta) => MediaListOperation::Move(delta),
            Self::Page(delta) => MediaListOperation::Page(delta),
            Self::First => MediaListOperation::First,
            Self::Last => MediaListOperation::Last,
            Self::Activate => MediaListOperation::ActivateCurrent,
            Self::Context => MediaListOperation::ContextCurrent,
            Self::Wheel { delta, .. } => MediaListOperation::Move(delta),
            Self::Click(_) => MediaListOperation::Select(target?),
            Self::ToggleClick(_) => MediaListOperation::Toggle(target?),
            Self::RangeClick(_) => MediaListOperation::Range(target?),
            Self::DoubleClick(_) => MediaListOperation::Activate(target?),
            Self::ContextClick(_) => MediaListOperation::Context(target?),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaListDisposition {
    Unhandled,
    Consumed,
}

/// Stable coordination identity for a MediaList selection. This identifies
/// the list that produced a projection or delayed action; it never carries
/// selection membership.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SelectionOrigin {
    Library(LibrarySelectionOrigin),
    Queue,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LibrarySelectionOrigin {
    Home,
    Feeds,
    Service(BrowserKey),
}

/// Read-only presentation projection of a MediaList selection. Membership is
/// deliberately private to the owner and cannot be reconstructed here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectionSummary {
    pub count: usize,
    pub origin: SelectionOrigin,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaListTransition<Target> {
    pub disposition: MediaListDisposition,
    pub selected_target: Option<Target>,
    pub selection_summary: Option<SelectionSummary>,
    pub external_intent: Option<RowIntent<Target>>,
}

impl<Target> MediaListTransition<Target> {
    pub(crate) fn unhandled() -> Self {
        Self {
            disposition: MediaListDisposition::Unhandled,
            selected_target: None,
            selection_summary: None,
            external_intent: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowIntent<Target> {
    Activate(Target),
    Context(Target),
    ContextSelection(Vec<Target>),
}

/// A closed, provider-neutral row vocabulary for embedded media lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaListRow<Target> {
    Item {
        target: Target,
        primary: String,
        /// Optional secondary title painted after `primary` in the yellow
        /// focus-accent role (the episode title of a series/show row).
        secondary: Option<String>,
        /// Left-aligned FOAM metadata rendered right after `primary`.
        trailing: Option<String>,
        /// A duration/time string. Rendered as a distinct right-aligned
        /// green (`STATUS_AVAILABLE`) element, never as FOAM `trailing`.
        duration: Option<String>,
        /// Drill-in container vs. playable leaf; gates the duration slot.
        kind: MediaKind,
        semantic_state: MediaSemanticState,
    },
    Heading {
        text: String,
    },
    Spacer,
}

impl<Target> MediaListRow<Target> {
    /// Returns the stable identity only for selectable item rows.
    pub fn selectable_target(&self) -> Option<&Target> {
        match self {
            Self::Item { target, .. } => Some(target),
            Self::Heading { .. } | Self::Spacer => None,
        }
    }
}

/// The clamped one-column viewport of a [`MediaList`] for a given painted
/// height: `offset` is the display-row index at the viewport top, so display
/// row `i` paints at screen row `i - offset`. `total_rows` counts every
/// display row (items, headings, spacers alike).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WideViewport {
    pub offset: usize,
    pub height: usize,
    pub total_rows: usize,
}

impl WideViewport {
    /// The largest `offset` that still fills the viewport.
    pub fn max_offset(&self) -> usize {
        self.total_rows.saturating_sub(self.height)
    }

    /// Whether the content is taller than the viewport (scrollbar territory).
    pub fn overflows(&self) -> bool {
        self.total_rows > self.height
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
            marquee_text: String::new(),
            marquee_started_at: Instant::now(),
        }
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

    /// Number of selectable rows.
    fn selectable_len(&self) -> usize {
        self.selectable.len()
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
            MediaListOperation::ContextSelection => None,
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
