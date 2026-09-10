use super::TvWorkspaceComponent;
use crate::app::components::media_list::RowLocalInput;

impl TvWorkspaceComponent {
    pub(super) fn move_episode(&mut self, delta: i64) {
        self.episodes.delegate(RowLocalInput::Move(delta), None);
    }

    pub(super) fn move_season(&mut self, delta: i64) {
        let count = self
            .context
            .series_detail
            .as_ref()
            .map_or(0, |detail| detail.seasons.len());
        if count > 0 {
            self.season_cursor =
                (self.season_cursor as i64 + delta).rem_euclid(count as i64) as usize;
            self.refresh_episode_rows();
            self.episodes.select_first();
        }
    }

    /// Move the series cursor by `rows` selectable rows through the common
    /// delegation seam. `WideMediaList` is the sole source of truth for the
    /// one-column rail -- the legacy layout row map holds display-row indices
    /// (headings included), not selectable cursor indices, so consulting it
    /// made Down jump across grouped rows.
    pub(super) fn move_rows(&mut self, rows: i64) {
        self.list.delegate(RowLocalInput::Move(rows), None);
    }

    pub(super) fn jump_cursor(&mut self, to_end: bool) {
        let input = if to_end {
            RowLocalInput::Last
        } else {
            RowLocalInput::First
        };
        self.list.delegate(input, None);
    }
}
