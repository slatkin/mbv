use super::album_plan::{GroupedAlbumDisplayPlan, HeaderFocusCtx};
use crate::app::layout::LibraryRowTarget;
use crate::app::music_grouping::GroupedAlbumCatalog;
use crate::app::App;
use mbv_core::api::EmbyItem;

impl App {
    fn grouped_album_navigation_targets(plan: &GroupedAlbumDisplayPlan) -> Vec<LibraryRowTarget> {
        plan.rows
            .iter()
            .filter_map(|row| row.row_target())
            .collect()
    }

    /// Navigation targets for the settled catalog: every album in display
    /// order. Artist headers are display-only and not navigation targets.
    fn catalog_album_navigation_targets(
        catalog: &GroupedAlbumCatalog,
        albums_len: usize,
    ) -> Vec<LibraryRowTarget> {
        let mut targets = Vec::new();
        for group in &catalog.groups {
            for entry in &catalog.entries[group.start..group.end] {
                if entry.album_index < albums_len {
                    targets.push(LibraryRowTarget::Album(entry.album_index));
                }
            }
        }
        targets
    }

    /// Navigation targets for the current music album level, sourced from
    /// the settled catalog when available and a synchronous display plan
    /// otherwise. Callers must have already checked the level is non-empty.
    fn music_group_navigation(
        &mut self,
        lib_idx: usize,
        albums: &[EmbyItem],
        cursor: usize,
    ) -> Vec<LibraryRowTarget> {
        let catalog = self.libs[lib_idx]
            .nav_stack
            .last()
            .and_then(|l| l.music_grouping.as_ref())
            .and_then(|s| s.settled.as_ref());
        if let Some(cat) = catalog {
            return Self::catalog_album_navigation_targets(cat, albums.len());
        }
        let album_info = self.group_album_info(albums, None);
        let order = super::sorted_group_album_order(&album_info);
        let plan = self.build_grouped_album_display_plan(
            albums,
            &album_info,
            &order,
            cursor,
            false,
            HeaderFocusCtx {
                in_music_group_view: false,
                expand_selected: false,
            },
            None,
            false, // hero_handles_detail: cursor needs the full plan
        );
        Self::grouped_album_navigation_targets(&plan)
    }

    pub(in crate::app) fn move_music_group_display_cursor(
        &mut self,
        lib_idx: usize,
        delta: i64,
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
        let cursor = level.cursor;
        let albums = level.items.clone();
        let targets = self.music_group_navigation(lib_idx, &albums, cursor);
        if targets.is_empty() {
            return true;
        }
        let current_pos = targets
            .iter()
            .position(|target| matches!(target, LibraryRowTarget::Album(idx) if *idx == cursor))
            .unwrap_or(0);
        let new_pos = Self::grouped_cursor_target(&targets, current_pos, delta);
        let target = targets[new_pos].clone();
        match target {
            LibraryRowTarget::Album(idx) => {
                if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
                    if level.cursor != idx {
                        level.cursor = idx;
                        self.libs[lib_idx].album_track_focus = None;
                    }
                }
            }
            // `music_group_navigation` (Emby-only) never produces this.
            LibraryRowTarget::Book(_) => {}
        }
        true
    }

    /// Target position for a `delta`-item move within a grouped album
    /// list. `delta` is already column-scaled by the caller: `±cols` for
    /// vertical up/down moves (one rendered row) and `±1` for horizontal
    /// left/right moves (one album). With `cols == 1` the two coincide.
    fn grouped_cursor_target(targets: &[LibraryRowTarget], pos: usize, delta: i64) -> usize {
        let len = targets.len();
        if len == 0 {
            return pos;
        }
        crate::app::ui_util::move_cursor(pos, delta, len)
    }

    pub(in crate::app) fn jump_music_group_display_cursor(
        &mut self,
        lib_idx: usize,
        to_end: bool,
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
        let albums = level.items.clone();
        let targets = self.music_group_navigation(lib_idx, &albums, level.cursor);
        let Some(target) = (if to_end {
            targets.last().cloned()
        } else {
            targets.first().cloned()
        }) else {
            return true;
        };
        match target {
            LibraryRowTarget::Album(idx) => {
                if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
                    level.cursor = idx;
                    self.libs[lib_idx].album_track_focus = None;
                }
            }
            // `music_group_navigation` (Emby-only) never produces this.
            LibraryRowTarget::Book(_) => {}
        }
        true
    }

    pub(in crate::app) fn page_grouped_album_cursor(
        &mut self,
        lib_idx: usize,
        page_down: bool,
    ) -> bool {
        if self.tab.emby_library_index() != Some(lib_idx)
            || !matches!(self.panel_focus, crate::app::PanelFocus::Library)
            || self.libs[lib_idx].album_track_focus.is_some()
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

        let cursor = level.cursor;
        let albums = level.items.clone();
        let cols = self.current_library_columns(lib_idx);
        // A page is one viewport of *rows*, each holding `cols` albums.
        let page_rows = (self.layout.main.left_area.height as usize).max(1);
        let page = cols.max(1).saturating_mul(page_rows);
        let targets = self.music_group_navigation(lib_idx, &albums, cursor);
        if let Some(current_pos) = targets
            .iter()
            .position(|target| matches!(target, LibraryRowTarget::Album(idx) if *idx == cursor))
        {
            let target_pos = if page_down {
                current_pos.saturating_add(page).min(targets.len() - 1)
            } else {
                current_pos.saturating_sub(page)
            };
            let target = targets[target_pos].clone();
            match target {
                LibraryRowTarget::Album(new_cursor) => {
                    if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
                        if level.cursor != new_cursor {
                            level.cursor = new_cursor;
                            self.libs[lib_idx].album_track_focus = None;
                        }
                    }
                }
                // `music_group_navigation` (Emby-only) never produces this.
                LibraryRowTarget::Book(_) => {}
            }
        }
        if idle {
            self.maybe_fetch_next_page(lib_idx);
        }
        true
    }
}
