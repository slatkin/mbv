use crate::app::App;

impl App {
    pub(in crate::app) fn move_music_group_display_cursor(
        &mut self,
        lib_idx: usize,
        target: usize,
    ) -> bool {
        if !self.is_viewing_album_folders(lib_idx) {
            return false;
        }
        let Some(level) = self.libs[lib_idx].nav_stack.last() else {
            return true;
        };
        if level.items.is_empty() {
            return true;
        }
        if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
            let idx = target;
            if level.resting().cursor() != idx {
                level.set_resting_cursor(idx);
            }
        }
        true
    }

    pub(in crate::app) fn jump_music_group_display_cursor(
        &mut self,
        lib_idx: usize,
        target: usize,
    ) -> bool {
        if !self.is_music_group_view(lib_idx) {
            return false;
        }
        let Some(level) = self.libs[lib_idx].nav_stack.last() else {
            return true;
        };
        if level.items.is_empty() {
            return true;
        }
        if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
            let idx = target;
            level.set_resting_cursor(idx);
        }
        true
    }

    pub(in crate::app) fn page_grouped_album_cursor(
        &mut self,
        lib_idx: usize,
        target: usize,
    ) -> bool {
        if self.tab.emby_library_index() != Some(lib_idx)
            || !matches!(
                self.effective_panel_focus(),
                crate::app::PanelFocus::Library
            )
            || !self.is_viewing_album_folders(lib_idx)
        {
            return false;
        }

        let idle = self.list_image_fetches_allowed();
        let now = std::time::Instant::now();
        self.last_nav_at = now;
        self.mark_library_navigation(now);

        let Some(level) = self.libs[lib_idx].nav_stack.last() else {
            return false;
        };
        if level.items.is_empty() {
            return true;
        }

        if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
            let new_cursor = target;
            if level.resting().cursor() != new_cursor {
                level.set_resting_cursor(new_cursor);
            }
        }
        if idle {
            self.maybe_fetch_next_page(lib_idx, target);
        }
        true
    }
}
