use super::BrowserComponent;
use crate::app::library_column_width::library_column_count;
use crate::app::ui_util::move_cursor;

impl BrowserComponent {
    /// Return the column count used by the painted browse geometry.
    pub(super) fn columns(&self) -> usize {
        if self.wide_movies
            || self.narrow_extras.inline_hero.is_some()
            || self.narrow_extras.hero_placeholder
        {
            1
        } else {
            library_column_count(self.layout.left_area.width)
        }
    }

    /// Painted item rows the pager moves per PageUp/PageDown, mirroring
    /// `App::lib_page_size`: the painted list area's height minus its top
    /// count/search header line, floored at one row (list rows are
    /// single-line).
    pub(super) fn page_rows(&self) -> i64 {
        self.layout.left_area.height.saturating_sub(1).max(1) as i64
    }

    /// Move the component-local cursor `item_rows` displayed item rows down
    /// (positive) or up (negative), matching the former legacy row movement for
    /// the generic/Movies/home-video browser (task 5.3d prep): letter-
    /// grouped lists resolve the target through the last painted
    /// `left_item_rows`/`left_sorted_indices` (headers/gaps skipped, a
    /// ragged target row falls back to its last item), and flat lists stride
    /// by the painted column count. The legacy stale-layout fallback
    /// (sorted present but cursor unpainted) moves in sorted order by the
    /// multiplied delta, using the same displayed-row rules.
    pub(super) fn move_rows(&mut self, item_rows: i64) -> usize {
        if !self.layout.left_sorted_indices.is_empty() {
            if let Some(delta) = self.letter_vertical_delta(item_rows) {
                return self.move_cursor_delta(delta);
            }
        }
        self.move_cursor_delta(item_rows * self.columns() as i64)
    }

    /// Move the component-local cursor by `delta` items, using sorted display order when the last painted
    /// list is letter-grouped, raw item order otherwise.
    pub(super) fn move_cursor_delta(&mut self, delta: i64) -> usize {
        if !self.layout.left_sorted_indices.is_empty() {
            self.move_sorted_cursor(delta);
        } else {
            self.move_raw_cursor(delta);
        }
        self.cursor
    }

    /// Move in the letter-grouped display order: the cursor's position in
    /// `left_sorted_indices` is the authority, according to the painted sorted indices.
    pub(super) fn move_sorted_cursor(&mut self, delta: i64) {
        let sorted = &self.layout.left_sorted_indices;
        if sorted.is_empty() {
            return;
        }
        let pos = sorted.iter().position(|&i| i == self.cursor).unwrap_or(0);
        let new_pos = move_cursor(pos, delta, sorted.len());
        self.cursor = sorted[new_pos];
    }

    /// Move the component cursor by `delta` in raw item order, clamped to the item count.
    pub(super) fn move_raw_cursor(&mut self, delta: i64) {
        let count = self.context.item_count();
        if count > 0 {
            self.cursor = move_cursor(self.cursor, delta, count);
        }
    }

    /// Flat (sorted-order) delta to the item `item_rows` rows up/down from
    /// the component cursor in the last painted item rows. Headers/spacers/
    /// fillers do not participate; ragged rows fall back to their last item.
    /// Returns `None` when the layout is stale, so callers use flat arithmetic.
    pub(super) fn letter_vertical_delta(&self, item_rows: i64) -> Option<i64> {
        let all_rows = &self.layout.left_item_rows;
        if all_rows.is_empty() || self.layout.left_sorted_indices.is_empty() {
            return None;
        }
        let item_row_list: Vec<&Vec<usize>> = all_rows.iter().filter(|r| !r.is_empty()).collect();
        if item_row_list.is_empty() {
            return None;
        }
        let (cur_row, cur_col) = item_row_list.iter().enumerate().find_map(|(r, row)| {
            row.iter()
                .position(|&i| i == self.cursor)
                .map(|col| (r, col))
        })?;
        let row_count = item_row_list.len();
        let target_row = if item_rows < 0 {
            cur_row.saturating_sub(item_rows.unsigned_abs() as usize)
        } else {
            cur_row
                .saturating_add(item_rows as usize)
                .min(row_count.saturating_sub(1))
        };
        let target = item_row_list[target_row]
            .get(cur_col)
            .copied()
            .or_else(|| item_row_list[target_row].last().copied())?;

        // Single pass over `sorted` for both positions instead of two
        // separate `.position()` scans — this runs on every j/k/Up/Down
        // keypress in letter-grouped view, so halving the work (and
        // early-exiting once both are found) matters on large libraries.
        let mut cur_pos = None;
        let mut target_pos = None;
        for (pos, &idx) in self.layout.left_sorted_indices.iter().enumerate() {
            if idx == self.cursor {
                cur_pos = Some(pos);
            }
            if idx == target {
                target_pos = Some(pos);
            }
            if cur_pos.is_some() && target_pos.is_some() {
                break;
            }
        }
        Some(target_pos? as i64 - cur_pos? as i64)
    }

    /// Home/End jump to the first/last item in sorted display order when
    /// the last painted list is letter-grouped, else the raw first/last.
    pub(super) fn jump_cursor(&mut self, to_end: bool) -> usize {
        if !self.layout.left_sorted_indices.is_empty() {
            let n = self.layout.left_sorted_indices.len();
            self.cursor = self.layout.left_sorted_indices[if to_end { n - 1 } else { 0 }];
        } else {
            let count = self.context.item_count();
            if count > 0 {
                self.cursor = if to_end { count - 1 } else { 0 };
            }
        }
        self.cursor
    }
}
