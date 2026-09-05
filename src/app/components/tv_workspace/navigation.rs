use super::TvWorkspaceComponent;
use crate::app::ui_util::move_cursor;

impl TvWorkspaceComponent {
    pub(super) fn move_episode(&mut self, delta: i64) {
        self.episodes.move_selection(delta);
    }

    pub(super) fn move_season(&mut self, delta: i64) {
        let count = self
            .context
            .series_detail
            .as_ref()
            .map_or(0, |detail| detail.seasons.len());
        if count > 0 {
            self.season_cursor = move_cursor(self.season_cursor, delta, count);
            self.refresh_episode_rows();
            self.episodes.select_first();
        }
    }

    /// Move the series cursor by `rows` selectable rows. `WideMediaList` is
    /// the sole source of truth for the one-column rail — the legacy layout
    /// row map holds display-row indices (headings included), not selectable
    /// cursor indices, so consulting it made Down jump across grouped rows.
    pub(super) fn move_rows(&mut self, rows: i64) {
        if self.list.selectable_len() == 0 {
            return;
        }
        self.list.move_selection(rows);
        self.cursor = self.list.cursor();
    }

    pub(super) fn jump_cursor(&mut self, to_end: bool) {
        if to_end {
            self.list.select_last();
        } else {
            self.list.select_first();
        }
        self.cursor = self.list.cursor();
    }
}
