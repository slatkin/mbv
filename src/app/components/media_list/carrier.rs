//! Destination-side carrier for one logical media-row flow (design.md D1/D2).
//!
//! A destination composes one [`MediaListCarrier`] holding the persistent
//! [`WideMediaList`] and [`InlineMediaBrowser`] presentations over a single
//! [`MediaList`](super::MediaList) owner. [`Presentation`] is the
//! centrally-defined closed set; the carrier owns which member currently
//! holds the owner and moves the owner between them on a responsive change,
//! preserving only the outgoing selected-row viewport offset. Destinations
//! derive the active member from their own kind, breakpoint, and painted
//! chrome and retain provider content, chrome, pane state, and typed intent
//! translation.
//!
//! Task 5.8 (unify-screens-under-panel-components): `Presentation::Grid` is
//! deleted (design D13 — the Grid presentation served only non-hero catalogs
//! and no library in use lacks a hero), and `ensure_presentation` is renamed
//! to `set_presentation`, the panel-equivalent API (design D3): the Library
//! panel calls `set_presentation(Wide | Inline, ..)` from its own breakpoint
//! choice through the object-safe [`super::PanelList`]-style surface, and
//! un-migrated destinations call the same method from theirs.

use ratatui::layout::{Position, Rect};
use tuirealm::event::{Key, KeyEvent, KeyModifiers};

use super::{
    InlineMediaBrowser, MediaListRow, RowLocalInput, RowLocalOutcome, ViewportAnchor, WideMediaList,
};

/// The centrally-defined closed set of media-list presentations (CONTEXT.md
/// "Presentation (media-list)"). Every destination derives the active member
/// from its kind, breakpoint, and painted chrome; it never invents a new one.
/// The Grid presentation was deleted with the non-hero catalogs it served
/// (design D13, task 5.8); the Library panel drives `set_presentation` for
/// converted owners, and the two remaining members are all a media-row flow
/// can hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presentation {
    Wide,
    Inline,
}

/// One logical row flow's destination-side carrier: the persistent
/// presentation adapters (exactly one of which holds the shared owner at a
/// time) plus which one is active. A responsive change moves the same owner
/// between the adapters — the owner is never copied and no row-local state is
/// transferred.
pub struct MediaListCarrier<Target> {
    active: Presentation,
    wide: WideMediaList<Target>,
    inline: InlineMediaBrowser<Target>,
    selection_changed: bool,
}

impl<Target> MediaListCarrier<Target> {
    /// Create a carrier whose owner starts in `active`.
    pub fn new(active: Presentation) -> Self {
        Self {
            active,
            wide: WideMediaList::new(),
            inline: InlineMediaBrowser::new(),
            selection_changed: false,
        }
    }

    /// The presentation currently holding the shared owner. Un-migrated
    /// destinations read this to hand their painters the active adapter;
    /// once a destination converts to a Library panel content owner, the
    /// panel drives `set_presentation` instead (design D3).
    pub fn active(&self) -> Presentation {
        self.active
    }

    /// The Wide presentation adapter. Used by the render/paint layer and by
    /// destinations that own a Wide-only geometry contract.
    pub fn wide(&self) -> &WideMediaList<Target> {
        &self.wide
    }

    /// Mutable Wide presentation adapter.
    pub fn wide_mut(&mut self) -> &mut WideMediaList<Target> {
        &mut self.wide
    }

    /// The Inline presentation adapter.
    pub fn inline(&self) -> &InlineMediaBrowser<Target> {
        &self.inline
    }

    /// Mutable Inline presentation adapter.
    pub fn inline_mut(&mut self) -> &mut InlineMediaBrowser<Target> {
        &mut self.inline
    }

    /// The active owner's stable selected target.
    pub fn selected_target(&self) -> Option<&Target> {
        match self.active {
            Presentation::Wide => self.wide.selected_target(),
            Presentation::Inline => self.inline.selected_target(),
        }
    }

    pub fn multi_selection(&self) -> &[Target] {
        match self.active {
            Presentation::Wide => self.wide.multi_selection(),
            Presentation::Inline => self.inline.multi_selection(),
        }
    }

    pub fn is_visual_mode(&self) -> bool {
        !self.multi_selection().is_empty()
    }

