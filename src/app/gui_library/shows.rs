// The Shows destination's actions (add-gui-library-panel design D14): the
// select, play, play-next, and add commands, and the item resolution they
// share with the projection. The projection itself lives in
// `shows_projection.rs`, as the other destinations split their data path
// from their actions (`emby.rs` and `emby_projection.rs`).

use super::shows_projection::{GuiShowsMode, gui_shows_mode_of};
use crate::app::App;

impl App {
    /// Select a Shows row (design D12): the All series mode's select is the
    /// series select — the Hero's series, the season reset, and the detail
    /// load — and a flat mode's select stores the row the Hero shows. A
    /// stale frame's row — no row of the active mode's list — is an ignored
    /// command, so a stale select never replaces a good selection.
    pub(in crate::app::gui_library) fn select_gui_shows_row(&mut self, lib_idx: usize, row: &str) {
        if !self.gui_tv_library(lib_idx) {
            Self::log_ignored_library_command(
                "showsSelect",
                format_args!("the destination is no Shows library"),
            );
            return;
        }
        let Some(tv_mode) = self.gui_tv_mode(lib_idx) else {
            Self::log_ignored_library_command(
                "showsSelect",
                format_args!("the destination holds no view mode"),
            );
            return;
        };
        match gui_shows_mode_of(&tv_mode) {
            GuiShowsMode::All => {
                let unchanged = self
                    .gui_tv_shows
                    .get(&lib_idx)
                    .and_then(|selection| selection.series_id.as_deref())
                    == Some(row);
                if !unchanged {
                    // A pick move is library navigation: it closes the image
                    // warm gate, so fast arrow traffic warms no neighbour art
                    // (the TUI's own rule).
                    self.stamp_navigation();
                }
                self.gui_tv_select_series(lib_idx, row);
            }
            GuiShowsMode::Latest | GuiShowsMode::Upcoming => {
                if !self.gui_shows_flat_row_exists(lib_idx, row) {
                    Self::log_stale_library_item("showsSelect", row);
                    return;
                }
                let unchanged = self
                    .gui_tv_shows
                    .get(&lib_idx)
                    .and_then(|selection| selection.flat_row.as_deref())
                    == Some(row);
                if unchanged {
                    return;
                }
                self.gui_tv_shows.entry(lib_idx).or_default().flat_row = Some(row.to_string());
                self.stamp_navigation();
            }
        }
    }

    /// Whether the flat mode's list carries `row`: the root level's episode
    /// ids in the Latest mode, the snapshot's premiere keys in the Upcoming
    /// mode.
    fn gui_shows_flat_row_exists(&self, lib_idx: usize, row: &str) -> bool {
        if let Some(level) = self.libs.get(lib_idx).and_then(|lib| lib.nav_stack.last())
            && level.items.iter().any(|item| item.id == row)
        {
            return true;
        }
        let library_id = &self.libs[lib_idx].library.id;
        self.tv_upcoming_snapshots
            .get(library_id)
            .is_some_and(|premieres| {
                premieres
                    .iter()
                    .any(|premiere| premiere.stable_key() == row)
            })
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
