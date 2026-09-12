use super::BrowserComponent;
use crate::app::components::media_list::RowLocalInput;

impl BrowserComponent {
    pub(super) fn reanchor_content(&mut self) {
        if let Some(anchor) = self.preserved_anchor.as_ref() {
            let target = anchor.selected_target.clone();
            self.carrier.select_target(&target);
        }
    }

    /// Painted item rows the pager moves per PageUp/PageDown: the one-column
    /// presentations stride one selectable row per painted row. (The Grid
    /// presentation's column-preserving stride was deleted with Grid as
    /// unreachable, design D13.)
    pub(super) fn page_rows(&self) -> i64 {
        self.layout.left_area.height.saturating_sub(1).max(1) as i64
    }

    /// Move the shared owner by `item_rows` painted item rows: every
    /// presentation over the shared owner is one-column, so this strides one
    /// selectable row per item row.
    pub(super) fn move_by_item_rows(&mut self, item_rows: i64) -> usize {
        self.ensure_carrier();
        self.carrier.move_selection(item_rows);
        self.carrier.sync_viewport(self.painted_viewport_height());
        self.cursor()
    }

    /// Move by one selectable item in the shared owner's row order.
    pub(super) fn move_cursor_delta(&mut self, delta: i64) -> usize {
        self.ensure_carrier();
        self.carrier.delegate(RowLocalInput::Move(delta), None);
        if self.carrier.selected_target().is_some() {
            self.carrier.sync_viewport(self.painted_viewport_height());
        }
        self.cursor()
    }

    /// Home/End select the first/last target in the shared owner (the same
    /// row order the active presentation paints).
    pub(super) fn jump_cursor(&mut self, to_end: bool) -> usize {
        self.ensure_carrier();
        if to_end {
            self.carrier.select_last();
        } else {
            self.carrier.select_first();
        }
        if self.carrier.selected_target().is_some() {
            self.carrier.sync_viewport(self.painted_viewport_height());
        }
        self.cursor()
    }
}