    /// The active owner's cursor as an index into its selectable rows.
    pub fn cursor(&self) -> usize {
        match self.active {
            Presentation::Wide => self.wide.cursor(),
            Presentation::Inline => self.inline.cursor(),
        }
    }

    /// The active owner's resting scroll offset.
    pub fn scroll(&self) -> usize {
        match self.active {
            Presentation::Wide => self.wide.scroll(),
            Presentation::Inline => self.inline.scroll(),
        }
    }

    /// The active owner's painted rows.
    pub fn rows(&self) -> &[MediaListRow<Target>] {
        match self.active {
            Presentation::Wide => self.wide.rows(),
            Presentation::Inline => self.inline.rows(),
        }
    }

    /// The active presentation's retained current-frame content rect, when it
    /// has completed a view. Used by destinations that own a
    /// presentation-specific viewport geometry contract (Grouped Music's
    /// responsive anchor hand-off); a fresh/moved presentation retains none.
    pub fn current_content_rect(&self) -> Option<Rect> {
        match self.active {
            Presentation::Wide => self.wide.current_content_rect(),
            Presentation::Inline => self.inline.current_content_rect(),
        }
    }

    /// The active presentation's retained selected-row rect, when it has
    /// completed a view with a selectable row.
    pub fn current_selected_row_rect(&self) -> Option<Rect> {
        match self.active {
            Presentation::Wide => self.wide.current_selected_row_rect(),
            Presentation::Inline => self.inline.current_selected_row_rect(),
        }
    }

    /// Number of rows in the active presentation's retained current-frame
    /// flow, when it has completed a view.
    pub fn current_flow_len(&self) -> Option<usize> {
        match self.active {
            Presentation::Wide => self.wide.current_flow_len(),
            Presentation::Inline => self.inline.current_flow_len(),
        }
    }

    /// The retained current-frame flow target at `row`.
    pub fn current_flow_target_at(&self, row: usize) -> Option<Option<&Target>> {
        match self.active {
            Presentation::Wide => self.wide.current_flow_target_at(row),
            Presentation::Inline => self.inline.current_flow_target_at(row),
        }
    }

    /// The retained current-frame flow offset.
    pub fn current_flow_offset(&self) -> Option<usize> {
        match self.active {
            Presentation::Wide => self.wide.current_flow_offset(),
            Presentation::Inline => self.inline.current_flow_offset(),
        }
    }

    /// Invalidate the active presentation's retained paint geometry when a
    /// different owner (such as Inline Search) takes over the surface.
    pub fn invalidate_paint(&mut self) {
        match self.active {
            Presentation::Wide => self.wide.invalidate_paint(),
            Presentation::Inline => self.inline.invalidate_paint(),
        }
    }

    /// Whether the active owner has no selectable rows.
    pub fn is_empty(&self) -> bool {
        match self.active {
            Presentation::Wide => self.wide.is_empty(),
            Presentation::Inline => self.inline.is_empty(),
        }
    }
}

impl<Target: Clone + PartialEq> MediaListCarrier<Target> {
    /// Move the shared owner into `presentation` when it diverges from the
    /// active one. A responsive change moves the same owner between the
    /// persistent adapters and preserves only the outgoing selected-row
    /// viewport offset (design.md D2); no cursor, scroll, or selection is
    /// copied between presentations. The `viewport_height` is the receiving
    /// presentation's painted row height: the outgoing selection's offset is
    /// measured against it so the selected row lands at the same screen row
    /// (design D3's `set_presentation(Wide | Inline, anchor)`; the anchor is
    /// derived here from the carrier's own retained selection).
    pub fn set_presentation(&mut self, presentation: Presentation, viewport_height: usize) {
        if self.active == presentation {
            return;
        }
        // The outgoing presentation's painted content height is the viewport
        // its retained selected-row offset was measured against; fall back to
        // the receiving height before the first successful paint.
        let outgoing_height = self
            .current_content_rect()
            .map(|rect| rect.height.max(1) as usize)
            .unwrap_or_else(|| viewport_height.max(1));
        let handoff = self.viewport_anchor(outgoing_height);
        let core = match self.active {
            Presentation::Wide => std::mem::take(&mut self.wide).into_media_list(),
            Presentation::Inline => std::mem::take(&mut self.inline).into_media_list(),
        };
        match presentation {
            Presentation::Wide => self.wide = WideMediaList::from_media_list(core),
            Presentation::Inline => self.inline = InlineMediaBrowser::from_media_list(core),
        }
        self.active = presentation;
        if let Some(anchor) = handoff {
            self.apply_viewport_anchor(&anchor, viewport_height);
        }
    }

