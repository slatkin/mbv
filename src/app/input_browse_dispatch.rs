use mbv_core::api::EmbyItem;

use super::App;

impl App {
    /// Applies the single Series activation gate shared by keyboard Enter and
    /// browse double-click. Narrow presentations open the selection modal;
    /// wide presentations retain the persistent season/episode workspace.
    ///
    /// Resolves the target via `selected_series_item`, then delegates the
    /// wide/narrow branch to `activate_selected_series_item`.
    pub(super) fn activate_selected_series(&mut self, lib_idx: usize) -> bool {
        let cursor = self.libs[lib_idx]
            .nav_stack
            .last()
            .map_or(0, |level| level.resting().cursor());
        let Some(item) = self.selected_series_item(lib_idx, cursor) else {
            return false;
        };
        self.activate_selected_series_item(lib_idx, &item)
    }

    /// Item-targeted Series activation: the caller supplies the resolved
    /// Series item instead of an App browse cursor, along with the library
    /// index the wide/narrow gate should check.
    ///
    /// Keeps the non-Series guard (`enter_series_selection` also enforces
    /// `item_type == "Series"` and a non-empty id) and the wide-fetch vs
    /// narrow-modal branch.
    pub(super) fn activate_selected_series_item(
        &mut self,
        lib_idx: usize,
        item: &EmbyItem,
    ) -> bool {
        if item.item_type != "Series" || item.id.is_empty() {
            return false;
        }
        if self.wide_tv_library_area(lib_idx).is_some() {
            self.enter_series_selection(lib_idx, item);
        } else {
            self.open_series_selection_modal(item);
        }
        true
    }
}
