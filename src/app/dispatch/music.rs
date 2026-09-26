use crate::app::state::types::browse::BrowseResting;
use crate::app::{App, BrowseLevel};

/// The shared eligibility gate for the grouped Music owner, consumed by
/// `is_music_group_view` and grouped landing validation. Keep this in sync
/// with the shape that `is_music_group_view` exposes to the shell: the
/// configured path must begin at the grouping level, have a group and album
/// level on the stack.
fn wrapped_index(current: usize, delta: i64, count: usize) -> usize {
    let current = i128::try_from(current).expect("usize fits in i128");
    let count = i128::try_from(count).expect("usize fits in i128");
    usize::try_from((current + i128::from(delta)).rem_euclid(count))
        .expect("wrapped index fits in usize")
}

fn is_grouped_music_path(music_levels: &[String], nav_stack: &[BrowseLevel]) -> bool {
    music_levels.first().is_some_and(|level| level == "group")
        && nav_stack.len() >= 2
        && music_levels
            .get(nav_stack.len() - 1)
            .is_some_and(|level| level == "album")
}

impl App {
    /// True when mbv should show the combined music group view:
    /// a group-selector bar at top with the album list below.
    /// Activated when `music.levels` starts with `"group"` and the nav stack
    /// has a group level plus an album level above it.
    pub(in crate::app) fn is_grouped_music_library(&self, lib_idx: usize) -> bool {
        self.libs[lib_idx].library.collection_type == "music"
            && self
                .music_levels
                .first()
                .is_some_and(|level| level == "group")
    }

    pub(in crate::app) fn is_music_group_view(&self, lib_idx: usize) -> bool {
        if !self.is_grouped_music_library(lib_idx) {
            return false;
        }
        let lib = &self.libs[lib_idx];
        is_grouped_music_path(&self.music_levels, &lib.nav_stack)
    }

    /// D7: validate a prepared recursive-album landing before its drain
    /// commits anything. The target library must still exist and the prepared
    /// stack must reproduce the configured `music.levels` album shape through
    /// the shared `is_grouped_music_path` gate used by the view and landing
    /// validation. A
    /// miss is a rejected apply: the caller flashes the library error and
    /// leaves the active tab, nav stack, saved Library position, and
    /// retained-owner selection unchanged.
    pub(in crate::app) fn validate_grouped_music_landing(
        &self,
        library_id: &str,
        nav_stack: &[BrowseLevel],
    ) -> Result<(), String> {
        let could_not_build =
            || format!("Could not build the configured music path for '{library_id}'");
        let lib = self
            .libs
            .iter()
            .find(|lib| lib.library.id == library_id)
            .ok_or_else(|| format!("Could not find library {library_id}"))?;
        if self.music_levels.is_empty()
            || self.music_levels.last().map(String::as_str) != Some("album")
        {
            return Err("Could not resolve the configured music album level".into());
        }
        if nav_stack.len() != self.music_levels.len()
            || lib.library.collection_type != "music"
            || !is_grouped_music_path(&self.music_levels, nav_stack)
        {
            return Err(could_not_build());
        }
        // Each level must hang off the selected item of the level above,
        // rooted at the library, with a resting item at every step.
        let mut parent_id = lib.library.id.as_str();
        for level in nav_stack {
            if level.parent_id != parent_id {
                return Err(could_not_build());
            }
            let Some(item) = level.items.get(level.resting().cursor()) else {
                return Err(could_not_build());
            };
            parent_id = item.id.as_str();
        }
        Ok(())
    }

