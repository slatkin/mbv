use super::BrowserComponent;
use crate::app::components::media_list::ViewportAnchor;
use crate::app::library_column_width::library_column_count;
use crate::app::ui_util::move_cursor;

impl BrowserComponent {
    /// Return the column count used by the legacy two-column browse geometry.
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

    /// Painted item rows the pager moves per PageUp/PageDown. Canonical
    /// controls are one-column, while the legacy two-column path keeps its
    /// existing header exclusion and height-sensitive stride.
    pub(super) fn page_rows(&self) -> i64 {
        self.layout.left_area.height.saturating_sub(1).max(1) as i64
    }

    pub(super) fn uses_inline_control(&self) -> bool {
        !self.wide_movies
            && (self.narrow_extras.inline_hero.is_some() || self.narrow_extras.hero_placeholder)
    }

    fn has_active_control(&self) -> bool {
        self.wide_movies || self.uses_inline_control()
    }

    /// Resolve the active control's height-sensitive viewport and retain the
    /// resulting resting offset for the shell's navigation persistence seam.
    fn sync_active_viewport(&mut self) {
        let viewport_height = self.painted_viewport_height().max(1);
        if self.wide_movies {
            let offset = self.wide_list.resolve_viewport(viewport_height).offset;
            self.wide_list.set_scroll(offset);
            self.scroll = self.wide_list.scroll();
        } else if self.uses_inline_control() {
            let offset = self.inline_browser.resolve_viewport(viewport_height).offset;
            self.inline_browser.set_scroll(offset);
            self.scroll = self.inline_browser.scroll();
        }
    }

    /// Move the active persistent control and resolve its viewport immediately.
    /// The control returns a stable `context.items` index, which is the payload
    /// understood by `ShellRequest::BrowserCursorIndex`.
    fn move_active_selection(&mut self, delta: i64) -> Option<usize> {
        let target = if self.wide_movies {
            self.wide_list.move_selection(delta);
            self.wide_list.selected_target().copied()
        } else if self.uses_inline_control() {
            self.inline_browser.move_selection(delta);
            self.inline_browser.selected_target().copied()
        } else {
            return None;
        };
        if let Some(target) = target {
            self.cursor = target;
            self.sync_active_viewport();
        }
        target
    }

    /// Move by selectable order in the active canonical control. The method
    /// name is retained for the keyboard's item-row vocabulary; canonical
    /// controls are one-column, so one item row equals one selectable step.
    pub(super) fn move_sorted_cursor(&mut self, delta: i64) {
        let _ = self.move_active_selection(delta);
    }

    /// Move the component by displayed item rows. Canonical controls own the
    /// selectable order and viewport; legacy two-column lists reconstruct only
    /// their source order so their existing arrangement remains unchanged.
    pub(super) fn move_by_item_rows(&mut self, item_rows: i64) -> usize {
        if self.has_active_control() {
            self.move_sorted_cursor(item_rows);
        } else {
            self.move_legacy_item_rows(item_rows);
        }
        self.cursor
    }

    /// Move by one selectable item in the legacy source order.
    pub(super) fn move_cursor_delta(&mut self, delta: i64) -> usize {
        if self.has_active_control() {
            self.move_sorted_cursor(delta);
        } else if self.uses_letter_grouping() {
            let sorted = self.sorted_indices();
            if !sorted.is_empty() {
                self.cursor = move_cursor(
                    sorted
                        .iter()
                        .position(|&index| index == self.cursor)
                        .unwrap_or(0),
                    delta,
                    sorted.len(),
                );
                self.cursor = sorted[self.cursor];
            }
        } else {
            self.move_raw_cursor(delta);
        }
        self.cursor
    }

    /// Move the component cursor by `delta` in raw item order, clamped to the
    /// item count. This is retained for the non-grouped legacy two-column path.
    pub(super) fn move_raw_cursor(&mut self, delta: i64) {
        let count = self.context.item_count();
        if count > 0 {
            self.cursor = move_cursor(self.cursor, delta, count);
        }
    }

    fn uses_letter_grouping(&self) -> bool {
        !self.context.is_search_active()
            && (self.context.true_total() >= 50 || self.context.letter_filter.is_some())
    }

    fn sorted_indices(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = (0..self.context.items.len()).collect();
        indices.sort_by_cached_key(|&index| {
            crate::app::ui_util::natural_sort_key(crate::app::render::effective_sort_str(
                &self.context.items[index],
            ))
        });
        indices
    }

