use super::album_plan::{GroupedAlbumDisplayPlan, GroupedAlbumDisplayRow};
use crate::app::layout::LibraryRowTarget;
use crate::app::music_grouping::GroupedAlbumCatalog;
use crate::app::{App, ArtistHeaderSelection};
use mbv_core::api::MediaItem;

impl App {
    pub(super) fn selected_power_music_artist_header(
        &self,
        lib_idx: usize,
    ) -> Option<ArtistHeaderSelection> {
        if !self.is_music_group_view(lib_idx) {
            return None;
        }
        self.libs.get(lib_idx)?.artist_header_focus.clone()
    }

    pub(in crate::app) fn clear_artist_header_focus(&mut self, lib_idx: usize) {
        if let Some(lib) = self.libs.get_mut(lib_idx) {
            lib.artist_header_focus = None;
        }
    }

    fn set_artist_header_focus(&mut self, lib_idx: usize, selection: ArtistHeaderSelection) {
        if let Some(lib) = self.libs.get_mut(lib_idx) {
            lib.album_track_focus = None;
            lib.artist_header_focus = Some(selection);
        }
    }

    fn grouped_album_navigation_targets(
        albums: &[mbv_core::api::MediaItem],
        plan: &GroupedAlbumDisplayPlan,
    ) -> Vec<LibraryRowTarget> {
        let Some(selected_group) = plan.selected_group_indices.as_ref() else {
            return plan
                .rows
                .iter()
                .filter_map(|row| row.row_target(true))
                .collect();
        };
        let Some(first_idx) = selected_group.first().copied() else {
            return Vec::new();
        };
        let Some(first_album_id) = albums.get(first_idx).map(|album| album.id.as_str()) else {
            return Vec::new();
        };

        let mut targets = Vec::new();
        let mut inside_selected_group = false;
        for row in &plan.rows {
            match row {
                GroupedAlbumDisplayRow::ArtistHeader(selection) => {
                    inside_selected_group = selection.first_album_id == first_album_id;
                    targets.push(LibraryRowTarget::ArtistHeader(selection.clone()));
                    if inside_selected_group {
                        targets.extend(selected_group.iter().copied().map(LibraryRowTarget::Album));
                    }
                }
                GroupedAlbumDisplayRow::Album(idx) if !inside_selected_group => {
                    targets.push(LibraryRowTarget::Album(*idx));
                }
                _ => {}
            }
        }
        targets
    }

    /// Navigation targets for the settled catalog: every artist header
    /// followed by every album in its group, in the catalog's display order.
    /// Matches the plan-derived target list the fallback path produces.
    fn catalog_album_navigation_targets(
        catalog: &GroupedAlbumCatalog,
        albums_len: usize,
    ) -> Vec<LibraryRowTarget> {
        let mut targets = Vec::new();
        for group in &catalog.groups {
            targets.push(LibraryRowTarget::ArtistHeader(ArtistHeaderSelection {
                first_album_id: catalog.entries[group.start].album_id.clone(),
                artist_label: group.artist.clone(),
            }));
            for entry in &catalog.entries[group.start..group.end] {
                if entry.album_index < albums_len {
                    targets.push(LibraryRowTarget::Album(entry.album_index));
                }
            }
        }
        targets
    }

    /// Navigation targets and header validity for the current music album
    /// level, sourced from the settled catalog when available (no display
    /// plan rebuild, no artist derivation) and a synchronous display plan
    /// otherwise. Callers must have already checked the level is non-empty.
    fn music_group_navigation(
        &mut self,
        lib_idx: usize,
        albums: &[MediaItem],
        cursor: usize,
        selected: Option<&ArtistHeaderSelection>,
    ) -> (Vec<LibraryRowTarget>, bool) {
        let catalog = self.libs[lib_idx]
            .nav_stack
            .last()
            .and_then(|l| l.music_grouping.as_ref())
            .and_then(|s| s.settled.as_ref());
        if let Some(cat) = catalog {
            let header_valid = selected.is_none_or(|sel| {
                cat.groups.iter().any(|g| {
                    g.artist == sel.artist_label
                        && cat.entries[g.start].album_id == sel.first_album_id
                })
            });
            let targets = Self::catalog_album_navigation_targets(cat, albums.len());
            return (targets, header_valid);
        }
        let expand_selected = self.libs[lib_idx].album_track_focus.is_some();
        let album_info = self.group_album_info(albums, None);
        let order = super::sorted_group_album_order(&album_info);
        let plan = self.build_grouped_album_display_plan(
            albums,
            &album_info,
            &order,
            cursor,
            false,
            true,
            selected,
            expand_selected,
            None,
            false, // hero_handles_detail: cursor needs the full plan
        );
        let targets = Self::grouped_album_navigation_targets(albums, &plan);
        (targets, plan.selected_artist_header_valid)
    }