    /// Replace the active owner's display rows, preserving the selected
    /// target where possible and locally clamping otherwise (design.md D3).
    pub fn set_content(&mut self, rows: Vec<MediaListRow<Target>>) {
        let before = self.multi_selection().len();
        match self.active {
            Presentation::Wide => self.wide.set_content(rows),
            Presentation::Inline => self.inline.set_content(rows),
        }
        self.selection_changed |= before != self.multi_selection().len();
    }

    /// Move the active owner's selection to `target` when it is present.
    pub fn select_target(&mut self, target: &Target) -> bool {
        match self.active {
            Presentation::Wide => self.wide.select_target(target),
            Presentation::Inline => self.inline.select_target(target),
        }
    }

    pub fn enter_visual_mode(&mut self) {
        match self.active {
            Presentation::Wide => self.wide.enter_visual_mode(),
            Presentation::Inline => self.inline.enter_visual_mode(),
        }
        self.selection_changed = true;
    }

    /// Handle the shared Visual-mode chords after destination-local
    /// preemption (notably Inline Search) has had first refusal.
    pub fn handle_visual_key(&mut self, key: &KeyEvent) -> Option<usize> {
        if matches!(key.code, Key::Char('v') | Key::Char('V'))
            && key.modifiers == KeyModifiers::SHIFT
        {
            self.enter_visual_mode();
            self.selection_changed = false;
            return Some(self.multi_selection().len());
        }
        if !self.is_visual_mode() || !key.modifiers.is_empty() {
            return None;
        }
        match key.code {
            Key::Esc => {
                self.clear_selection();
                self.selection_changed = false;
                Some(0)
            }
            Key::Char(' ') => {
                let target = self.selected_target()?.clone();
                self.toggle_selection(&target);
                self.selection_changed = false;
                Some(self.multi_selection().len())
            }
            _ => None,
        }
    }

    pub fn toggle_selection(&mut self, target: &Target) {
        let before = self.multi_selection().len();
        match self.active {
            Presentation::Wide => self.wide.toggle_selection(target),
            Presentation::Inline => self.inline.toggle_selection(target),
        }
        self.selection_changed |= before != self.multi_selection().len();
    }

    pub fn extend_selection_to(&mut self, target: &Target) {
        let before = self.multi_selection().len();
        match self.active {
            Presentation::Wide => self.wide.extend_selection_to(target),
            Presentation::Inline => self.inline.extend_selection_to(target),
        }
        self.selection_changed |= before != self.multi_selection().len();
    }

    pub fn clear_selection(&mut self) {
        if !self.multi_selection().is_empty() {
            self.selection_changed = true;
        }
        match self.active {
            Presentation::Wide => self.wide.clear_selection(),
            Presentation::Inline => self.inline.clear_selection(),
        }
    }

    /// Return the count from the most recent selection mutation, once. This
    /// keeps pointer selection forwarding at the component boundary without
    /// making the shell inspect the carrier's local state.
    pub fn selection_changed_msg(&mut self) -> Option<usize> {
        if !self.selection_changed {
            return None;
        }
        self.selection_changed = false;
        Some(self.multi_selection().len())
    }

    /// Replace one existing active-owner row by stable target without
    /// rebuilding indexes or disturbing selection/scroll.
    pub fn patch_row(&mut self, target: &Target, row: MediaListRow<Target>) -> bool {
        match self.active {
            Presentation::Wide => self.wide.patch_row(target, row),
            Presentation::Inline => self.inline.patch_row(target, row),
        }
    }

    /// Select the active owner's first selectable row.
    pub fn select_first(&mut self) {
        match self.active {
            Presentation::Wide => self.wide.select_first(),
            Presentation::Inline => self.inline.select_first(),
        }
    }

