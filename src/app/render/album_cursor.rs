use super::album_plan::{GroupedAlbumDisplayPlan, GroupedAlbumDisplayRow};
use crate::app::layout::LibraryRowTarget;
use crate::app::{App, ArtistHeaderSelection};

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

    pub(in crate::app) fn move_power_music_group_display_cursor(
        &mut self,
        lib_idx: usize,
        delta: i64,
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
        let expand_selected = self.libs[lib_idx].album_track_focus.is_some();
        let plan = self.build_grouped_album_display_plan(
            &albums,
            cursor,
            false,
            true,
            selected.as_ref(),
            expand_selected,
            None,
        );
        if selected.is_some() && !plan.selected_artist_header_valid {
            self.clear_artist_header_focus(lib_idx);
        }
        let targets = Self::grouped_album_navigation_targets(&albums, &plan);
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
        let new_pos = (current_pos as i64 + delta).clamp(0, targets.len() as i64 - 1) as usize;
        let target = targets[new_pos].clone();
        drop(plan);
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
        let expand_selected = self.libs[lib_idx].album_track_focus.is_some();
        let plan = self.build_grouped_album_display_plan(
            &albums,
            level.cursor,
            false,
            true,
            selected.as_ref(),
            expand_selected,
            None,
        );
        let targets = Self::grouped_album_navigation_targets(&albums, &plan);
        let Some(target) = (if to_end {
            targets.last().cloned()
        } else {
            targets.first().cloned()
        }) else {
            return true;
        };
        drop(plan);
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
        let expand_selected = self.libs[lib_idx].album_track_focus.is_some();
        let plan = self.build_grouped_album_display_plan(
            &albums,
            level.cursor,
            false,
            true,
            Some(selection),
            expand_selected,
            None,
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
            || self.libs[lib_idx].search.is_some()
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
        let page = (self.layout.main.left_area.height as usize).max(1);
        let selected = self.selected_power_music_artist_header(lib_idx);
        let selectable_headers = self.is_music_group_view(lib_idx);
        let expand_selected = !selectable_headers || self.libs[lib_idx].album_track_focus.is_some();
        let plan = self.build_grouped_album_display_plan(
            &albums,
            cursor,
            false,
            selectable_headers,
            selected.as_ref(),
            expand_selected,
            None,
        );
        if selected.is_some() && !plan.selected_artist_header_valid {
            self.clear_artist_header_focus(lib_idx);
        }
        let targets = Self::grouped_album_navigation_targets(&albums, &plan);
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