    /// Switch to the previous (`delta == -1`) or next (`delta == 1`) group
    /// while in the combined music group view. Pops the current album level,
    /// adjusts the group cursor (wraps around), then kicks off a fetch for
    /// the new group's albums.
    pub(in crate::app) fn switch_music_group(&mut self, lib_idx: usize, delta: i64) {
        let stack_len = self.libs[lib_idx].nav_stack.len();
        if stack_len < 2 {
            return;
        }

        // Verify count before popping so we never lose the album level.
        // Nothing to switch between with fewer than two groups: bail before
        // popping/re-fetching so `[`/`]` is a true no-op there.
        let n = self.libs[lib_idx].nav_stack[stack_len - 2].items.len();
        if n < 2 {
            return;
        }

        // Pop the album level.
        self.libs[lib_idx].nav_stack.pop();
        let cur = self.libs[lib_idx]
            .nav_stack
            .last()
            .map_or(0, |l| l.resting().cursor());
        // Wrap-around navigation (unlike seasons which clamp).
        let new_cursor = wrapped_index(cur, delta, n);
        if let Some(group_lvl) = self.libs[lib_idx].nav_stack.last_mut() {
            group_lvl.set_resting_cursor(new_cursor);
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
            fetched_rows: 0,
            parent_id: group_id.clone(),
            title: group_name.clone(),
            items: vec![],
            total_count: 0,
            resting: BrowseResting::new(0, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: true,

            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
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

    pub(in crate::app) fn select_music_group(&mut self, lib_idx: usize, group_cursor: usize) {
        let stack_len = self.libs[lib_idx].nav_stack.len();
        if stack_len < 2 {
            return;
        }
        let n = self.libs[lib_idx].nav_stack[stack_len - 2].items.len();
        if group_cursor >= n {
            return;
        }
        self.libs[lib_idx].nav_stack.pop();
        if let Some(group_lvl) = self.libs[lib_idx].nav_stack.last_mut() {
            group_lvl.set_resting_cursor(group_cursor);
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
            fetched_rows: 0,
            parent_id: group_id.clone(),
            title: group_name.clone(),
            items: vec![],
            total_count: 0,
            resting: BrowseResting::new(0, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: true,
            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
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
    pub(in crate::app) fn should_show_letter_pills(&self, lib_idx: usize) -> bool {
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
    pub(in crate::app) fn select_letter_pill(&mut self, lib_idx: usize, pill_index: usize) {
        if !self.should_show_letter_pills(lib_idx) {
            return;
        }
        if self.libs[lib_idx].library.collection_type == "tvshows" {
            self.select_tv_letter_pill(lib_idx, pill_index);
        } else {
            self.select_library_letter_pill(lib_idx, pill_index);
        }
    }

    fn select_tv_letter_pill(&mut self, lib_idx: usize, pill_index: usize) {
        if self.libs[lib_idx].tv_content_mode.is_none() {
            self.libs[lib_idx].tv_content_mode = self.libs[lib_idx]
                .nav_stack
                .last()
                .and_then(|level| level.tv_content_mode.clone());
        }
        // Old saved positions and direct callers can still arrive with no
        // resolved TV mode. Treat an explicit pill selection as the
        // legacy range until the first load resolves the size default.
        if self.libs[lib_idx].tv_content_mode.is_none() {
            self.select_legacy_tv_letter_pill(lib_idx, pill_index);
            return;
        }
        let large = self.libs[lib_idx]
            .library_total
            .is_some_and(|total| total > crate::app::render::LIBRARY_PILL_THRESHOLD);
        let Some(mode) = Self::tv_mode_for_pill(pill_index, large) else {
            return;
        };
        if matches!((&mode, large), (mbv_core::config::TvContentMode::Range(index), true) if *index >= crate::app::render::LetterFilter::count_for_kind(crate::app::render::LetterFilterKind::Tv))
        {
            return;
        }
        self.apply_tv_letter_mode(lib_idx, &mode);
    }

    fn tv_mode_for_pill(pill_index: usize, large: bool) -> Option<mbv_core::config::TvContentMode> {
        match pill_index {
            0 => Some(mbv_core::config::TvContentMode::Latest),
            1 => Some(mbv_core::config::TvContentMode::Upcoming),
            index if large => Some(mbv_core::config::TvContentMode::Range(index - 2)),
            2 => Some(mbv_core::config::TvContentMode::All),
            _ => None,
        }
    }

    fn select_legacy_tv_letter_pill(&mut self, lib_idx: usize, pill_index: usize) {
        let Some(filter) = crate::app::render::LetterFilter::for_index_for_kind(
            pill_index,
            crate::app::render::LetterFilterKind::Tv,
        ) else {
            return;
        };
        let Some(level) = self.libs[lib_idx].nav_stack.last() else {
            return;
        };
        let mut key = crate::app::state::types::browse::LevelFetchKey::from_level(level);
        key.letter_filter = Some(filter.clone());
        if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
            level.letter_filter = Some(filter);
            level.set_resting_cursor(0);
            level.set_resting_scroll(0);
            level.loading = true;
            level.items.clear();
        }
        self.spawn_refresh(lib_idx, 0, key);
        self.save_default_library_position(lib_idx);
    }

    fn apply_tv_letter_mode(&mut self, lib_idx: usize, mode: &mbv_core::config::TvContentMode) {
        let Some(level) = self.libs[lib_idx].nav_stack.last() else {
            return;
        };
        let parent_id = level.parent_id.clone();
        let unplayed_only = level.unplayed_only;
        let sort_by = level.sort_by.clone();
        let sort_order = level.sort_order.clone();
        let current = self.libs[lib_idx].tv_content_mode.as_ref();
        if current == Some(mode) && !matches!(mode, mbv_core::config::TvContentMode::Upcoming) {
            return;
        }
        self.libs[lib_idx].tv_content_mode = Some(mode.clone());
        if let Some(last) = self.libs[lib_idx].nav_stack.last_mut() {
            last.tv_content_mode = Some(mode.clone());
            last.letter_filter = None;
            last.set_resting_cursor(0);
            last.set_resting_scroll(0);
            last.all_items = None;
            last.items.clear();
        }
        self.refresh_tv_letter_mode(lib_idx, mode, parent_id, unplayed_only, sort_by, sort_order);
        self.save_default_library_position(lib_idx);
    }

    fn refresh_tv_letter_mode(
        &mut self,
        lib_idx: usize,
        mode: &mbv_core::config::TvContentMode,
        parent_id: String,
        unplayed_only: bool,
        sort_by: String,
        sort_order: String,
    ) {
        match mode {
            mbv_core::config::TvContentMode::Latest => {
                if let Some(last) = self.libs[lib_idx].nav_stack.last_mut() {
                    last.loading = true;
                    last.item_types = Some("Episode".into());
                }
                self.spawn_tv_latest(lib_idx, parent_id, self.libs[lib_idx].library.name.clone());
            }
            mbv_core::config::TvContentMode::Upcoming => {
                if let Some(last) = self.libs[lib_idx].nav_stack.last_mut() {
                    last.loading = true;
                    last.item_types = Some("Episode".into());
                }
                self.spawn_tv_upcoming(lib_idx, parent_id, self.libs[lib_idx].library.name.clone());
            }
            mbv_core::config::TvContentMode::All => {
                if let Some(last) = self.libs[lib_idx].nav_stack.last_mut() {
                    last.loading = true;
                    last.item_types = Some("Series".into());
                }
                let key = crate::app::state::types::browse::LevelFetchKey {
                    parent_id,
                    item_types: Some("Series".into()),
                    unplayed_only,
                    sort_by,
                    sort_order,
                    letter_filter: None,
                };
                self.spawn_refresh(lib_idx, 0, key);
            }
            mbv_core::config::TvContentMode::Range(index) => {
                let Some(filter) = crate::app::render::LetterFilter::for_index_for_kind(
                    *index,
                    crate::app::render::LetterFilterKind::Tv,
                ) else {
                    return;
                };
                if let Some(last) = self.libs[lib_idx].nav_stack.last_mut() {
                    last.loading = true;
                    last.item_types = Some("Series".into());
                    last.letter_filter = Some(filter.clone());
                }
                let key = crate::app::state::types::browse::LevelFetchKey {
                    parent_id,
                    item_types: Some("Series".into()),
                    unplayed_only,
                    sort_by,
                    sort_order,
                    letter_filter: Some(filter),
                };
                self.spawn_refresh(lib_idx, 0, key);
            }
        }
    }

    fn select_library_letter_pill(&mut self, lib_idx: usize, pill_index: usize) {
        let filter_kind = crate::app::render::LetterFilterKind::from_collection_type(
            self.libs[lib_idx].library.collection_type.as_str(),
        );
        let Some(filter) =
            crate::app::render::LetterFilter::for_index_for_kind(pill_index, filter_kind)
        else {
            return;
        };
        let Some(level) = self.libs[lib_idx].nav_stack.last() else {
            return;
        };
        if level.letter_filter.as_ref() == Some(&filter) {
            return;
        }
        let mut key = crate::app::state::types::browse::LevelFetchKey::from_level(level);
        key.letter_filter = Some(filter.clone());
        if let Some(last) = self.libs[lib_idx].nav_stack.last_mut() {
            last.letter_filter = Some(filter);
            last.set_resting_cursor(0);
            last.set_resting_scroll(0);
            last.loading = true;
            last.items.clear();
            last.all_items = None;
        }
        self.spawn_refresh(lib_idx, 0, key);
        self.save_default_library_position(lib_idx);
    }

    /// Cycles the letter-range pill row by `delta` (`[`/`]` keyboard
    /// bindings), wrapping around -- the established pattern from
    /// `switch_music_group`.
    pub(in crate::app) fn cycle_letter_pill(&mut self, lib_idx: usize, delta: i64) {
        if !self.should_show_letter_pills(lib_idx) {
            return;
        }
        if self.libs[lib_idx].library.collection_type == "tvshows" {
            if self.libs[lib_idx].tv_content_mode.is_none() {
                self.libs[lib_idx].tv_content_mode = self.libs[lib_idx]
                    .nav_stack
                    .last()
                    .and_then(|level| level.tv_content_mode.clone());
            }
            if self.libs[lib_idx].tv_content_mode.is_none() {
                let count = crate::app::render::LetterFilter::count_for_kind(
                    crate::app::render::LetterFilterKind::Tv,
                );
                let current = self.libs[lib_idx]
                    .nav_stack
                    .last()
                    .and_then(|level| level.letter_filter.as_ref())
                    .map_or(0, |filter| filter.index);
                let next = wrapped_index(current, delta, count);
                self.libs[lib_idx].tv_content_mode = None;
                self.select_letter_pill(lib_idx, next);
                return;
            }
            let large = self.libs[lib_idx]
                .library_total
                .is_some_and(|total| total > crate::app::render::LIBRARY_PILL_THRESHOLD);
            let count = if large { 5 } else { 3 };
            let current = match self.libs[lib_idx].tv_content_mode.as_ref() {
                Some(mbv_core::config::TvContentMode::Latest) => 0,
                Some(mbv_core::config::TvContentMode::Upcoming) => 1,
                Some(mbv_core::config::TvContentMode::All) => 2,
                Some(mbv_core::config::TvContentMode::Range(index)) => index + 2,
                None => {
                    if large {
                        0
                    } else {
                        2
                    }
                }
            };
            let next = wrapped_index(
                current,
                delta,
                usize::try_from(count).expect("small pill count"),
            );
            self.select_letter_pill(lib_idx, next);
            return;
        }
        let filter_kind = crate::app::render::LetterFilterKind::from_collection_type(
            self.libs[lib_idx].library.collection_type.as_str(),
        );
        let n = crate::app::render::LetterFilter::count_for_kind(filter_kind);
        if n == 0 {
            return;
        }
        let current = self.libs[lib_idx]
            .nav_stack
            .last()
            .and_then(|l| l.letter_filter.as_ref())
            .map_or(0, |f| f.index);
        let next = wrapped_index(current, delta, n);
        self.select_letter_pill(lib_idx, next);
    }

    /// If the music-group library's `nav_stack` was truncated back to just the
    /// group level (e.g., by a stale breadcrumb click), immediately re-push the
    /// current group's album level so the combined view stays intact.
    pub(in crate::app) fn ensure_music_group_album_level(&mut self, lib_idx: usize) {
        if lib_idx >= self.libs.len() {
            return;
        }
        let should_push = self.libs[lib_idx].library.collection_type == "music"
            && self.music_levels.first().is_some_and(|s| s == "group")
            && self.libs[lib_idx].nav_stack.len() == 1
            && !self.libs[lib_idx].nav_stack[0].items.is_empty();
        if !should_push {
            return;
        }
        let cur = self.libs[lib_idx].nav_stack[0].resting().cursor();
        let n = self.libs[lib_idx].nav_stack[0].items.len();
        if cur >= n {
            return;
        }
        let (group_id, group_name) = {
            let g = &self.libs[lib_idx].nav_stack[0].items[cur];
            (g.id.clone(), g.name.clone())
        };
        self.libs[lib_idx].nav_stack.push(BrowseLevel {
            fetched_rows: 0,
            parent_id: group_id.clone(),
            title: group_name.clone(),
            items: vec![],
            total_count: 0,
            resting: BrowseResting::new(0, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: true,
            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
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
    pub(in crate::app) fn is_audio_item(&self) -> bool {
        self.playback_target().is_audio_item(self)
    }

    // Visibility bump: private -> `pub(in crate::app)`. Called from
    // `handle_lib_loaded`, which stays behind in `actions.rs`.
    pub(in crate::app) fn maybe_auto_push_music_group_level(&mut self, lib_idx: usize) {
        // When the group list loads for a music library with
        // levels = ["group", …], automatically push the first group's album
        // level so the user lands directly in the combined group view.
        let should_auto_push_music = self.tab.emby_library_index() == Some(lib_idx)
            && self.libs.get(lib_idx).is_some_and(|lib| {
                lib.library.collection_type == "music"
                    && self.music_levels.first().is_some_and(|s| s == "group")
                    && lib.nav_stack.len() == 1
                    && !lib.nav_stack[0].items.is_empty()
            });

        if should_auto_push_music {
            let (group_id, group_name) = self
                .libs
                .get(lib_idx)
                .and_then(|lib| lib.nav_stack.last())
                .and_then(|l| l.items.get(l.resting().cursor()))
                .map(|g| (g.id.clone(), g.name.clone()))
                .unwrap_or_default();
            if !group_id.is_empty() {
                if let Some(lib) = self.libs.get_mut(lib_idx) {
                    lib.nav_stack.push(BrowseLevel {
                        fetched_rows: 0,
                        parent_id: group_id.clone(),
                        title: group_name.clone(),
                        items: vec![],
                        total_count: 0,
                        resting: BrowseResting::new(0, 0),
                        item_types: None,
                        unplayed_only: false,
                        sort_by: "SortName".into(),
                        sort_order: "Ascending".into(),
                        loading: true,
                        all_items: None,
                        letter_filter: None,
                        tv_content_mode: None,
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