    /// Reconstruct the selectable item rows for the legacy two-column painter.
    /// Headings and spacers are deliberately omitted, matching its movement
    /// behavior without reading compatibility geometry from `LayoutMain`. This
    /// also preserves the selected column when moving vertically in a flat
    /// grid.
    fn legacy_item_rows(&self) -> Vec<Vec<usize>> {
        let sorted = if self.uses_letter_grouping() {
            self.sorted_indices()
        } else {
            (0..self.context.items.len()).collect()
        };
        let columns = self.columns().max(1);
        if !self.uses_letter_grouping() {
            return sorted.chunks(columns).map(|row| row.to_vec()).collect();
        }

        let bucket_total = if self.context.letter_filter.is_some() {
            usize::MAX
        } else {
            self.context.true_total()
        };
        let mut rows = Vec::new();
        let mut current_bucket = None;
        let mut current_row = Vec::with_capacity(columns);
        for index in sorted {
            let bucket =
                crate::app::render::letter_bucket(&self.context.items[index], bucket_total);
            if current_bucket.as_deref() != Some(bucket.as_str()) {
                if !current_row.is_empty() {
                    rows.push(std::mem::take(&mut current_row));
                }
                current_bucket = Some(bucket);
            }
            current_row.push(index);
            if current_row.len() == columns {
                rows.push(std::mem::take(&mut current_row));
            }
        }
        if !current_row.is_empty() {
            rows.push(current_row);
        }
        rows
    }

    fn move_legacy_item_rows(&mut self, item_rows: i64) {
        let rows = self.legacy_item_rows();
        let Some((row, col)) = rows.iter().enumerate().find_map(|(row, items)| {
            items
                .iter()
                .position(|&index| index == self.cursor)
                .map(|col| (row, col))
        }) else {
            return;
        };
        let target_row = if item_rows < 0 {
            row.saturating_sub(item_rows.unsigned_abs() as usize)
        } else {
            row.saturating_add(item_rows as usize)
                .min(rows.len().saturating_sub(1))
        };
        self.cursor = rows[target_row]
            .get(col)
            .copied()
            .or_else(|| rows[target_row].last().copied())
            .unwrap_or(self.cursor);
    }

    /// Home/End select the first/last target in the active control. Legacy
    /// grouped lists use the same natural sorted order as their painter.
    pub(super) fn jump_cursor(&mut self, to_end: bool) -> usize {
        if self.has_active_control() {
            if self.wide_movies {
                if to_end {
                    self.wide_list.select_last();
                } else {
                    self.wide_list.select_first();
                }
                if let Some(target) = self.wide_list.selected_target().copied() {
                    self.cursor = target;
                    self.sync_active_viewport();
                }
            } else {
                if to_end {
                    self.inline_browser.select_last();
                } else {
                    self.inline_browser.select_first();
                }
                if let Some(target) = self.inline_browser.selected_target().copied() {
                    self.cursor = target;
                    self.sync_active_viewport();
                }
            }
            return self.cursor;
        }
        if self.uses_letter_grouping() {
            let sorted = self.sorted_indices();
            if let Some(&target) = sorted.get(if to_end {
                sorted.len().saturating_sub(1)
            } else {
                0
            }) {
                self.cursor = target;
            }
        } else {
            let count = self.context.item_count();
            if count > 0 {
                self.cursor = if to_end { count - 1 } else { 0 };
            }
        }
        self.cursor
    }

    /// Resolve the active control's target/offset into the stable string
    /// anchor exchanged by the TV breakpoint seam.
    pub(super) fn active_viewport_anchor(
        &self,
        viewport_height: usize,
    ) -> Option<ViewportAnchor<String>> {
        let target = if self.wide_movies {
            self.wide_list.selected_target().copied()
        } else if self.uses_inline_control() {
            self.inline_browser.selected_target().copied()
        } else {
            None
        }?;
        let item = self.context.items.get(target)?;
        let selected_row_offset = if self.wide_movies {
            self.wide_list.selected_row_offset(viewport_height)?
        } else {
            self.inline_browser.selected_row_offset(viewport_height)?
        };
        Some(ViewportAnchor {
            selected_target: item.id.clone(),
            selected_row_offset,
        })
    }

    /// Apply a pending string anchor to the active control once the receiving
    /// viewport is known. Returns false for the legacy path or unavailable
    /// target, which keeps the old component-local fallback intact.
    pub(super) fn apply_active_viewport_anchor(
        &mut self,
        anchor: &ViewportAnchor<String>,
        viewport_height: usize,
    ) -> bool {
        let Some(target) = self
            .context
            .items
            .iter()
            .position(|item| item.id == anchor.selected_target)
        else {
            return false;
        };
        let control_anchor = ViewportAnchor {
            selected_target: target,
            selected_row_offset: anchor.selected_row_offset,
        };
        if self.wide_movies {
            self.wide_list
                .apply_viewport_anchor(&control_anchor, viewport_height);
            if self.wide_list.selected_target() != Some(&target) {
                return false;
            }
            self.cursor = target;
            self.scroll = self.wide_list.scroll();
            true
        } else if self.uses_inline_control() {
            self.inline_browser
                .apply_viewport_anchor(&control_anchor, viewport_height);
            if self.inline_browser.selected_target() != Some(&target) {
                return false;
            }
            self.cursor = target;
            self.scroll = self.inline_browser.scroll();
            true
        } else {
            false
        }
    }
}
