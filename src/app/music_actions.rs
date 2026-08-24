use super::{App, BrowseLevel};

impl App {
    /// True when mbv should show the combined music group view:
    /// a group-selector bar at top with the album list below.
    /// Activated when `music.levels` starts with `"group"` and the nav stack
    /// has a group level plus an album level above it.
    pub(super) fn is_music_group_view(&self, lib_idx: usize) -> bool {
        let lib = &self.libs[lib_idx];
        if lib.library.collection_type != "music" {
            return false;
        }
        // Only when the first configured level is "group".
        if self
            .music_levels
            .first()
            .map(|s| s != "group")
            .unwrap_or(true)
        {
            return false;
        }
        // Need at least a group level and an album level on the stack.
        if lib.nav_stack.len() < 2 {
            return false;
        }
        // The top nav level must be the album-folder level.
        self.is_viewing_album_folders(lib_idx)
    }

    /// Switch to the previous (`delta == -1`) or next (`delta == 1`) group
    /// while in the combined music group view. Pops the current album level,
    /// adjusts the group cursor (wraps around), then kicks off a fetch for
    /// the new group's albums.
    pub(super) fn switch_music_group(&mut self, lib_idx: usize, delta: i64) {
        let stack_len = self.libs[lib_idx].nav_stack.len();
        if stack_len < 2 {
            return;
        }

        // Verify count before popping so we never lose the album level.
        let n = self.libs[lib_idx].nav_stack[stack_len - 2].items.len();
        if n == 0 {
            return;
        }

        self.libs[lib_idx].clear_music_focus();

        // Pop the album level.
        self.libs[lib_idx].nav_stack.pop();
        let cur = self.libs[lib_idx]
            .nav_stack
            .last()
            .map(|l| l.cursor)
            .unwrap_or(0);
        // Wrap-around navigation (unlike seasons which clamp).
        let new_cursor = (cur as i64 + delta).rem_euclid(n as i64) as usize;
        if let Some(group_lvl) = self.libs[lib_idx].nav_stack.last_mut() {
            group_lvl.cursor = new_cursor;
        }

        // Collect new group's identity.
        let (group_id, group_name) = self.libs[lib_idx]
            .nav_stack
            .last()
            .and_then(|l| l.items.get(new_cursor))
            .map(|g| (g.id.clone(), g.name.clone()))
            .unwrap_or_default();
        if group_id.is_empty() {
            return;
        }

        // Push a loading placeholder so the Loaded handler can fill it in.
        self.libs[lib_idx].nav_stack.push(BrowseLevel {
            parent_id: group_id.clone(),
            title: group_name.clone(),
            items: vec![],
            total_count: 0,
            cursor: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: true,
            scroll: 0,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        });
        self.spawn_browse(
            lib_idx,
            group_id,
            group_name,
            None,
            false,
            "SortName".into(),
            "Ascending".into(),
        );
    }

    pub(super) fn select_music_group(&mut self, lib_idx: usize, group_cursor: usize) {
        let stack_len = self.libs[lib_idx].nav_stack.len();
        if stack_len < 2 {
            return;
        }
        let n = self.libs[lib_idx].nav_stack[stack_len - 2].items.len();
        if group_cursor >= n {
            return;
        }
        self.libs[lib_idx].clear_music_focus();
        self.libs[lib_idx].nav_stack.pop();
        if let Some(group_lvl) = self.libs[lib_idx].nav_stack.last_mut() {
            group_lvl.cursor = group_cursor;
        }
        let (group_id, group_name) = self.libs[lib_idx]
            .nav_stack
            .last()
            .and_then(|l| l.items.get(group_cursor))
            .map(|g| (g.id.clone(), g.name.clone()))
            .unwrap_or_default();
        if group_id.is_empty() {
            return;
        }
        self.libs[lib_idx].nav_stack.push(BrowseLevel {
            parent_id: group_id.clone(),
            title: group_name.clone(),
            items: vec![],
            total_count: 0,
            cursor: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: true,
            scroll: 0,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        });
        self.spawn_browse(
            lib_idx,
            group_id,
            group_name,
            None,
            false,
            "SortName".into(),
            "Ascending".into(),
        );
    }

    /// Whether the letter-range pill row applies to `lib_idx`
    /// right now: a non-music library at the top browse level of its nav
    /// stack (`nav_stack.len() == 1`), with a captured true total
    /// (`LibraryTab.library_total`) over `LIBRARY_PILL_THRESHOLD`. See
    /// `render::LetterFilter` and
    /// `maybe_capture_library_total_and_apply_default_pill`, which populates
    /// `library_total` on a library's first load.
    pub(super) fn should_show_letter_pills(&self, lib_idx: usize) -> bool {
        let Some(lib) = self.libs.get(lib_idx) else {
            return false;
        };
        if lib.library.collection_type == "music" {
            return false;
        }
        if self.is_home_video_view(lib_idx) {
            return false;
        }
        if lib.nav_stack.len() != 1 {
            return false;
        }
        lib.library_total.is_some()
    }

