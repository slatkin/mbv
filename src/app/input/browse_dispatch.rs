use mbv_emby_model::EmbyItem;

use crate::app::App;

impl App {
    /// Applies the single Series activation gate shared by keyboard Enter and
    /// browse double-click. Non-Wide activation opens the Library Hero
    /// overlay; Wide activation retains the persistent season/episode workspace.
    ///
    /// Resolves the target via `selected_series_item`, then delegates the
    /// wide/narrow branch to `activate_selected_series_item`.
    pub(in crate::app) fn activate_selected_series(&mut self, lib_idx: usize) -> bool {
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
    /// The wide gate reads the Client's write-back fact
    /// [`App::tv_workspace_wide`]: the breakpoint is terminal geometry, so the
    /// core cannot compute it (design D2). A Client that paints no such
    /// Workspace leaves the set empty, which keeps this branch narrow.
    ///
    /// Keeps the non-Series guard (`enter_series_selection` also enforces
    /// `item_type == "Series"` and a non-empty id) and the wide-fetch vs
    /// narrow-modal branch.
    pub(in crate::app) fn activate_selected_series_item(
        &mut self,
        lib_idx: usize,
        item: &EmbyItem,
    ) -> bool {
        if item.item_type != "Series" || item.id.is_empty() {
            return false;
        }
        if self.tv_workspace_wide.contains(&lib_idx) {
            self.enter_series_selection(lib_idx, item);
        }
        // Non-Wide activation is handled by the mounted Library panel, whose
        // owner can open the Hero overlay without constructing a modal.
        true
    }
}
