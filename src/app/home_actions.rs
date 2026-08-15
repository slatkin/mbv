use super::App;
use mbv_core::api::EmbyItem;

impl App {
    // ── Home flat list ───────────────────────────────────────────────────────

    /// The EmbyItem at the current flat `home_cursor`, or None.
    pub(super) fn home_current_item(&self) -> Option<EmbyItem> {
        let cursor = self.home.home_cursor;
        let mut pos = 0usize;
        for item in &self.home.continue_items {
            if pos == cursor {
                return Some(item.clone());
            }
            pos += 1;
        }
        for (_, _, items, _) in &self.home.latest {
            for item in items {
                if pos == cursor {
                    return Some(item.clone());
                }
                pos += 1;
            }
        }
        None
    }

    /// Flat cursor range for a home section. Section 0 is Keep Watching;
    /// non-empty latest sections keep their regular Home section index.
    fn home_section_range(&self, section_idx: usize) -> Option<(usize, usize)> {
        let mut pos = 0usize;
        if section_idx == 0 {
            return Some((0, self.home.continue_items.len()));
        }
        pos += self.home.continue_items.len();
        for (idx, (_, _, items, _)) in self.home.latest.iter().enumerate() {
            let current_section = idx + 1;
            if current_section == section_idx {
                return if items.is_empty() {
                    None
                } else {
                    Some((pos, items.len()))
                };
            }
            pos += items.len();
        }
        None
    }

    fn home_new_sections(&self) -> Vec<usize> {
        let mut sections = Vec::new();
        for (idx, (_, _, items, _)) in self.home.latest.iter().enumerate() {
            if !items.is_empty() {
                sections.push(idx + 1);
            }
        }
        sections
    }

    /// Whether `section_idx` is a selectable Home pill: section 0 (Continue
    /// Watching) is always valid, and any other index is valid iff it has a
    /// non-empty Newest section.
    pub(super) fn home_section_is_valid(&self, section_idx: usize) -> bool {
        section_idx == 0 || self.home_new_sections().contains(&section_idx)
    }

    pub(super) fn home_select_section(&mut self, section_idx: usize) {
        let section_idx = if self.home_section_is_valid(section_idx) {
            section_idx
        } else if let Some(first) = self.home_new_sections().first() {
            *first
        } else {
            self.home.section = 0;
            return;
        };
        self.home.section = section_idx;
        self.home.home_scroll = 0;
        if let Some((start, len)) = self.home_section_range(section_idx) {
            self.home.home_cursor = if len == 0 {
                start
            } else {
                self.home.home_cursor.clamp(start, start + len - 1)
            };
        }
    }

    fn home_visible_indices(&self) -> Vec<usize> {
        let mut indices = Vec::new();
        let selected = if self.home_section_is_valid(self.home.section) {
            self.home.section
        } else {
            self.home_new_sections().first().copied().unwrap_or(0)
        };
        if let Some((start, len)) = self.home_section_range(selected) {
            indices.extend(start..start + len);
        }
        indices
    }

    /// Move the flat home cursor by `delta`, clamped to the selected home
    /// section.
    pub(super) fn home_move_cursor(&mut self, delta: i64) {
        let indices = self.home_visible_indices();
        if indices.is_empty() {
            self.home.home_cursor = 0;
            return;
        };
        let pos = indices
            .iter()
            .position(|idx| *idx == self.home.home_cursor)
            .unwrap_or(0);
        let next = super::ui_util::move_cursor(pos, delta, indices.len());
        self.home.home_cursor = indices[next];
    }

    pub(super) fn home_select_start(&mut self) {
        if let Some(first) = self.home_visible_indices().first() {
            self.home.home_cursor = *first;
        }
    }

    pub(super) fn home_select_end(&mut self) {
        if let Some(last) = self.home_visible_indices().last() {
            self.home.home_cursor = *last;
        }
    }

    pub(super) fn home_move_down(&mut self) {
        self.home_move_cursor(1);
    }

    pub(super) fn home_move_up(&mut self) {
        self.home_move_cursor(-1);
    }

    /// Cycle the selected home section, wrapping at the ends. `dir` = -1 previous,
    /// +1 next.
    pub(super) fn home_move_section(&mut self, dir: i64) {
        let mut sections = vec![0];
        sections.extend(self.home_new_sections());
        let pos = sections
            .iter()
            .position(|&section_idx| section_idx == self.home.section);
        let next_pos = match pos {
            Some(p) => {
                let n = sections.len() as i64;
                (((p as i64 + dir) % n + n) % n) as usize
            }
            None => 0,
        };
        self.home_select_section(sections[next_pos]);
    }

    /// Play the item under the flat home cursor.
    pub(super) fn home_play(&mut self) {
        let Some(item) = self.home_current_item() else {
            return;
        };
        if item.is_folder {
            return;
        }
        let cursor = self.home.home_cursor;
        let cw_len = self.home.continue_items.len();
        if cursor < cw_len {
            // CW items: use select_home for proper resume handling.
            let (saved_sec, saved_cursor) = (self.home.section, self.home.continue_cursor);
            self.home.section = 0;
            self.home.continue_cursor = cursor;
            self.select_home();
            self.home.section = saved_sec;
            self.home.continue_cursor = saved_cursor;
        } else {
            self.play_item(item);
        }
    }

    /// Enqueue the item under the flat home cursor.
    pub(super) fn home_enqueue(&mut self) {
        let cursor = self.home.home_cursor;
        let cw_len = self.home.continue_items.len();
        if cursor < cw_len {
            let (saved_sec, saved_cursor) = (self.home.section, self.home.continue_cursor);
            self.home.section = 0;
            self.home.continue_cursor = cursor;
            self.enqueue_selected(None);
            self.home.section = saved_sec;
            self.home.continue_cursor = saved_cursor;
        } else {
            let Some(item) = self.home_current_item() else {
                return;
            };
            self.do_enqueue_folder(item);
        }
    }
}
