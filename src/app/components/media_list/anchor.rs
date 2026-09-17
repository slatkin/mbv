// The anchor seam carries a stable target plus a screen-row offset across a
// change to the row flow's geometry (design.md D3/D5): the breakpoint hand-off
// uses the selection form; `MediaList::set_content` uses the window-top form
// for the row-flow replacement anchor.

use super::{MediaList, MediaListRow};

/// The selection state a responsive parent hands from one canonical
/// media-list control to another at a breakpoint transition (design.md D3):
/// `selected_row_offset` is the zero-based screen-row offset from the viewport
/// top to the selected ordinary row. The receiving control restores the
/// target and places it at that offset where the receiving geometry allows,
/// clamping otherwise. The persisted resting position stays shell-owned; this
/// value carries no cursor/scroll mirror.
///
/// The same shape records the row-flow replacement anchor (design.md D5):
/// `selected_target` is the first selectable target the previous flow showed
/// at the window's top and `selected_row_offset` is 1 when a `Heading` sat
/// directly above it (the window is restored to that `Heading`), 0 otherwise.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewportAnchor<Target> {
    pub selected_target: Target,
    pub selected_row_offset: usize,
}

impl<Target: Clone + PartialEq> MediaList<Target> {
    /// Produce an anchor from the current selection for a painted viewport
    /// height. `None` when nothing is selectable.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn viewport_anchor(&self, viewport_height: usize) -> Option<ViewportAnchor<Target>> {
        Some(ViewportAnchor {
            selected_target: self.selected_target()?.clone(),
            selected_row_offset: self.selected_row_offset(viewport_height)?,
        })
    }

    /// Restore `anchor` at a painted viewport height: select the target when
    /// it is present, then set the resting scroll so the selected row lands at
    /// the requested offset where the receiving geometry allows, clamping
    /// otherwise. The height-aware clamp in `resolve_viewport` still applies
    /// at paint time.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn apply_viewport_anchor(
        &mut self,
        anchor: &ViewportAnchor<Target>,
        viewport_height: usize,
    ) {
        if let Some(cursor) = self.position_of(&anchor.selected_target) {
            self.cursor = cursor;
        }
        let height = viewport_height.max(1);
        let Some(row) = self.selected_display_row() else {
            return;
        };
        let max_offset = self.rows().len().saturating_sub(height);
        self.scroll = row
            .saturating_sub(anchor.selected_row_offset)
            .min(max_offset);
    }

    /// Record the row-flow replacement anchor (design.md D5): the first
    /// selectable target the current flow shows at the window's top, with
    /// whether a `Heading` sat directly above it (encoded as the anchor's
    /// row offset). `None` when the window shows no selectable row.
    pub(super) fn flow_anchor(&self) -> Option<ViewportAnchor<Target>> {
        let &row = self
            .selectable
            .iter()
            .find(|&&candidate| candidate >= self.scroll)?;
        let heading_above =
            row > 0 && matches!(self.rows.get(row - 1), Some(MediaListRow::Heading { .. }));
        Some(ViewportAnchor {
            selected_target: self.rows[row].selectable_target()?.clone(),
            selected_row_offset: usize::from(heading_above),
        })
    }

    /// Restore a row-flow replacement anchor (design.md D5): place the
    /// window at the anchored target's row — or one row higher, where the
    /// anchor recorded a `Heading` directly above it and the new flow still
    /// places one (the caller matched that structurally into the recorded
    /// offset; a `Heading` has no stable identity and is never matched by
    /// text). The selection is untouched. Returns `false` — leaving the
    /// caller's fallback in charge — when the target is gone from the flow.
    pub(super) fn apply_flow_anchor(&mut self, anchor: &ViewportAnchor<Target>) -> bool {
        let Some(&row) = self
            .position_of(&anchor.selected_target)
            .and_then(|index| self.selectable.get(index))
        else {
            return false;
        };
        self.scroll = row.saturating_sub(anchor.selected_row_offset);
        true
    }
}
