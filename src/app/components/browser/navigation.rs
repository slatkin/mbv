use super::BrowserComponent;
use super::Presentation;
use crate::app::components::component_id::BrowserKind;
use crate::app::components::media_list::ViewportAnchor;
use crate::app::library_column_width::library_column_count;

impl BrowserComponent {
    pub(super) fn reanchor_content(&mut self) {
        if let Some(anchor) = self.preserved_anchor.as_ref() {
            let target = anchor.selected_target.clone();
            self.carrier_select_target(&target);
        }
    }

    /// Return the column count of the active presentation: one for the Wide
    /// and Inline presentations, the arrangement's two-column policy for the
    /// Grid presentation.
    pub(super) fn columns(&self) -> usize {
        if self.wide_movies || self.uses_inline_control() {
            1
        } else {
            library_column_count(self.layout.left_area.width)
        }
    }

    /// Painted item rows the pager moves per PageUp/PageDown. One-column
    /// presentations stride one selectable row per painted row, while the
    /// Grid keeps its existing header-exclusion and height-sensitive stride.
    pub(super) fn page_rows(&self) -> i64 {
        self.layout.left_area.height.saturating_sub(1).max(1) as i64
    }

    pub(super) fn uses_inline_control(&self) -> bool {
        !self.wide_movies
            && (matches!(self.kind, BrowserKind::Movies | BrowserKind::HomeVideos)
                || self.narrow_extras.inline_hero.is_some()
                || self.narrow_extras.hero_placeholder)
    }

    /// Resolve the active presentation's target/offset into the stable string
    /// anchor exchanged by the TV breakpoint seam.
    pub(super) fn carrier_viewport_anchor(
        &self,
        viewport_height: usize,
    ) -> Option<ViewportAnchor<String>> {
        let selected_target = self.carrier_selected_target()?;
        let selected_row_offset = match self.carrier {
            Presentation::Wide => self.wide_list.selected_row_offset(viewport_height)?,
            Presentation::Inline => self.inline_browser.selected_row_offset(viewport_height)?,
            Presentation::Grid => self.grid.selected_row_offset(viewport_height)?,
        };
        Some(ViewportAnchor {
            selected_target,
            selected_row_offset,
        })
    }

    /// Apply an anchor to the presentation carrying the shared owner.
    pub(super) fn apply_anchor_to_carrier(
        &mut self,
        anchor: &ViewportAnchor<String>,
        viewport_height: usize,
    ) {
        match self.carrier {
            Presentation::Wide => self
                .wide_list
                .apply_viewport_anchor(anchor, viewport_height),
            Presentation::Inline => self
                .inline_browser
                .apply_viewport_anchor(anchor, viewport_height),
            Presentation::Grid => self.grid.apply_viewport_anchor(anchor, viewport_height),
        }
    }

    /// Move the shared owner by `item_rows` painted item rows. One-column
    /// presentations stride one selectable row per item row; the Grid
    /// presentation preserves the selected column and clamps into the target
    /// row's nearest cell (the established two-column catalog traversal).
    pub(super) fn move_by_item_rows(&mut self, item_rows: i64) -> usize {
        self.ensure_carrier();
        match self.carrier {
            Presentation::Grid => self.grid.move_item_rows(item_rows),
            _ => self.carrier_move_selection(item_rows),
        }
        self.carrier_sync_viewport();
        self.cursor()
    }

    /// Move by one selectable item in the shared owner's row order.
    pub(super) fn move_cursor_delta(&mut self, delta: i64) -> usize {
        self.ensure_carrier();
        self.carrier_move_selection(delta);
        if self.carrier_selected_target().is_some() {
            self.carrier_sync_viewport();
        }
        self.cursor()
    }

    /// Home/End select the first/last target in the shared owner (the same
    /// row order the active presentation paints).
    pub(super) fn jump_cursor(&mut self, to_end: bool) -> usize {
        self.ensure_carrier();
        if to_end {
            self.carrier_select_last();
        } else {
            self.carrier_select_first();
        }
        if self.carrier_selected_target().is_some() {
            self.carrier_sync_viewport();
        }
        self.cursor()
    }
}
