use super::{ListCore, MediaListRow, ViewportAnchor, WideViewport};

/// Embedded plain fixed-height, one-column media list: owns the display-row
/// list, the selectable index over it, the cursor, and the resting scroll
/// offset through the shared [`ListCore`]. It has no mouse hit-resolution API
/// and accepts no column-count or inline-detail options (design.md D1).
/// Painting is
/// `crate::app::render::components::media_list::render_wide_media_list`.
pub struct WideMediaList<Target> {
    core: ListCore<Target>,
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
        }
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

impl<Target: Clone + PartialEq> WideMediaList<Target> {
    /// Replace the display rows, preserving the selected target where possible
    /// and locally clamping otherwise (design.md D3).
    pub fn set_content(&mut self, rows: Vec<MediaListRow<Target>>) {
        self.core.set_content(rows);
    }

    /// Move the cursor to `target` when it is present; returns whether it was.
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
        self.core.apply_viewport_anchor(anchor, viewport_height);
    }
}
