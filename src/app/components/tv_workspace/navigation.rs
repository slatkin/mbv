use super::TvWorkspaceComponent;
use crate::app::ui_util::move_cursor;

impl TvWorkspaceComponent {
    /// Entering the Episodes pane by keyboard starts at the first available
    /// episode, while retaining an existing local selection.
    pub(super) fn ensure_episode_cursor(&mut self) {
        if self.episode_cursor.is_some() {
            return;
        }
        let has_episode = self
            .context
            .series_detail
            .as_ref()
            .and_then(|detail| detail.seasons.get(self.season_cursor))
            .and_then(|season| {
                self.context
                    .series_detail
                    .as_ref()?
                    .episodes
                    .get(&season.id)
            })
            .is_some_and(|episodes| !episodes.is_empty());
        if has_episode {
            self.episode_cursor = Some(0);
        }
    }

    pub(super) fn move_episode(&mut self, delta: i64) {
        let count = self
            .context
            .series_detail
            .as_ref()
            .and_then(|detail| detail.seasons.get(self.season_cursor))
            .and_then(|season| {
                self.context
                    .series_detail
                    .as_ref()?
                    .episodes
                    .get(&season.id)
            })
            .map_or(0, Vec::len);
        if count > 0 {
            let cursor = self.episode_cursor.unwrap_or(0);
            self.episode_cursor = Some(move_cursor(cursor, delta, count));
        }
    }

    pub(super) fn move_season(&mut self, delta: i64) {
        let count = self
            .context
            .series_detail
            .as_ref()
            .map_or(0, |detail| detail.seasons.len());
        if count > 0 {
            self.season_cursor = move_cursor(self.season_cursor, delta, count);
            self.episode_cursor = Some(0);
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
