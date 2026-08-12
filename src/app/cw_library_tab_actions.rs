use super::{App, PanelFocus, TabSelection};

impl App {
    /// Number of selectable left-panel tabs: Home/CW + all libraries + Feeds
    /// (when subscriptions exist).
    pub(super) fn library_tab_count(&self) -> usize {
        self.tab_count()
    }

    /// Jump directly to left-panel tab `idx` (0 = Home, 1..=libs.len() =
    /// library index `idx - 1`, or Feeds at the end when present).
    pub(super) fn set_library_tab(&mut self, idx: usize) {
        if idx >= self.library_tab_count() {
            return;
        }
        self.tab = TabSelection::from_position_with_counts(
            idx,
            self.libs.len(),
            self.audiobookshelf_libraries.len(),
            self.has_feeds_subscriptions(),
        );
        self.last_card_height = 0; // reset stale image height for new view
        self.last_card_width = 0;
        if self.tab.is_feeds() {
            self.set_panel_focus(PanelFocus::Library);
        } else if self.tab.is_audiobookshelf() {
            self.set_panel_focus(PanelFocus::Library);
            self.select_audiobookshelf_show(0);
        } else if let Some(lib_idx) = self.tab.library_index() {
            self.set_panel_focus(PanelFocus::Library);
            self.activate_library_position(lib_idx);
        }
        self.ensure_tab_visible();
        self.save_prefs();
    }

    /// Advance the left-panel tab (wrapping); load the library if needed.
    pub(super) fn library_tab_next(&mut self) {
        let n = self.library_tab_count();
        let pos = self
            .tab
            .to_position_with_counts(self.libs.len(), self.feeds_tab_pos());
        let new_pos = (pos + 1) % n;
        self.tab = TabSelection::from_position_with_counts(
            new_pos,
            self.libs.len(),
            self.audiobookshelf_libraries.len(),
            self.has_feeds_subscriptions(),
        );
        self.last_card_height = 0; // reset stale image height for new view
        self.last_card_width = 0;
        if self.tab.is_feeds() {
            self.set_panel_focus(PanelFocus::Library);
        } else if self.tab.is_audiobookshelf() {
            self.set_panel_focus(PanelFocus::Library);
            self.select_audiobookshelf_show(0);
        } else if let Some(lib_idx) = self.tab.library_index() {
            self.set_panel_focus(PanelFocus::Library);
            self.activate_library_position(lib_idx);
        }
        self.ensure_tab_visible();
        self.save_prefs();
    }

    /// Retreat the left-panel tab (wrapping); load the library if needed.
    pub(super) fn library_tab_prev(&mut self) {
        let n = self.library_tab_count();
        let pos = self
            .tab
            .to_position_with_counts(self.libs.len(), self.feeds_tab_pos());
        let new_pos = (pos + n - 1) % n;
        self.tab = TabSelection::from_position_with_counts(
            new_pos,
            self.libs.len(),
            self.audiobookshelf_libraries.len(),
            self.has_feeds_subscriptions(),
        );
        self.last_card_height = 0;
        self.last_card_width = 0;
        if self.tab.is_feeds() {
            self.set_panel_focus(PanelFocus::Library);
        } else if self.tab.is_audiobookshelf() {
            self.set_panel_focus(PanelFocus::Library);
            self.select_audiobookshelf_show(0);
        } else if let Some(lib_idx) = self.tab.library_index() {
            self.set_panel_focus(PanelFocus::Library);
            self.activate_library_position(lib_idx);
        }
        self.ensure_tab_visible();
        self.save_prefs();
    }

    /// Move the cursor in the Continue Watching column, clamped to its bounds.
    pub(super) fn cw_move_cursor(&mut self, delta: i64) {
        let n = self.home.continue_items.len();
        if n == 0 {
            return;
        }
        let cur = self.home.continue_cursor.min(n - 1) as i64;
        self.home.continue_cursor = (cur + delta).clamp(0, n as i64 - 1) as usize;
    }

    // The Continue Watching column shares state with the Home tab's
    // Continue Watching section, so these reuse the Home actions by briefly
    // pointing the Home context at that section.
    pub(super) fn cw_play(&mut self) {
        let Some(item) = self
            .home
            .continue_items
            .get(self.home.continue_cursor)
            .cloned()
        else {
            return;
        };
        if item.is_folder {
            return;
        }
        let saved_sec = self.home.section;
        self.home.section = 0;
        self.select_home();
        self.home.section = saved_sec;
    }

    pub(super) fn cw_enqueue(&mut self) {
        let saved_sec = self.home.section;
        self.home.section = 0;
        self.enqueue_selected();
        self.home.section = saved_sec;
    }

    pub(super) fn cw_toggle_watched(&mut self) {
        let saved_sec = self.home.section;
        self.home.section = 0;
        self.toggle_watched_home();
        self.home.section = saved_sec;
    }
}
