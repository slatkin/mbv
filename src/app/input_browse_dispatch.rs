use super::App;

impl App {
    /// Applies the single Series activation gate shared by keyboard Enter and
    /// browse double-click. Narrow presentations open the selection modal;
    /// wide presentations retain the persistent season/episode workspace.
    pub(super) fn activate_selected_series(&mut self, lib_idx: usize) -> bool {
        let Some(item) = self.selected_series_item(lib_idx) else {
            return false;
        };
        if self.layout.main.is_wide_tv_active() {
            self.enter_series_selection(lib_idx, &item);
        } else {
            self.open_series_selection_modal(&item);
        }
        true
    }
}
