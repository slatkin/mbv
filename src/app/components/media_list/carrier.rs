//! Destination-side carrier for one logical media-row flow (design.md D1/D2).
//!
//! A destination composes one [`MediaListCarrier`] holding the persistent
//! [`WideMediaList`], [`InlineMediaBrowser`], and [`GridMediaList`]
//! presentations over a single [`MediaList`](super::MediaList) owner.
//! [`Presentation`] is the centrally-defined closed set; the carrier owns which
//! member currently holds the owner and moves the owner between them on a
//! responsive change, preserving only the outgoing selected-row viewport
//! offset. Destinations derive the active member from their own kind,
//! breakpoint, and painted chrome and retain provider content, chrome, pane
//! state, and typed intent translation.

use ratatui::layout::{Position, Rect};

use super::{
    GridMediaList, InlineMediaBrowser, MediaListRow, RowLocalInput, RowLocalOutcome,
    ViewportAnchor, WideMediaList,
};

/// The centrally-defined closed set of media-list presentations (CONTEXT.md
/// "Presentation (media-list)"). Every destination derives the active member
/// from its kind, breakpoint, and painted chrome; it never invents a new one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presentation {
    Wide,
    Inline,
    Grid,
}

/// One logical row flow's destination-side carrier: the three persistent
/// presentation adapters (exactly one of which holds the shared owner at a
/// time) plus which one is active. A responsive change moves the same owner
/// between the adapters — the owner is never copied and no row-local state is
/// transferred.
pub struct MediaListCarrier<Target> {
    active: Presentation,
    wide: WideMediaList<Target>,
    inline: InlineMediaBrowser<Target>,
    grid: GridMediaList<Target>,
}

impl<Target> MediaListCarrier<Target> {
    /// Create a carrier whose owner starts in `active`.
    pub fn new(active: Presentation) -> Self {
        Self {
            active,
            wide: WideMediaList::new(),
            inline: InlineMediaBrowser::new(),
            grid: GridMediaList::new(),
        }
    }

    /// The presentation currently holding the shared owner.
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

    /// The Grid presentation adapter.
    pub fn grid(&self) -> &GridMediaList<Target> {
        &self.grid
    }

    /// Mutable Grid presentation adapter.
    pub fn grid_mut(&mut self) -> &mut GridMediaList<Target> {
        &mut self.grid
    }

    /// The active owner's stable selected target.
    pub fn selected_target(&self) -> Option<&Target> {
        match self.active {
            Presentation::Wide => self.wide.selected_target(),
            Presentation::Inline => self.inline.selected_target(),
            Presentation::Grid => self.grid.selected_target(),
        }
    }

    /// The active owner's cursor as an index into its selectable rows.
    pub fn cursor(&self) -> usize {
        match self.active {
            Presentation::Wide => self.wide.cursor(),
            Presentation::Inline => self.inline.cursor(),
            Presentation::Grid => self.grid.cursor(),
        }
    }

    /// The active owner's resting scroll offset.
    pub fn scroll(&self) -> usize {
        match self.active {
            Presentation::Wide => self.wide.scroll(),
            Presentation::Inline => self.inline.scroll(),
            Presentation::Grid => self.grid.scroll(),
        }
    }

    /// The active owner's painted rows.
    pub fn rows(&self) -> &[MediaListRow<Target>] {
        match self.active {
            Presentation::Wide => self.wide.rows(),
            Presentation::Inline => self.inline.rows(),
            Presentation::Grid => self.grid.rows(),
        }
    }

    /// Whether the active owner has no selectable rows.
    pub fn is_empty(&self) -> bool {
        match self.active {
            Presentation::Wide => self.wide.is_empty(),
            Presentation::Inline => self.inline.is_empty(),
            Presentation::Grid => self.grid.is_empty(),
        }
    }
}

impl<Target: Clone + PartialEq> MediaListCarrier<Target> {
    /// Move the shared owner into `presentation` when it diverges from the
    /// active one. A responsive change moves the same owner between the
    /// persistent adapters and preserves only the outgoing selected-row
    /// viewport offset (design.md D2); no cursor, scroll, or selection is
    /// copied between presentations.
    pub fn ensure_presentation(&mut self, presentation: Presentation, viewport_height: usize) {
        if self.active == presentation {
            return;
        }
        let handoff = self.viewport_anchor(viewport_height);
        let core = match self.active {
            Presentation::Wide => std::mem::take(&mut self.wide).into_media_list(),
            Presentation::Inline => std::mem::take(&mut self.inline).into_media_list(),
            Presentation::Grid => std::mem::take(&mut self.grid).into_media_list(),
        };
        match presentation {
            Presentation::Wide => self.wide = WideMediaList::from_media_list(core),
            Presentation::Inline => self.inline = InlineMediaBrowser::from_media_list(core),
            Presentation::Grid => self.grid = GridMediaList::from_media_list(core),
        }
        self.active = presentation;
        if let Some(anchor) = handoff {
            self.apply_viewport_anchor(&anchor, viewport_height);
        }
    }

    /// Replace the active owner's display rows, preserving the selected
    /// target where possible and locally clamping otherwise (design.md D3).
    pub fn set_content(&mut self, rows: Vec<MediaListRow<Target>>) {
        match self.active {
            Presentation::Wide => self.wide.set_content(rows),
            Presentation::Inline => self.inline.set_content(rows),
            Presentation::Grid => self.grid.set_content(rows),
        }
    }