    /// Select the active owner's last selectable row.
    pub fn select_last(&mut self) {
        match self.active {
            Presentation::Wide => self.wide.select_last(),
            Presentation::Inline => self.inline.select_last(),
        }
    }

    /// Place the active owner's selection at selectable index `index`, clamped
    /// to the last row. Used for a shell-owned discrete re-anchor (design.md
    /// D5), not for row-local input, which goes through [`Self::delegate`].
    pub fn select_index(&mut self, index: usize) {
        match self.active {
            Presentation::Wide => self.wide.select_index(index),
            Presentation::Inline => self.inline.select_index(index),
        }
    }

    /// Move the active owner's selection by `delta` selectable rows.
    pub fn move_selection(&mut self, delta: i64) {
        match self.active {
            Presentation::Wide => self.wide.move_selection(delta),
            Presentation::Inline => self.inline.move_selection(delta),
        }
    }

    /// Store the offset a painter resolved, so the next frame resumes from it.
    pub fn set_scroll(&mut self, offset: usize) {
        match self.active {
            Presentation::Wide => self.wide.set_scroll(offset),
            Presentation::Inline => self.inline.set_scroll(offset),
        }
    }

    /// Resolve the active owner's viewport for `viewport_height` and retain
    /// the resulting resting offset.
    pub fn sync_viewport(&mut self, viewport_height: usize) {
        let viewport_height = viewport_height.max(1);
        let offset = match self.active {
            Presentation::Wide => self.wide.resolve_viewport(viewport_height).offset,
            Presentation::Inline => self.inline.resolve_viewport(viewport_height).offset,
        };
        self.set_scroll(offset);
    }

    /// Offer one already-normalized row-local input to the active owner,
    /// returning its closed provider-neutral outcome.
    pub fn delegate(
        &mut self,
        input: RowLocalInput,
        target: Option<Target>,
    ) -> RowLocalOutcome<Target> {
        let before = self.multi_selection().len();
        let outcome = match self.active {
            Presentation::Wide => self.wide.delegate(input, target),
            Presentation::Inline => self.inline.delegate(input, target),
        };
        self.selection_changed |= before != self.multi_selection().len();
        outcome
    }

    /// Whether the active presentation's retained current frame claims
    /// `point`. This respects D6 frame invalidation: a presentation configured
    /// for a new frame that has not completed its view claims nothing, so a
    /// destination that re-projects its rows on every sync (Feeds) or a stale
    /// frame cannot accept pointer input.
    pub fn claims_current_point(&self, point: Position) -> bool {
        match self.active {
            Presentation::Wide => self.wide.claims_current_point(point),
            Presentation::Inline => self.inline.claims_current_point(point),
        }
    }

    /// Resolve `point` to a stable target from the active presentation's
    /// retained current-frame geometry. The Inline presentation maps its whole
    /// selected-row replacement block, not only its first flow row, to the
    /// retained selected target.
    pub fn resolve_current_point(&self, point: Position) -> Option<&Target> {
        match self.active {
            Presentation::Wide => self.wide.resolve_current_point(point),
            Presentation::Inline => self.inline.resolve_current_point(point),
        }
    }

    /// Produce a [`ViewportAnchor`] from the active presentation's current
    /// selection for a painted viewport height. `None` when nothing is
    /// selectable.
    pub fn viewport_anchor(&self, viewport_height: usize) -> Option<ViewportAnchor<Target>> {
        let selected_target = self.selected_target()?.clone();
        let selected_row_offset = match self.active {
            Presentation::Wide => self.wide.selected_row_offset(viewport_height)?,
            Presentation::Inline => self.inline.selected_row_offset(viewport_height)?,
        };
        Some(ViewportAnchor {
            selected_target,
            selected_row_offset,
        })
    }

    /// Restore `anchor` on the active presentation, clamping when the
    /// receiving geometry cannot honor the requested offset (design.md D3).
    pub fn apply_viewport_anchor(
        &mut self,
        anchor: &ViewportAnchor<Target>,
        viewport_height: usize,
    ) {
        match self.active {
            Presentation::Wide => self.wide.apply_viewport_anchor(anchor, viewport_height),
            Presentation::Inline => self.inline.apply_viewport_anchor(anchor, viewport_height),
        }
    }
}
