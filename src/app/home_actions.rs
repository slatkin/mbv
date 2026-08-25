use super::App;
use mbv_core::playback_queue::QueueItem;

impl App {
    // ── Home flat list ───────────────────────────────────────────────────────

    /// The QueueItem at the given flat `cursor` (the Home component's
    /// target index), or None. The caller supplies the cursor — the effect
    /// never consults `App::home.home_cursor`, so the component's target is
    /// honored even when the two differ (task 5.3d, Home typed-effect prep).
    pub(super) fn home_current_item(&self, cursor: usize) -> Option<QueueItem> {
        let mut pos = 0usize;
        for item in &self.home.continue_items {
            if pos == cursor {
                return Some(QueueItem::Emby(Box::new(item.clone())));
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
                return Some((pos, items.len()));
            }
            pos += items.len();
        }
        None
    }

    fn home_new_sections(&self) -> Vec<usize> {
        // Every section in `home.latest` is a selectable pill (an ABS library,
        // an Emby view, or Feeds), empty or not — matching Continue Watching.
        // An empty section renders as an "(empty)" row rather than vanishing.
        (0..self.home.latest.len()).map(|idx| idx + 1).collect()
    }

    /// Whether `section_idx` is a selectable Home pill: section 0 (Continue
    /// Watching) is always valid, and any other index is valid iff it maps to
    /// a section in `home.latest` (even an empty one).
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
        if let Some((start, len)) = self.home_section_range(section_idx) {
            self.home.home_cursor = if len == 0 {
                start
            } else {
                self.home.home_cursor.clamp(start, start + len - 1)
            };
        }
        // Persist the selection so the pill is restored on the next launch.
        self.save_prefs();
    }

    /// Play the item at the component-provided flat `cursor`. Uses the
    /// supplied target directly instead of `App::home.home_cursor`, so the
    /// request's own target is honored (task 5.3d, Home typed-effect prep).
    pub(super) fn home_play(&mut self, cursor: usize) {
        let Some(item) = self.home_current_item(cursor) else {
            return;
        };
        if matches!(&item, QueueItem::Emby(inner) if inner.is_folder) {
            return;
        }
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
            match item {
                QueueItem::Emby(item) => self.play_item(*item),
                // Non-Emby `latest` items (Audiobookshelf today, Feeds in
                // Part 3) submit through the shared helper, which neither
                // reads nor mutates the owning tab's cursor/filter.
                other => {
                    self.submit_queue_item(other, true);
                }
            }
        }
    }

    /// Enqueue the item at the component-provided flat `cursor` (task 5.3d,
    /// Home typed-effect prep). Uses the supplied target directly instead of
    /// `App::home.home_cursor`.
    pub(super) fn home_enqueue(&mut self, cursor: usize) {
        let cw_len = self.home.continue_items.len();
        if cursor < cw_len {
            let (saved_sec, saved_cursor) = (self.home.section, self.home.continue_cursor);
            self.home.section = 0;
            self.home.continue_cursor = cursor;
            self.enqueue_selected(None);
            self.home.section = saved_sec;
            self.home.continue_cursor = saved_cursor;
        } else {
            let Some(item) = self.home_current_item(cursor) else {
                return;
            };
            match item {
                QueueItem::Emby(item) => self.do_enqueue_folder(*item),
                other => {
                    self.submit_queue_item(other, false);
                }
            }
        }
    }

    /// Remove the Continue Watching item at the component-provided flat
    /// `cursor` from the resume row, guarding on the CW range exactly like the
    /// legacy `handle_cw_key` Delete arm. Non-CW cursors are ignored, and the
    /// CW column cursor is saved/restored around the removal just like the
    /// legacy arm (task 5.3d, Home typed-effect prep).
    pub(super) fn home_delete(&mut self, cursor: usize) {
        let cw_len = self.home.continue_items.len();
        if cursor >= cw_len {
            return;
        }
        let saved = self.home.continue_cursor;
        self.home.continue_cursor = cursor;
        self.remove_from_continue_watching();
        self.home.continue_cursor = saved;
    }
}
