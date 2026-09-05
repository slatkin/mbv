use super::ListCore;

/// The selection state a responsive parent hands from one canonical
/// media-list control to another at a breakpoint transition (design.md D3):
/// `selected_row_offset` is the zero-based screen-row offset from the viewport
/// top to the selected ordinary row. The receiving control restores the
/// target and places it at that offset where the receiving geometry allows,
/// clamping otherwise. The persisted resting position stays shell-owned; this
/// value carries no cursor/scroll mirror.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewportAnchor<Target> {
    pub selected_target: Target,
    pub selected_row_offset: usize,
}

impl<Target: Clone + PartialEq> ListCore<Target> {
    /// Produce an anchor from the current selection for a painted viewport
    /// height. `None` when nothing is selectable.
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
}