    /// Move the active owner's selection to `target` when it is present.
    pub fn select_target(&mut self, target: &Target) -> bool {
        match self.active {
            Presentation::Wide => self.wide.select_target(target),
            Presentation::Inline => self.inline.select_target(target),
            Presentation::Grid => self.grid.select_target(target),
        }
    }

    /// Replace one existing active-owner row by stable target without
    /// rebuilding indexes or disturbing selection/scroll.
    pub fn patch_row(&mut self, target: &Target, row: MediaListRow<Target>) -> bool {
        match self.active {
            Presentation::Wide => self.wide.patch_row(target, row),
            Presentation::Inline => self.inline.patch_row(target, row),
            Presentation::Grid => self.grid.patch_row(target, row),
        }
    }

    /// Select the active owner's first selectable row.
    pub fn select_first(&mut self) {
        match self.active {
            Presentation::Wide => self.wide.select_first(),
            Presentation::Inline => self.inline.select_first(),
            Presentation::Grid => self.grid.select_first(),
        }
    }

    /// Select the active owner's last selectable row.
    pub fn select_last(&mut self) {
        match self.active {
            Presentation::Wide => self.wide.select_last(),
            Presentation::Inline => self.inline.select_last(),
            Presentation::Grid => self.grid.select_last(),
        }
    }

    /// Place the active owner's selection at selectable index `index`, clamped
    /// to the last row. Used for a shell-owned discrete re-anchor (design.md
    /// D5), not for row-local input, which goes through [`Self::delegate`].
    pub fn select_index(&mut self, index: usize) {
        match self.active {
            Presentation::Wide => self.wide.select_index(index),
            Presentation::Inline => self.inline.select_index(index),
            Presentation::Grid => self.grid.select_index(index),
        }
    }

    /// Move the active owner's selection by `delta` selectable rows.
    pub fn move_selection(&mut self, delta: i64) {
        match self.active {
            Presentation::Wide => self.wide.move_selection(delta),
            Presentation::Inline => self.inline.move_selection(delta),
            Presentation::Grid => self.grid.move_selection(delta),
        }
    }

    /// Move by `item_rows` painted item rows: the Grid presentation preserves
    /// the selected column and clamps into the target row's nearest cell
    /// (the established two-column catalog traversal); one-column
    /// presentations stride one selectable row per item row.
    pub fn move_item_rows(&mut self, item_rows: i64) {
        match self.active {
            Presentation::Grid => self.grid.move_item_rows(item_rows),
            Presentation::Wide | Presentation::Inline => self.move_selection(item_rows),
        }
    }

    /// Store the offset a painter resolved, so the next frame resumes from it.
    pub fn set_scroll(&mut self, offset: usize) {
        match self.active {
            Presentation::Wide => self.wide.set_scroll(offset),
            Presentation::Inline => self.inline.set_scroll(offset),
            Presentation::Grid => self.grid.set_scroll(offset),
        }
    }

    /// Resolve the active owner's viewport for `viewport_height` and retain
    /// the resulting resting offset.
    pub fn sync_viewport(&mut self, viewport_height: usize) {
        let viewport_height = viewport_height.max(1);
        let offset = match self.active {
            Presentation::Wide => self.wide.resolve_viewport(viewport_height).offset,
            Presentation::Inline => self.inline.resolve_viewport(viewport_height).offset,
            Presentation::Grid => self.grid.resolve_viewport(viewport_height).offset,
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
        match self.active {
            Presentation::Wide => self.wide.delegate(input, target),
            Presentation::Inline => self.inline.delegate(input, target),
            Presentation::Grid => self.grid.delegate(input, target),
        }
    }

    /// Whether the active presentation's current frame claims `point` inside
    /// the parent-supplied `list_area`. Wide and Inline claim that region;
    /// Grid claims its own retained current-frame cells region.
    pub fn claims_point(&self, list_area: Rect, point: Position) -> bool {
        match self.active {
            Presentation::Wide => self.wide.claims_point(list_area, point),
            Presentation::Inline => self.inline.claims_point(list_area, point),
            Presentation::Grid => self.grid.claims_current_point(point),
        }
    }

    /// Whether the active presentation's retained current frame claims
    /// `point`. Unlike [`Self::claims_point`], this respects D6 frame
    /// invalidation: a presentation configured for a new frame that has not
    /// completed its view claims nothing, so a destination that re-projects
    /// its rows on every sync (Feeds) or a stale frame cannot accept pointer
    /// input.
    pub fn claims_current_point(&self, point: Position) -> bool {
        match self.active {
            Presentation::Wide => self.wide.claims_current_point(point),
            Presentation::Inline => self.inline.claims_current_point(point),
            Presentation::Grid => self.grid.claims_current_point(point),
        }
    }

    /// Resolve `point` to a stable target from the active presentation's
    /// retained current-frame geometry. The Inline presentation maps its whole
    /// selected-row replacement block, not only its first flow row, to the
    /// retained selected target.
    pub fn resolve_current_point(&self, point: Position) -> Option<&Target> {
        match self.active {
            Presentation::Wide => self.wide.resolve_current_point(point),
            Presentation::Grid => self.grid.resolve_current_point(point),
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
            Presentation::Grid => self.grid.selected_row_offset(viewport_height)?,
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
            Presentation::Grid => self.grid.apply_viewport_anchor(anchor, viewport_height),
        }
    }
}
