use super::{ListCore, MediaListRow, ViewportAnchor, WideViewport};

/// Resolved geometry for one painted frame of an [`InlineMediaBrowser`]
/// (design.md D1). `detail_rows == 0` means the detail block did not fit the
/// painted viewport and the browser fell back to painting the ordinary
/// selected row; otherwise the block replaces the selected row in the flow
/// and `detail_screen_row` is its row offset from the viewport top.
pub struct InlineLayout {
    /// Display-row index at the viewport top.
    pub offset: usize,
    /// Painted viewport height (at least 1).
    pub height: usize,
    /// Admitted detail-block height, or `0` on fallback.
    pub detail_rows: usize,
    /// The detail block's row offset from the viewport top, when admitted.
    pub detail_screen_row: Option<usize>,
    /// Total display rows after the selected row is swallowed by the block.
    pub total_display_rows: usize,
}

/// Embedded plain media browser that replaces the selected row with a
/// variable-height detail block when it fits, and falls back to the ordinary
/// row when it does not (design.md D1). Shares
/// [`WideMediaList`](super::WideMediaList)'s list mechanics through the
/// private [`ListCore`]; the fit admission, fallback, and replacement paint
/// geometry live in [`resolve_inline_layout`](Self::resolve_inline_layout).
/// No mouse hit-resolution API (design.md D4). Painting is
/// `crate::app::render::components::media_list::render_inline_media_browser`.
pub struct InlineMediaBrowser<Target> {
    core: ListCore<Target>,
}

impl<Target> Default for InlineMediaBrowser<Target> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Target> InlineMediaBrowser<Target> {
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

    /// The clamped ordinary-row viewport for a painted `viewport_height`,
    /// keeping the selected row on screen. This is the fallback flow and the
    /// geometry the [`ViewportAnchor`] is measured against.
    pub fn resolve_viewport(&self, viewport_height: usize) -> WideViewport {
        self.core.resolve_viewport(viewport_height)
    }

    /// Zero-based screen-row offset from the viewport top to the selected
    /// ordinary row, for the responsive [`ViewportAnchor`] hand-off
    /// (design.md D3).
    pub fn selected_row_offset(&self, viewport_height: usize) -> Option<usize> {
        self.core.selected_row_offset(viewport_height)
    }

    /// Resolve the replacement paint geometry for a painted `viewport_height`
    /// and a `desired_detail_rows` detail block. The block is admitted only
    /// when it is shorter than the viewport, leaving room for at least one
    /// ordinary row (mirrors `hero::inline_detail_flow`'s admission test in
    /// the render layer); otherwise the browser falls back to the ordinary
    /// selected row and this returns `detail_rows == 0`.
    pub fn resolve_inline_layout(
        &self,
        viewport_height: usize,
        desired_detail_rows: usize,
    ) -> InlineLayout {
        let height = viewport_height.max(1);
        let source_rows = self.core.rows().len();

        let admit = match self.core.selected_display_row() {
            Some(row) if desired_detail_rows > 0 && desired_detail_rows < height => Some(row),
            _ => None,
        };

        match admit {
            Some(row) => {
                let lower_bound = (row + desired_detail_rows).saturating_sub(height).min(row);
                let offset = self.core.scroll().clamp(lower_bound, row);
                InlineLayout {
                    offset,
                    height,
                    detail_rows: desired_detail_rows,
                    detail_screen_row: Some(row - offset),
                    // One source row is swallowed by the block
                    // (`hero::inline_display_row_count`).
                    total_display_rows: source_rows - 1 + desired_detail_rows,
                }
            }
            None => {
                let viewport = self.core.resolve_viewport(height);
                InlineLayout {
                    offset: viewport.offset,
                    height,
                    detail_rows: 0,
                    detail_screen_row: None,
                    total_display_rows: viewport.total_rows,
                }
            }
        }
    }
}

impl<Target: Clone + PartialEq> InlineMediaBrowser<Target> {
    /// Replace the display rows, preserving the selected target where possible
    /// and locally clamping otherwise (design.md D3).
    pub fn set_content(&mut self, rows: Vec<MediaListRow<Target>>) {
        self.core.set_content(rows);
    }

    /// Produce a [`ViewportAnchor`] from the current selection for a painted
    /// viewport height (design.md D3). `None` when nothing is selectable.
    pub fn viewport_anchor(&self, viewport_height: usize) -> Option<ViewportAnchor<Target>> {
        self.core.viewport_anchor(viewport_height)
    }

    /// Restore a [`ViewportAnchor`] at a painted viewport height: select the
    /// target if present, then place it at the requested offset where the
    /// ordinary-row geometry allows, clamping otherwise (design.md D3).
    pub fn apply_viewport_anchor(
        &mut self,
        anchor: &ViewportAnchor<Target>,
        viewport_height: usize,
    ) {
        self.core.apply_viewport_anchor(anchor, viewport_height);
    }
}