    /// Selects letter-range pill `pill_index` for `lib_idx`'s top level (a
    /// direct precedent: `select_music_group`). Resets cursor/scroll, marks
    /// the level loading, and spawns a scoped refresh fetching only that
    /// range from Emby (`get_items_sorted_ranged`) -- the existing in-list
    /// letter headers (`list.rs`) then bucket the smaller slice per-letter.
    /// Persists the choice so it survives a restart (`LibraryPositionLevel`).
    pub(super) fn select_letter_pill(&mut self, lib_idx: usize, pill_index: usize) {
        if !self.should_show_letter_pills(lib_idx) {
            return;
        }
        let Some(filter) = super::render::LetterFilter::for_index(pill_index) else {
            return;
        };
        let Some(lvl) = self.libs[lib_idx].nav_stack.last() else {
            return;
        };
        if lvl.letter_filter.as_ref() == Some(&filter) {
            return;
        }
        let parent_id = lvl.parent_id.clone();
        let item_types = lvl.item_types.clone();
        let unplayed_only = lvl.unplayed_only;
        let sort_by = lvl.sort_by.clone();
        let sort_order = lvl.sort_order.clone();
        if let Some(last) = self.libs[lib_idx].nav_stack.last_mut() {
            last.letter_filter = Some(filter.clone());
            last.cursor = 0;
            last.scroll = 0;
            last.loading = true;
            last.items.clear();
            last.all_items = None;
        }
        self.spawn_refresh(
            lib_idx,
            parent_id,
            item_types,
            unplayed_only,
            sort_by,
            sort_order,
            0,
            Some(filter),
        );
        self.save_default_library_position(lib_idx);
    }

    /// Cycles the letter-range pill row by `delta` (`[`/`]` keyboard
    /// bindings), wrapping around -- the established pattern from
    /// `switch_music_group`.
    pub(super) fn cycle_letter_pill(&mut self, lib_idx: usize, delta: i64) {
        if !self.should_show_letter_pills(lib_idx) {
            return;
        }
        let n = super::render::LetterFilter::count();
        if n == 0 {
            return;
        }
        let current = self.libs[lib_idx]
            .nav_stack
            .last()
            .and_then(|l| l.letter_filter.as_ref())
            .map(|f| f.index)
            .unwrap_or(0);
        let next = (current as i64 + delta).rem_euclid(n as i64) as usize;
        self.select_letter_pill(lib_idx, next);
    }

    /// If the music-group library's nav_stack was truncated back to just the
    /// group level (e.g., by a stale breadcrumb click), immediately re-push the
    /// current group's album level so the combined view stays intact.
    pub(super) fn ensure_music_group_album_level(&mut self, lib_idx: usize) {
        if lib_idx >= self.libs.len() {
            return;
        }
        let should_push = self.libs[lib_idx].library.collection_type == "music"
            && self
                .music_levels
                .first()
                .map(|s| s == "group")
                .unwrap_or(false)
            && self.libs[lib_idx].nav_stack.len() == 1
            && !self.libs[lib_idx].nav_stack[0].items.is_empty();
        if !should_push {
            return;
        }
        let cur = self.libs[lib_idx].nav_stack[0].cursor;
        let n = self.libs[lib_idx].nav_stack[0].items.len();
        if cur >= n {
            return;
        }
        let (group_id, group_name) = {
            let g = &self.libs[lib_idx].nav_stack[0].items[cur];
            (g.id.clone(), g.name.clone())
        };
        self.libs[lib_idx].nav_stack.push(BrowseLevel {
            parent_id: group_id.clone(),
            title: group_name.clone(),
            items: vec![],
            total_count: 0,
            cursor: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: true,
            scroll: 0,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        });
        self.spawn_browse(
            lib_idx,
            group_id,
            group_name,
            None,
            false,
            "SortName".into(),
            "Ascending".into(),
        );
    }

    /// Whether the item currently playing is audio-only, used to decide
    /// `a`'s mute-vs-cycle branch (`Action::ToggleMuteOrCycleAudio`). When a
    /// remote session is connected, reads the same `media_info.audio_only`
    /// flag the render layer already uses to pick audio-only vs. video
    /// indicators for that session (see #88), rather than the local
    /// playlist/cursor state, which doesn't reflect what the session is
    /// playing.
    pub(super) fn is_audio_item(&self) -> bool {
        self.playback_target().is_audio_item(self)
    }

    // Visibility bump: private -> `pub(super)`. Called from
    // `handle_lib_loaded`, which stays behind in `actions.rs`.
    pub(super) fn maybe_auto_push_music_group_level(&mut self, lib_idx: usize) {
        // When the group list loads for a music library with
        // levels = ["group", …], automatically push the first group's album
        // level so the user lands directly in the combined group view.
        let should_auto_push_music = self.tab.emby_library_index() == Some(lib_idx)
            && self
                .libs
                .get(lib_idx)
                .map(|lib| {
                    lib.library.collection_type == "music"
                        && self
                            .music_levels
                            .first()
                            .map(|s| s == "group")
                            .unwrap_or(false)
                        && lib.nav_stack.len() == 1
                        && !lib.nav_stack[0].items.is_empty()
                })
                .unwrap_or(false);

        if should_auto_push_music {
            let (group_id, group_name) = self
                .libs
                .get(lib_idx)
                .and_then(|lib| lib.nav_stack.last())
                .and_then(|l| l.items.get(l.cursor))
                .map(|g| (g.id.clone(), g.name.clone()))
                .unwrap_or_default();
            if !group_id.is_empty() {
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    lib.nav_stack.push(BrowseLevel {
                        parent_id: group_id.clone(),
                        title: group_name.clone(),
                        items: vec![],
                        total_count: 0,
                        cursor: 0,
                        item_types: None,
                        unplayed_only: false,
                        sort_by: "SortName".into(),
                        sort_order: "Ascending".into(),
                        loading: true,
                        scroll: 0,
                        all_items: None,
                        letter_filter: None,
                        music_grouping: None,
                    });
                }
                self.spawn_browse(
                    lib_idx,
                    group_id,
                    group_name,
                    None,
                    false,
                    "SortName".into(),
                    "Ascending".into(),
                );
            }
        }
    }
}
