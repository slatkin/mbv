use super::album_plan::GroupedAlbumDisplayRow;
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
        let selectable: Vec<usize> = plan
            .rows
            .iter()
            .enumerate()
            .filter_map(|(idx, row)| row.row_target(true).map(|_| idx))
            .collect();
        if selectable.is_empty() {
            return true;
        }
        let current_pos = selectable
            .iter()
            .position(|row_idx| *row_idx == plan.display_cursor)
            .unwrap_or(0);
        let new_pos = (current_pos as i64 + delta).clamp(0, selectable.len() as i64 - 1) as usize;
        let mut target = plan.rows[selectable[new_pos]].row_target(true);
        if matches!(&target, Some(LibraryRowTarget::Album(idx)) if *idx == cursor) {
            if let Some(group) = plan
                .selected_group_indices
                .as_ref()
                .filter(|group| group.len() > super::album_plan::SELECTED_ALBUM_WINDOW)
            {
                let direction = delta.signum();
                let cursor_pos = plan.order.iter().position(|&idx| idx == cursor);
                let candidate = cursor_pos
                    .and_then(|pos| pos.checked_add_signed(direction as isize))
                    .and_then(|pos| plan.order.get(pos).copied());
                if let Some(candidate) = candidate {
                    let visible = plan.rows.iter().any(
                        |row| matches!(row, GroupedAlbumDisplayRow::Album(idx) if *idx == candidate),
                    );
                    if !visible && group.contains(&candidate) {
                        target = Some(LibraryRowTarget::Album(candidate));
                    }
                }
            }
        }
        drop(plan);
        match target {
            Some(LibraryRowTarget::ArtistHeader(selection)) => {
                self.set_artist_header_focus(lib_idx, selection);
            }
            Some(LibraryRowTarget::Album(idx)) => {
                self.clear_artist_header_focus(lib_idx);
                if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
                    if level.cursor != idx {
                        level.cursor = idx;
                        self.libs[lib_idx].album_track_focus = None;
                    }
                }
            }
            None => {}
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
        let target = if to_end {
            plan.rows.iter().rev().find_map(|row| row.row_target(true))
        } else {
            plan.rows.iter().find_map(|row| row.row_target(true))
        };
        drop(plan);
        match target {
            Some(LibraryRowTarget::ArtistHeader(selection)) => {
                self.set_artist_header_focus(lib_idx, selection);
            }
            Some(LibraryRowTarget::Album(idx)) => {
                self.clear_artist_header_focus(lib_idx);
                if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
                    level.cursor = idx;
                    self.libs[lib_idx].album_track_focus = None;
                }
            }
            None => {}
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
        let target_row = if page_down {
            (plan.display_cursor + page).min(plan.rows.len().saturating_sub(1))
        } else {
            plan.display_cursor.saturating_sub(page)
        };
        let new_cursor = if let Some(group) = plan
            .selected_group_indices
            .as_ref()
            .filter(|group| group.len() > super::album_plan::SELECTED_ALBUM_WINDOW)
        {
            let cursor_pos = plan
                .order
                .iter()
                .position(|&idx| idx == cursor)
                .unwrap_or(0);
            let target_pos = if page_down {
                cursor_pos
                    .saturating_add(page)
                    .min(plan.order.len().saturating_sub(1))
            } else {
                cursor_pos.saturating_sub(page)
            };
            let target = plan.order[target_pos];
            if group.contains(&target) {
                target
            } else if page_down {
                group.last().copied().unwrap_or(cursor)
            } else {
                group.first().copied().unwrap_or(cursor)
            }
        } else if page_down {
            plan.rows
                .iter()
                .skip(target_row)
                .find_map(GroupedAlbumDisplayRow::album_index)
                .unwrap_or_else(|| plan.order.last().copied().unwrap_or(cursor))
        } else {
            plan.rows[..=target_row]
                .iter()
                .rev()
                .find_map(GroupedAlbumDisplayRow::album_index)
                .unwrap_or_else(|| plan.order.first().copied().unwrap_or(cursor))
        };

        self.clear_artist_header_focus(lib_idx);
        if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
            if level.cursor != new_cursor {
                level.cursor = new_cursor;
                self.libs[lib_idx].album_track_focus = None;
            }
        }
        if idle {
            self.maybe_fetch_next_page(lib_idx);
        }
        true
    }
}
