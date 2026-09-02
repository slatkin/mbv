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

    /// Move through the rows painted by the last frame. Grouped TV lists
    /// publish their sorted order and row map just like BrowserComponent;
    /// using raw item indices here would make the local cursor disagree with
    /// the App's letter-group navigation.
    pub(super) fn move_rows(&mut self, rows: i64) {
        // WideMediaList is the sole source of truth for the one-column rail.
        // The legacy layout row map contains display-row indices (including
        // headings), not selectable cursor indices; consulting it made Down
        // jump across grouped rows.
        self.move_cursor_delta(rows);
    }

    pub(super) fn move_cursor_delta(&mut self, delta: i64) {
        let count = self.list.selectable_len();
        if count == 0 {
            return;
        }
        self.list.move_selection(delta);
        self.cursor = self.list.cursor();
    }

    pub(super) fn letter_vertical_delta(&self, rows: i64) -> Option<i64> {
        let item_rows: Vec<&Vec<usize>> = self
            .layout
            .left_item_rows
            .iter()
            .filter(|row| !row.is_empty())
            .collect();
        if item_rows.is_empty() || self.layout.left_sorted_indices.is_empty() {
            return None;
        }
        let (current_row, current_col) =
            item_rows.iter().enumerate().find_map(|(row, items)| {
                items
                    .iter()
                    .position(|&index| index == self.cursor)
                    .map(|col| (row, col))
            })?;
        let target_row = if rows < 0 {
            current_row.saturating_sub(rows.unsigned_abs() as usize)
        } else {
            current_row
                .saturating_add(rows as usize)
                .min(item_rows.len().saturating_sub(1))
        };
        let target = item_rows[target_row]
            .get(current_col)
            .copied()
            .or_else(|| item_rows[target_row].last().copied())?;
        let sorted = &self.layout.left_sorted_indices;
        let current_position = sorted.iter().position(|&index| index == self.cursor)?;
        let target_position = sorted.iter().position(|&index| index == target)?;
        Some(target_position as i64 - current_position as i64)
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
