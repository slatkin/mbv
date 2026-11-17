// The Shows destination's actions (add-gui-library-panel design D14;
// gui-collection-dropdown design D8): the select, play, play-next, and add
// commands, and the item resolution they share with the projection. The
// projection itself lives in `shows_projection.rs`, as the other destinations
// split their data path from their actions. The GUI Shows list is the series
// list alone, so a select names a series.

use crate::app::App;

impl App {
    /// Select a Shows series (design D12, D8): the Hero's series, the season
    /// reset, and the detail load. A stale frame's row — no series of the
    /// library's root list — is an ignored command, so a stale select never
    /// replaces a good selection.
    pub(in crate::app::gui_library) fn select_gui_shows_row(&mut self, lib_idx: usize, row: &str) {
        if !self.gui_tv_library(lib_idx) {
            Self::log_ignored_library_command(
                "showsSelect",
                format_args!("the destination is no Shows library"),
            );
            return;
        }
        let unchanged = self
            .gui_tv_shows
            .get(&lib_idx)
            .and_then(|selection| selection.series_id.as_deref())
            == Some(row);
        if !unchanged {
            // A pick move is library navigation: it closes the image warm
            // gate, so fast arrow traffic warms no neighbour art (the TUI's
            // own rule).
            self.stamp_navigation();
        }
        self.gui_tv_select_series(lib_idx, row);
    }

    /// Play the Shows episode a command names (design D12): the shared
    /// library play path, whose admission and route checks are the Owner's
    /// queue's own. A stale id plays nothing.
    pub(in crate::app::gui_library) fn play_gui_shows_item(
        &mut self,
        lib_idx: usize,
        item_id: &str,
    ) {
        match self.gui_shows_item(lib_idx, item_id) {
            Some(item) => self.play_or_activate_lib_item(lib_idx, item),
            None => Self::log_stale_library_item("showsPlay", item_id),
        }
    }

    /// Play the Shows episode next (design D12): the shared play-next
    /// admission, so the episode enters directly after the active slot.
    pub(in crate::app::gui_library) fn play_next_gui_shows_item(
        &mut self,
        lib_idx: usize,
        item_id: &str,
    ) {
        match self.gui_shows_item(lib_idx, item_id) {
            Some(item) => self.play_lib_item_next(lib_idx, item),
            None => Self::log_stale_library_item("showsPlayNext", item_id),
        }
    }

    /// Add the Shows episode to the queue (design D12): the shared add path.
    pub(in crate::app::gui_library) fn enqueue_gui_shows_item(
        &mut self,
        lib_idx: usize,
        item_id: &str,
    ) {
        match self.gui_shows_item(lib_idx, item_id) {
            Some(item) => self.enqueue_lib_item(lib_idx, item),
            None => Self::log_stale_library_item("showsQueue", item_id),
        }
    }
}
