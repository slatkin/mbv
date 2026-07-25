use super::{App, PanelFocus};

impl App {
    /// Number of selectable left-panel tabs in Power View: Home/CW + all libraries.
    pub(super) fn library_tab_count(&self) -> usize {
        1 + self.libs.len()
    }

    /// Jump directly to left-panel tab `idx` (0 = Home, 1..=libs.len() =
    /// library index `idx - 1`), e.g. from a tab-bar click or a digit key.
    pub(super) fn set_library_tab(&mut self, idx: usize) {
        if idx >= self.library_tab_count() {
            return;
        }
        self.library_tab = idx;
        self.last_card_height = 0; // reset stale image height for new view
        if idx > 0 {
            self.set_panel_focus(PanelFocus::Library);
            self.activate_library_position(idx - 1);
        }
        self.ensure_tab_visible();
        self.save_prefs();
    }

    /// Advance the left-panel tab (wrapping); load the library if needed.
    pub(super) fn library_tab_next(&mut self) {
        let n = self.library_tab_count();
        self.library_tab = (self.library_tab + 1) % n;
        self.last_card_height = 0; // reset stale image height for new view
        if self.library_tab > 0 {
            self.set_panel_focus(PanelFocus::Library);
            self.activate_library_position(self.library_tab - 1);
        }
        self.ensure_tab_visible();
        self.save_prefs();
    }

    /// Retreat the left-panel tab (wrapping); load the library if needed.
    pub(super) fn library_tab_prev(&mut self) {
        let n = self.library_tab_count();
        self.library_tab = (self.library_tab + n - 1) % n;
        self.last_card_height = 0;
        if self.library_tab > 0 {
            self.set_panel_focus(PanelFocus::Library);
            self.activate_library_position(self.library_tab - 1);
        }
        self.ensure_tab_visible();
        self.save_prefs();
    }

    /// Move the cursor in the Continue Watching power column, clamped to its bounds.
    pub(super) fn power_cw_move_cursor(&mut self, delta: i64) {
        let n = self.home.continue_items.len();
        if n == 0 {
            return;
        }
        let cur = self.home.continue_cursor.min(n - 1) as i64;
        self.home.continue_cursor = (cur + delta).clamp(0, n as i64 - 1) as usize;
    }

    // The Continue Watching power column shares state with the Home tab's
    // Continue Watching section, so these reuse the Home actions by briefly
    // pointing the Home context at that section.
    pub(super) fn power_cw_play(&mut self) {
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

    pub(super) fn power_cw_enqueue(&mut self) {
        let saved_sec = self.home.section;
        self.home.section = 0;
        self.enqueue_selected();
        self.home.section = saved_sec;
    }

    pub(super) fn power_cw_toggle_watched(&mut self) {
        let saved_sec = self.home.section;
        self.home.section = 0;
        self.toggle_watched_home();
        self.home.section = saved_sec;
    }
}