    pub(in crate::app) fn move_power_music_group_display_cursor(
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
            self.clear_artist_header_focus(lib_idx);
            return true;
        }
        let cursor = level.cursor;
        let albums = level.items.clone();
        let selected = self.selected_power_music_artist_header(lib_idx);
        let (targets, header_valid) =
            self.music_group_navigation(lib_idx, &albums, cursor, selected.as_ref());
        if selected.is_some() && !header_valid {
            self.clear_artist_header_focus(lib_idx);
        }
        if targets.is_empty() {
            return true;
        }
        let current_pos = targets
            .iter()
            .position(|target| {
                (selected.as_ref().is_some_and(|selection| {
                    matches!(target, LibraryRowTarget::ArtistHeader(current) if current == selection)
                }))
                    || (selected.is_none()
                        && matches!(target, LibraryRowTarget::Album(idx) if *idx == cursor))
            })
            .unwrap_or(0);
        let new_pos = Self::grouped_cursor_target(&targets, current_pos, delta);
        let target = targets[new_pos].clone();
        match target {
            LibraryRowTarget::ArtistHeader(selection) => {
                self.set_artist_header_focus(lib_idx, selection);
            }
            LibraryRowTarget::Album(idx) => {
                self.clear_artist_header_focus(lib_idx);
                if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
                    if level.cursor != idx {
                        level.cursor = idx;
                        self.libs[lib_idx].album_track_focus = None;
                    }
                }
            }
        }
        true
    }

    /// Target position for a `delta`-item move within a grouped album
    /// list. `delta` is already column-scaled by the caller: `±cols` for
    /// vertical up/down moves (one rendered row) and `±1` for horizontal
    /// left/right moves (one album). With `cols == 1` the two coincide.
    ///
    /// Artist headers are never a resting position for arrow movement: a
    /// move that lands on one continues to the nearest album in the
    /// movement direction. That makes "down from a group's last row" land
    /// on the first album of the next group and "up from a group's first
    /// row" land on the last album of the previous group -- the same
    /// column-wrapping the letter-group cursor uses. When the cursor
    /// already rests on a header (reached via Ctrl+PgUp/PgDn or a click),
    /// up/down step into the adjacent album row.
    fn grouped_cursor_target(targets: &[LibraryRowTarget], pos: usize, delta: i64) -> usize {
        let len = targets.len();
        if len == 0 {
            return pos;
        }
        if matches!(targets[pos], LibraryRowTarget::ArtistHeader(_)) {
            // A header spans the full row: step into the row of albums
            // that follows it (down) or precedes it (up).
            if delta > 0 {
                return next_album_target(targets, pos).unwrap_or(pos);
            }
            return prev_album_target(targets, pos).unwrap_or(pos);
        }
        let naive = (pos as i64 + delta).clamp(0, len as i64 - 1) as usize;
        match targets[naive] {
            LibraryRowTarget::ArtistHeader(_) if delta > 0 => {
                next_album_target(targets, naive).unwrap_or(pos)
            }
            LibraryRowTarget::ArtistHeader(_) => prev_album_target(targets, naive).unwrap_or(pos),
            _ => naive,
        }
    }

    pub(in crate::app) fn jump_power_music_group_display_cursor(
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
            self.clear_artist_header_focus(lib_idx);
            return true;
        }
        let albums = level.items.clone();
        let selected = self.selected_power_music_artist_header(lib_idx);
        let (targets, _) =
            self.music_group_navigation(lib_idx, &albums, level.cursor, selected.as_ref());
        let Some(target) = (if to_end {
            targets.last().cloned()
        } else {
            targets.first().cloned()
        }) else {
            return true;
        };
        match target {
            LibraryRowTarget::ArtistHeader(selection) => {
                self.set_artist_header_focus(lib_idx, selection);
            }
            LibraryRowTarget::Album(idx) => {
                self.clear_artist_header_focus(lib_idx);
                if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
                    level.cursor = idx;
                    self.libs[lib_idx].album_track_focus = None;
                }
            }
        }
        true
    }

    pub(in crate::app) fn jump_power_music_group_display_cursor_to_artist(
        &mut self,
        lib_idx: usize,
        forward: bool,
    ) -> bool {
        if !self.is_music_group_view(lib_idx) {
            return false;
        }
        let Some(level) = self.libs[lib_idx].nav_stack.last() else {
            return true;
        };
        if level.items.is_empty() {
            self.clear_artist_header_focus(lib_idx);
            return true;
        }
        let cursor = level.cursor;
        let albums = level.items.clone();
        let selected = self.selected_power_music_artist_header(lib_idx);
        let (targets, header_valid) =
            self.music_group_navigation(lib_idx, &albums, cursor, selected.as_ref());
        if selected.is_some() && !header_valid {
            self.clear_artist_header_focus(lib_idx);
        }
        if targets.is_empty() {
            return true;
        }
        let current_pos = targets
            .iter()
            .position(|target| {
                (selected.as_ref().is_some_and(|selection| {
                    matches!(target, LibraryRowTarget::ArtistHeader(current) if current == selection)
                }))
                    || (selected.is_none()
                        && matches!(target, LibraryRowTarget::Album(idx) if *idx == cursor))
            })
            .unwrap_or(0);
        let found = if forward {
            targets
                .iter()
                .skip(current_pos + 1)
                .find(|t| matches!(t, LibraryRowTarget::ArtistHeader(_)))
                .cloned()
                .unwrap_or_else(|| targets.last().cloned().unwrap())
        } else {
            targets[..current_pos]
                .iter()
                .rev()
                .find(|t| matches!(t, LibraryRowTarget::ArtistHeader(_)))
                .cloned()
                .unwrap_or_else(|| targets.first().cloned().unwrap())
        };
        match found {
            LibraryRowTarget::ArtistHeader(selection) => {
                self.set_artist_header_focus(lib_idx, selection);
            }
            LibraryRowTarget::Album(idx) => {
                self.clear_artist_header_focus(lib_idx);
                if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
                    if level.cursor != idx {
                        level.cursor = idx;
                        self.libs[lib_idx].album_track_focus = None;
                    }
                }
            }
        }
        true
    }

    pub(in crate::app) fn selected_artist_header_album_items(
        &mut self,
        lib_idx: usize,
    ) -> Option<(ArtistHeaderSelection, Vec<mbv_core::api::MediaItem>)> {
        let selection = self.selected_power_music_artist_header(lib_idx)?;
        self.artist_header_album_items_for_selection(lib_idx, &selection)
            .map(|items| (selection, items))
    }

    pub(in crate::app) fn artist_header_album_items_for_selection(
        &mut self,
        lib_idx: usize,
        selection: &ArtistHeaderSelection,
    ) -> Option<Vec<mbv_core::api::MediaItem>> {
        if !self.is_music_group_view(lib_idx) {
            return None;
        }
        let level = self.libs[lib_idx].nav_stack.last()?;
        let albums = level.items.clone();
        if albums.is_empty() {
            self.clear_artist_header_focus(lib_idx);
            return None;
        }
        let catalog = self.libs[lib_idx]
            .nav_stack
            .last()
            .and_then(|l| l.music_grouping.as_ref())
            .and_then(|s| s.settled.as_ref());
        if let Some(cat) = catalog {
            let valid = cat.groups.iter().any(|g| {
                g.artist == selection.artist_label
                    && cat.entries[g.start].album_id == selection.first_album_id
            });
            if !valid {
                if self.libs[lib_idx]
                    .artist_header_focus
                    .as_ref()
                    .is_some_and(|focused| focused == selection)
                {
                    self.clear_artist_header_focus(lib_idx);
                }
                return None;
            }
            let group = cat.group_for_artist(&selection.artist_label)?;
            return Some(
                cat.entries[group.start..group.end]
                    .iter()
                    .filter_map(|e| albums.get(e.album_index).cloned())
                    .collect(),
            );
        }
        let expand_selected = self.libs[lib_idx].album_track_focus.is_some();
        let album_info = self.group_album_info(&albums, None);
        let order = super::sorted_group_album_order(&album_info);
        let plan = self.build_grouped_album_display_plan(
            &albums,
            &album_info,
            &order,
            level.cursor,
            false,
            true,
            Some(selection),
            expand_selected,
            None,
            false, // hero_handles_detail: cursor needs the full plan
        );
        if !plan.selected_artist_header_valid {
            if self.libs[lib_idx]
                .artist_header_focus
                .as_ref()
                .is_some_and(|focused| focused == selection)
            {
                self.clear_artist_header_focus(lib_idx);
            }
            return None;
        }

        if let Some(indices) = plan.selected_group_indices {
            return Some(
                indices
                    .into_iter()
                    .filter_map(|idx| albums.get(idx).cloned())
                    .collect(),
            );
        }

        let mut in_group = false;
        let mut members = Vec::new();
        for row in plan.rows {
            match row {
                GroupedAlbumDisplayRow::ArtistHeader(header) => {
                    in_group = header == *selection;
                }
                GroupedAlbumDisplayRow::Album(idx) if in_group => {
                    if let Some(album) = albums.get(idx) {
                        members.push(album.clone());
                    }
                }
                _ => {}
            }
        }
        Some(members)
    }

    pub(in crate::app) fn page_power_grouped_album_cursor(
        &mut self,
        lib_idx: usize,
        page_down: bool,
    ) -> bool {
        if self.library_tab != lib_idx + 1
            || !matches!(self.panel_focus, crate::app::PanelFocus::Library)
            || self.libs[lib_idx].album_track_focus.is_some()
            || !self.is_viewing_album_folders(lib_idx)
        {
            return false;
        }

        let idle = self.list_image_fetches_allowed();
        let now = std::time::Instant::now();
        self.last_nav_at = now;
        self.mark_power_library_navigation(now);

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
        let selected = self.selected_power_music_artist_header(lib_idx);
        let (targets, header_valid) =
            self.music_group_navigation(lib_idx, &albums, cursor, selected.as_ref());
        if selected.is_some() && !header_valid {
            self.clear_artist_header_focus(lib_idx);
        }
        if let Some(current_pos) = targets.iter().position(|target| {
            (selected.as_ref().is_some_and(|selection| {
                matches!(target, LibraryRowTarget::ArtistHeader(current) if current == selection)
            }))
                || (selected.is_none()
                    && matches!(target, LibraryRowTarget::Album(idx) if *idx == cursor))
        }) {
            let target_pos = if page_down {
                current_pos.saturating_add(page).min(targets.len() - 1)
            } else {
                current_pos.saturating_sub(page)
            };
            let target = match targets[target_pos].clone() {
                LibraryRowTarget::ArtistHeader(_) if page_down => targets
                    .iter()
                    .skip(target_pos + 1)
                    .find(|target| matches!(target, LibraryRowTarget::Album(_)))
                    .cloned()
                    .unwrap_or_else(|| targets[target_pos].clone()),
                LibraryRowTarget::ArtistHeader(_) => targets[..target_pos]
                    .iter()
                    .rev()
                    .find(|target| matches!(target, LibraryRowTarget::Album(_)))
                    .cloned()
                    .or_else(|| {
                        targets
                            .iter()
                            .find(|target| matches!(target, LibraryRowTarget::Album(_)))
                            .cloned()
                    })
                    .unwrap_or_else(|| targets[target_pos].clone()),
                target => target,
            };
            match target {
                LibraryRowTarget::ArtistHeader(selection) => {
                    self.set_artist_header_focus(lib_idx, selection);
                }
                LibraryRowTarget::Album(new_cursor) => {
                    self.clear_artist_header_focus(lib_idx);
                    if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
                        if level.cursor != new_cursor {
                            level.cursor = new_cursor;
                            self.libs[lib_idx].album_track_focus = None;
                        }
                    }
                }
            }
        }
        if idle {
            self.maybe_fetch_next_page(lib_idx);
        }
        true
    }
}

/// Index of the first album target strictly after `from`, or `None`.
fn next_album_target(targets: &[LibraryRowTarget], from: usize) -> Option<usize> {
    (from + 1..targets.len()).find(|&i| matches!(targets[i], LibraryRowTarget::Album(_)))
}

/// Index of the last album target strictly before `from`, or `None`.
fn prev_album_target(targets: &[LibraryRowTarget], from: usize) -> Option<usize> {
    (0..from)
        .rev()
        .find(|&i| matches!(targets[i], LibraryRowTarget::Album(_)))
}
