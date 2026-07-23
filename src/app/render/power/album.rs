use super::super::super::ui_util::*;
use super::{natural_sort_key, parse_album_folder_name, strip_article};
use crate::app::layout::{LayoutPower, PowerLeftRowTarget};
use crate::app::{palette, App, ArtistHeaderSelection};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;

const INLINE_ALBUM_DETAIL_INDENT: u16 = 2;
const INLINE_ALBUM_TITLE_EXTRA_INDENT: u16 = 1;
const INLINE_ALBUM_TRACK_EXTRA_INDENT: u16 = 2;
const INLINE_ALBUM_ART_COLS: u16 = 24;
const INLINE_ALBUM_ART_ROWS: u16 = 12;
const INLINE_ALBUM_ART_GAP: u16 = 2;
const INLINE_ALBUM_ART_RIGHT_PAD: u16 = 2;
const INLINE_ALBUM_ART_RESERVED: u16 =
    INLINE_ALBUM_ART_COLS + INLINE_ALBUM_ART_GAP + INLINE_ALBUM_ART_RIGHT_PAD;

fn inline_album_art_cache_key(album_id: &str) -> String {
    format!("{album_id}:P")
}

/// Computes the reserved-column art box: right-aligned within `area`
/// (leaving `INLINE_ALBUM_ART_RIGHT_PAD`), sized up to
/// `INLINE_ALBUM_ART_COLS`x`INLINE_ALBUM_ART_ROWS` (clamped to `area`).
/// Shared by the single-album inline-art path and the artist-header collage
/// so their outer geometry can't drift apart.
fn inline_art_box_rect(area: Rect) -> Rect {
    let box_w = INLINE_ALBUM_ART_COLS.min(area.width);
    let box_h = INLINE_ALBUM_ART_ROWS.min(area.height);
    Rect {
        x: area.x
            + area
                .width
                .saturating_sub(box_w + INLINE_ALBUM_ART_RIGHT_PAD),
        y: area.y,
        width: box_w,
        height: box_h,
    }
}

#[derive(Clone, Copy)]
enum ArtAnchorX {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy)]
enum ArtAnchorY {
    Top,
    Center,
    Bottom,
}

/// Places a `w`x`h` image within `container` anchored to the given corner/edge,
/// letterboxing the leftover margin to the opposite side(s). The single-album
/// art uses `(Right, Top)`; collage tiles anchor toward the box center so any
/// margin falls on the outer edges and the tiles abut with no internal seam.
fn align_art(container: Rect, w: u16, h: u16, ax: ArtAnchorX, ay: ArtAnchorY) -> Rect {
    let free_w = container.width.saturating_sub(w);
    let free_h = container.height.saturating_sub(h);
    let x = match ax {
        ArtAnchorX::Left => container.x,
        ArtAnchorX::Center => container.x + free_w / 2,
        ArtAnchorX::Right => container.x + free_w,
    };
    let y = match ay {
        ArtAnchorY::Top => container.y,
        ArtAnchorY::Center => container.y + free_h / 2,
        ArtAnchorY::Bottom => container.y + free_h,
    };
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

enum GroupedAlbumDisplayRow {
    ArtistHeader(ArtistHeaderSelection),
    AlbumDetailRule,
    Album(usize),
    /// Action-hint row shown directly under the selected album's title when
    /// it is *not* expanded into full track-selection mode (`AlbumDetailStart`
    /// covers the hint once expanded).
    AlbumActionHint,
    /// Action-hint row shown directly under a selected artist header.
    ArtistActionHint,
    AlbumDetailStart(usize),
    AlbumDetailContinuation,
    AlbumLoading,
}

struct GroupedAlbumDisplayPlan {
    order: Vec<usize>,
    rows: Vec<GroupedAlbumDisplayRow>,
    display_cursor: usize,
    selected_artist_header_valid: bool,
    /// Absolute (unscrolled) indices into `rows` of the selected album's
    /// framing `AlbumDetailRule` rows — `(top_rule_idx, bottom_rule_idx)`.
    /// `None` when the selected album has no colored-block framing (header
    /// is the actual focus, or the track cache resolved to an empty vec).
    selected_block_bounds: Option<(usize, usize)>,
}

impl GroupedAlbumDisplayRow {
    fn album_index(&self) -> Option<usize> {
        match self {
            Self::Album(idx) => Some(*idx),
            _ => None,
        }
    }

    fn row_target(&self, selectable_headers: bool) -> Option<PowerLeftRowTarget> {
        match self {
            Self::Album(idx) => Some(PowerLeftRowTarget::Album(*idx)),
            Self::ArtistHeader(selection) if selectable_headers => {
                Some(PowerLeftRowTarget::ArtistHeader(selection.clone()))
            }
            _ => None,
        }
    }
}

impl App {
    fn build_grouped_album_display_plan(
        &mut self,
        albums: &[mbv_core::api::MediaItem],
        cursor: usize,
        fetch_missing_tracks: bool,
        selectable_headers: bool,
        selected_artist_header: Option<&ArtistHeaderSelection>,
        expand_selected: bool,
    ) -> GroupedAlbumDisplayPlan {
        let mut album_info: Vec<(String, String, String)> = Vec::with_capacity(albums.len());
        for item in albums {
            let artist = self.resolve_group_album_artist(item);
            let (year_str, album_name) = if !item.artist.is_empty() {
                let year_str = if item.production_year > 0 {
                    item.production_year.to_string()
                } else {
                    String::new()
                };
                (year_str, item.display_name())
            } else if let Some((_, year, album)) = parse_album_folder_name(&item.name) {
                let year_str = if year > 0 {
                    year.to_string()
                } else {
                    String::new()
                };
                (year_str, album)
            } else {
                (String::new(), item.display_name())
            };
            album_info.push((artist, year_str, album_name));
        }

        let mut order: Vec<usize> = (0..album_info.len()).collect();
        order.sort_by_key(|&i| natural_sort_key(strip_article(&album_info[i].0)));

        // When an artist header itself is the focused row, no album beneath
        // it should still render as "selected" -- otherwise the album under
        // the cursor (which the header focus was entered from) keeps showing
        // its selected styling/hint/expansion alongside the header.
        let header_selected = selectable_headers && selected_artist_header.is_some();

        let inline_art_rows_after_album = if self.images_enabled() {
            INLINE_ALBUM_ART_ROWS.saturating_sub(1) as usize
        } else {
            0
        };

        let mut rows: Vec<GroupedAlbumDisplayRow> = Vec::new();
        let mut last_artist = String::new();
        let mut selected_block_bounds: Option<(usize, usize)> = None;
        for &idx in &order {
            let artist = &album_info[idx].0;
            if artist != &last_artist {
                let header_selection = ArtistHeaderSelection {
                    first_album_id: albums[idx].id.clone(),
                    artist_label: artist.clone(),
                };
                let this_header_selected =
                    header_selected && selected_artist_header == Some(&header_selection);
                if this_header_selected {
                    // Wrap the selected artist header in the same colored
                    // block frame as a selected album (see the `!expand_selected`
                    // album branch below): border space, bg padding, the
                    // header row itself, an action-hint row, filler rows so
                    // the block is tall enough for the collage, bg padding,
                    // border space.
                    rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // space for top border
                    let top_idx = rows.len();
                    rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // colored bg top padding
                    rows.push(GroupedAlbumDisplayRow::ArtistHeader(header_selection));
                    rows.push(GroupedAlbumDisplayRow::ArtistActionHint);
                    rows.extend(
                        std::iter::repeat_with(|| GroupedAlbumDisplayRow::AlbumDetailContinuation)
                            .take(inline_art_rows_after_album.saturating_sub(1)),
                    );
                    let bottom_idx = rows.len();
                    rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // colored bg bottom padding
                    rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // space for bottom border
                    selected_block_bounds = Some((top_idx, bottom_idx));
                } else {
                    rows.push(GroupedAlbumDisplayRow::ArtistHeader(header_selection));
                }
                last_artist = artist.clone();
            }
            if idx == cursor && header_selected {
                rows.push(GroupedAlbumDisplayRow::Album(idx));
            } else if idx == cursor && !expand_selected {
                // Hint-only state (album selected, tracks not yet shown): wrap in block frame
                // Insert extra detail rule rows for borders (one before, one after the colored block)
                rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // space for top border
                let top_idx = rows.len();
                rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // colored bg top padding
                rows.push(GroupedAlbumDisplayRow::Album(idx));
                rows.push(GroupedAlbumDisplayRow::AlbumActionHint);
                rows.extend(
                    std::iter::repeat_with(|| GroupedAlbumDisplayRow::AlbumDetailContinuation)
                        .take(inline_art_rows_after_album.saturating_sub(1)),
                );
                let bottom_idx = rows.len();
                rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // colored bg bottom padding
                rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // space for bottom border
                selected_block_bounds = Some((top_idx, bottom_idx));
            } else if idx == cursor {
                match self.album_tracks_cache.get(&albums[idx].id) {
                    Some(tracks) if !tracks.is_empty() => {
                        let detail_rows = (2 + tracks.len()).max(inline_art_rows_after_album);
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // space for top border
                        let top_idx = rows.len();
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // colored bg top padding
                        rows.push(GroupedAlbumDisplayRow::Album(idx));
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailStart(idx));
                        rows.extend(
                            std::iter::repeat_with(|| {
                                GroupedAlbumDisplayRow::AlbumDetailContinuation
                            })
                            .take(detail_rows.saturating_sub(1)),
                        );
                        let bottom_idx = rows.len();
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // colored bg bottom padding
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // space for bottom border
                        selected_block_bounds = Some((top_idx, bottom_idx));
                    }
                    Some(_) => rows.push(GroupedAlbumDisplayRow::Album(idx)),
                    None => {
                        if fetch_missing_tracks {
                            self.fetch_album_tracks(albums[idx].id.clone());
                        }
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // space for top border
                        let top_idx = rows.len();
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // colored bg top padding
                        rows.push(GroupedAlbumDisplayRow::Album(idx));
                        rows.push(GroupedAlbumDisplayRow::AlbumLoading);
                        rows.extend(
                            std::iter::repeat_with(|| {
                                GroupedAlbumDisplayRow::AlbumDetailContinuation
                            })
                            .take(inline_art_rows_after_album.saturating_sub(1)),
                        );
                        let bottom_idx = rows.len();
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // colored bg bottom padding
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailRule); // space for bottom border
                        selected_block_bounds = Some((top_idx, bottom_idx));
                    }
                }
            } else {
                rows.push(GroupedAlbumDisplayRow::Album(idx));
            }
        }

        let display_cursor = rows
            .iter()
            .position(|row| {
                selectable_headers
                    && matches!(
                        (row, selected_artist_header),
                        (
                            GroupedAlbumDisplayRow::ArtistHeader(selection),
                            Some(selected)
                        ) if selection == selected
                    )
            })
            .or_else(|| {
                rows.iter()
                    .position(|row| matches!(row, GroupedAlbumDisplayRow::Album(i) if *i == cursor))
            })
            .unwrap_or(0);
        let selected_artist_header_valid = selected_artist_header.is_some_and(|selected| {
            selectable_headers
                && rows.iter().any(|row| {
                    matches!(row, GroupedAlbumDisplayRow::ArtistHeader(selection) if selection == selected)
                })
        });

        GroupedAlbumDisplayPlan {
            order,
            rows,
            display_cursor,
            selected_artist_header_valid,
            selected_block_bounds,
        }
    }

    fn selected_power_music_artist_header(&self, lib_idx: usize) -> Option<ArtistHeaderSelection> {
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
        let target = plan.rows[selectable[new_pos]].row_target(true);
        drop(plan);
        match target {
            Some(PowerLeftRowTarget::ArtistHeader(selection)) => {
                self.set_artist_header_focus(lib_idx, selection);
            }
            Some(PowerLeftRowTarget::Album(idx)) => {
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
        );
        let target = if to_end {
            plan.rows.iter().rev().find_map(|row| row.row_target(true))
        } else {
            plan.rows.iter().find_map(|row| row.row_target(true))
        };
        drop(plan);
        match target {
            Some(PowerLeftRowTarget::ArtistHeader(selection)) => {
                self.set_artist_header_focus(lib_idx, selection);
            }
            Some(PowerLeftRowTarget::Album(idx)) => {
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
        if self.view_mode != crate::app::ViewMode::Power
            || self.power_left_tab != lib_idx + 1
            || !matches!(self.power_focus, crate::app::PowerFocus::Left)
            || self.libs[lib_idx].search.is_some()
            || self.libs[lib_idx].album_track_focus.is_some()
            || !self.is_viewing_album_folders(lib_idx)
        {
            return false;
        }

        let idle = self.list_image_fetches_allowed();
        self.last_nav_at = std::time::Instant::now();

        let Some(level) = self.libs[lib_idx].nav_stack.last() else {
            return false;
        };
        if level.items.is_empty() {
            return true;
        }

        let cursor = level.cursor;
        let albums = level.items.clone();
        let page = (self.layout.power.left_area.height as usize).max(1);
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
        );
        if selected.is_some() && !plan.selected_artist_header_valid {
            self.clear_artist_header_focus(lib_idx);
        }
        let target_row = if page_down {
            (plan.display_cursor + page).min(plan.rows.len().saturating_sub(1))
        } else {
            plan.display_cursor.saturating_sub(page)
        };
        let new_cursor = if page_down {
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

    pub(super) fn render_power_grouped_album_rows(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        albums: &[mbv_core::api::MediaItem],
        cursor: usize,
        stored_scroll: usize,
        focused: bool,
        layout: &mut LayoutPower,
    ) -> usize {
        let visible = area.height as usize;
        let avail = (area.width as usize).saturating_sub(2);
        let mut album_info: Vec<(String, String, String)> = Vec::with_capacity(albums.len());
        for item in albums {
            let artist = self.resolve_group_album_artist(item);
            let (year_str, album_name) = if !item.artist.is_empty() {
                let year_str = if item.production_year > 0 {
                    item.production_year.to_string()
                } else {
                    String::new()
                };
                (year_str, item.display_name())
            } else if let Some((_, year, album)) = parse_album_folder_name(&item.name) {
                let year_str = if year > 0 {
                    year.to_string()
                } else {
                    String::new()
                };
                (year_str, album)
            } else {
                (String::new(), item.display_name())
            };
            album_info.push((artist, year_str, album_name));
        }

        layout.inline_image_rect = None;

        let selected = self.selected_power_music_artist_header(lib_idx);
        let selectable_headers = self.is_music_group_view(lib_idx);
        // When an artist header is the focused row, the album under the
        // cursor must not also render as selected -- only one row group
        // (header or album) is ever the actual focus target at a time.
        let header_selected = selected.is_some();
        // Inline track expansion for the selected album: in the music-group
        // (pill selector) view, only expand once the user has pressed Enter
        // to enter track-selection mode (`album_track_focus`); elsewhere
        // (plain album-folder browsing) the existing always-expand behavior
        // is unchanged.
        let expand_selected = !selectable_headers || self.libs[lib_idx].album_track_focus.is_some();
        let plan = self.build_grouped_album_display_plan(
            albums,
            cursor,
            true,
            selectable_headers,
            selected.as_ref(),
            expand_selected,
        );
        if selected.is_some() && !plan.selected_artist_header_valid {
            self.clear_artist_header_focus(lib_idx);
        }
        layout.left_sorted_indices = plan.order.clone();
        let display_cursor = plan.display_cursor;
        let display_rows = plan.rows;
        let selected_block_bounds = plan.selected_block_bounds;
        let selected_art_reserved_w = if self.images_enabled()
            && selected_block_bounds.is_some()
            && area.width >= INLINE_ALBUM_ART_RESERVED + 20
        {
            INLINE_ALBUM_ART_RESERVED
        } else {
            0
        };
        let selected_art_abs_rows =
            selected_block_bounds.and_then(|(top_pad_abs, bottom_pad_abs)| {
                if selected_art_reserved_w == 0 {
                    return None;
                }
                let art_top = top_pad_abs + 1;
                let art_bottom = (art_top + INLINE_ALBUM_ART_ROWS as usize).min(bottom_pad_abs);
                (art_bottom > art_top).then_some((art_top, art_bottom))
            });
        let top_bound = selected_block_bounds
            .map(|(top, _)| top.saturating_sub(1)) // include border row
            .unwrap_or(display_cursor);
        let rows_below_album = selected_block_bounds
            .map(|(_, bottom_pad_abs)| (bottom_pad_abs + 1).saturating_sub(display_cursor))
            .unwrap_or(0);
        let lower = (display_cursor + rows_below_album)
            .saturating_sub(visible.saturating_sub(1))
            .min(top_bound);
        let offset = stored_scroll.clamp(lower, top_bound);

        // Paint the colored background block before rendering row content
        if let Some((top_pad_abs, bottom_pad_abs)) = selected_block_bounds {
            super::render_selected_block_background(
                f,
                area,
                offset,
                visible,
                top_pad_abs,
                bottom_pad_abs,
                palette::MEDIA_SELECTED_BG,
            );
        }

        let visible_rows: Vec<&GroupedAlbumDisplayRow> =
            display_rows.iter().skip(offset).take(visible).collect();
        for (row_idx, row) in visible_rows.iter().enumerate() {
            let row_area = Rect {
                x: area.x,
                y: area.y + row_idx as u16,
                width: area.width,
                height: 1,
            };
            let detail_indent = INLINE_ALBUM_DETAIL_INDENT.min(row_area.width);
            let title_indent =
                (INLINE_ALBUM_DETAIL_INDENT + INLINE_ALBUM_TITLE_EXTRA_INDENT).min(row_area.width);
            let detail_row_area = Rect {
                x: row_area.x + detail_indent,
                width: row_area.width.saturating_sub(detail_indent),
                ..row_area
            };
            let title_row_area = Rect {
                x: row_area.x + title_indent,
                width: row_area.width.saturating_sub(title_indent),
                ..row_area
            };
            let abs_row_idx = offset + row_idx;
            let reserve_art = selected_art_abs_rows.is_some_and(|(art_top, art_bottom)| {
                abs_row_idx >= art_top && abs_row_idx < art_bottom
            });
            let row_detail_area = if reserve_art {
                Rect {
                    width: detail_row_area
                        .width
                        .saturating_sub(selected_art_reserved_w),
                    ..detail_row_area
                }
            } else {
                detail_row_area
            };
            let row_title_area = if reserve_art {
                Rect {
                    width: title_row_area.width.saturating_sub(selected_art_reserved_w),
                    ..title_row_area
                }
            } else {
                title_row_area
            };
            match row {
                GroupedAlbumDisplayRow::ArtistHeader(selection) => {
                    let selected = selectable_headers
                        && self.libs[lib_idx]
                            .artist_header_focus
                            .as_ref()
                            .is_some_and(|focused| focused == selection);
                    // The block + bold label carry the focus signal (like a
                    // selected album); no green glyph here. When the collage
                    // art column is reserved for this row, only shrink the
                    // label's width -- its start column (`row_area.x`) must
                    // never move so the label doesn't shift on
                    // select/deselect.
                    let label_area = if reserve_art {
                        Rect {
                            width: row_area.width.saturating_sub(selected_art_reserved_w),
                            ..row_area
                        }
                    } else {
                        row_area
                    };
                    let label_avail = (label_area.width as usize).saturating_sub(1);
                    let artist_label = trunc_str(&selection.artist_label, label_avail);
                    let label_style = if selected && focused {
                        Style::default()
                            .fg(palette::YELLOW)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(palette::YELLOW)
                    };
                    let spans = vec![Span::raw(" "), Span::styled(artist_label, label_style)];
                    f.render_widget(Paragraph::new(Line::from(spans)), label_area);
                }
                GroupedAlbumDisplayRow::AlbumDetailRule => {
                    // Padding rows for the colored block; the background is painted separately.
                    // This row renders as empty, letting the background block show through.
                }
                GroupedAlbumDisplayRow::Album(idx) => {
                    let selected = *idx == cursor && !header_selected;
                    let (_, year_str, album_name) = &album_info[*idx];
                    let prefix_w = if year_str.is_empty() {
                        3
                    } else {
                        year_str.len() + 6
                    };
                    let name_w = avail.saturating_sub(prefix_w);
                    let trunc_name = trunc_str(album_name, name_w);
                    let fg = if focused {
                        palette::WHITE
                    } else {
                        palette::SUBTLE
                    };
                    let name_color = if selected && focused {
                        palette::IRIS
                    } else {
                        fg
                    };

                    // Detect if this album is inside a colored block frame
                    // Check the absolute row index (not the display cursor) to see if it's
                    // the first content row after the top border of the block
                    let has_block = selected
                        && selected_block_bounds.is_some_and(|(top_pad_abs, _)| {
                            // Album row comes immediately after the top AlbumDetailRule border
                            abs_row_idx == top_pad_abs + 1
                        });

                    let mut spans: Vec<Span> = Vec::new();
                    if has_block {
                        // Movie-style: 1-col leading pad, no ▌ marker
                        spans.push(Span::raw(" "));
                    } else if selected {
                        // Legacy style: ▌ AQUA marker
                        spans.push(super::selection_marker(true));
                    } else {
                        // Unselected: plain space
                        spans.push(Span::raw(" "));
                    }

                    if has_block {
                        // Movie-style: keep padding style consistent
                        // Year prefix: SUBTLE color, always shown when present
                        if !year_str.is_empty() {
                            spans.push(Span::styled("(", Style::default().fg(palette::SUBTLE)));
                            spans.push(Span::styled(
                                year_str.as_str(),
                                Style::default().fg(palette::AQUA),
                            ));
                            spans.push(Span::styled(") ", Style::default().fg(palette::SUBTLE)));
                        } else {
                            spans.push(Span::raw(" ")); // Spacing when no year
                        }
                        // Title: YELLOW, bold if focused
                        let title_style = if focused {
                            Style::default()
                                .fg(palette::YELLOW)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(palette::YELLOW)
                        };
                        spans.push(Span::styled(trunc_name, title_style));
                    } else {
                        // Legacy style spacing
                        if selected {
                            spans.push(Span::raw(" "));
                        } else if year_str.is_empty() {
                            spans.push(Span::raw("   "));
                        } else {
                            spans.push(Span::styled("   (", Style::default().fg(palette::SUBTLE)));
                        }
                        if selected && !year_str.is_empty() {
                            spans.push(Span::styled("(", Style::default().fg(palette::SUBTLE)));
                        }
                        if !year_str.is_empty() {
                            spans.push(Span::styled(
                                year_str.as_str(),
                                Style::default().fg(palette::AQUA),
                            ));
                            spans.push(Span::styled(") ", Style::default().fg(palette::SUBTLE)));
                        }
                        // Title styling
                        let title_style = if selected && focused {
                            Style::default().fg(name_color).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(name_color)
                        };
                        spans.push(Span::styled(trunc_name, title_style));
                    }

                    let album_area = if has_block { row_title_area } else { row_area };
                    f.render_widget(Paragraph::new(Line::from(spans)), album_area);
                }
                GroupedAlbumDisplayRow::AlbumActionHint => {
                    let hint_w = row_detail_area.width.saturating_sub(1) as usize;
                    let hint = trunc_str(
                        "^P: Play | ^A: Enqueue | ^S: Shuffle | ENTER: Show tracks",
                        hint_w,
                    );
                    f.render_widget(
                        Paragraph::new(Line::from(vec![
                            Span::raw(" "),
                            Span::styled(
                                hint.to_string(),
                                Style::default().fg(palette::SOFT_WHITE),
                            ),
                        ])),
                        row_title_area,
                    );
                }
                GroupedAlbumDisplayRow::ArtistActionHint => {
                    // Aligned under the header label: `row_area` with the
                    // same single-space lead as the label, not the
                    // album-title indent used by `AlbumActionHint`.
                    let hint_w = (row_area.width as usize).saturating_sub(1);
                    let hint = trunc_str("^P: Play | ^A: Enqueue | ^S: Shuffle", hint_w);
                    f.render_widget(
                        Paragraph::new(Line::from(vec![
                            Span::raw(" "),
                            Span::styled(
                                hint.to_string(),
                                Style::default().fg(palette::SOFT_WHITE),
                            ),
                        ])),
                        row_area,
                    );
                }
                GroupedAlbumDisplayRow::AlbumDetailStart(idx) => {
                    let height = visible_rows[row_idx..]
                        .iter()
                        .take_while(|r| {
                            matches!(
                                r,
                                GroupedAlbumDisplayRow::AlbumDetailStart(_)
                                    | GroupedAlbumDisplayRow::AlbumDetailContinuation
                            )
                        })
                        .count() as u16;
                    if let Some(tracks) = self.album_tracks_cache.get(&albums[*idx].id).cloned() {
                        let cursor = self.libs[lib_idx].album_track_focus.unwrap_or(0);
                        let detail_focused = self.libs[lib_idx].album_track_focus.is_some();
                        self.render_power_album_detail(
                            f,
                            Rect {
                                height,
                                ..row_title_area
                            },
                            &tracks,
                            cursor,
                            detail_focused,
                            false, // show_title: Album(idx) row above already shows it
                            false,
                            layout,
                        );
                    }
                }
                GroupedAlbumDisplayRow::AlbumLoading => {
                    f.render_widget(
                        Paragraph::new(Line::from(vec![
                            super::selection_marker(true),
                            Span::raw(" "),
                            Span::styled("Loading\u{2026}", Style::default().fg(palette::MUTED)),
                        ])),
                        row_detail_area,
                    );
                }
                GroupedAlbumDisplayRow::AlbumDetailContinuation => {}
            }
        }

        if self.libs[lib_idx].album_track_focus.is_none() {
            layout.cursor_screen_y = Some(area.y + (display_cursor.saturating_sub(offset)) as u16);
        }

        layout.left_row_map = display_rows
            .iter()
            .skip(offset)
            .take(visible)
            .map(|dr| match dr {
                GroupedAlbumDisplayRow::ArtistHeader(_)
                | GroupedAlbumDisplayRow::AlbumDetailRule => None,
                GroupedAlbumDisplayRow::Album(idx) => Some(*idx),
                GroupedAlbumDisplayRow::AlbumActionHint
                | GroupedAlbumDisplayRow::ArtistActionHint
                | GroupedAlbumDisplayRow::AlbumDetailStart(_)
                | GroupedAlbumDisplayRow::AlbumDetailContinuation
                | GroupedAlbumDisplayRow::AlbumLoading => None,
            })
            .collect();
        layout.left_row_targets = display_rows
            .iter()
            .skip(offset)
            .take(visible)
            .map(|dr| dr.row_target(selectable_headers))
            .collect();

        let display_n = display_rows.len();
        if focused && display_n > visible {
            let max_off = display_n.saturating_sub(visible);
            super::render_power_scrollbar(
                f,
                super::right_panel_scrollbar_area(area),
                max_off,
                offset,
            );
        }

        if let Some((art_top, art_bottom)) = selected_art_abs_rows {
            if art_top >= offset && art_top < offset + visible {
                let visible_bottom = art_bottom.min(offset + visible);
                let art_rect = Rect {
                    x: area.x,
                    y: area.y + (art_top - offset) as u16,
                    width: area.width,
                    height: (visible_bottom - art_top) as u16,
                };
                if let Some(selection) = &selected {
                    // Collage: the selected artist header's albums, in the
                    // already-sorted `left_sorted_indices` order, first 4.
                    let header_albums: Vec<mbv_core::api::MediaItem> = layout
                        .left_sorted_indices
                        .iter()
                        .filter(|&&idx| album_info[idx].0 == selection.artist_label)
                        .take(4)
                        .filter_map(|&idx| albums.get(idx).cloned())
                        .collect();
                    self.render_inline_artist_collage(f, art_rect, &header_albums, layout);
                } else if let Some(album) = albums.get(cursor) {
                    self.render_inline_album_art(f, art_rect, album, layout);
                }
            }
        }

        // Paint the ▁/▔ border rows around the colored block (after content/scrollbar)
        if let Some((top_pad_abs, bottom_pad_abs)) = selected_block_bounds {
            super::render_selected_block_borders(
                f,
                area,
                offset,
                visible,
                top_pad_abs,
                bottom_pad_abs,
            );
        }

        offset
    }

    fn render_inline_album_art(
        &mut self,
        f: &mut Frame,
        area: Rect,
        album: &mbv_core::api::MediaItem,
        layout: &mut LayoutPower,
    ) {
        if !self.images_enabled() || area.width < 4 || area.height < 2 {
            return;
        }

        let box_rect = inline_art_box_rect(area);
        let nav_gate_open = self.list_image_renders_allowed();
        let img_rect = self.render_inline_art_cell(
            f,
            box_rect,
            album,
            inline_album_art_cache_key(&album.id),
            nav_gate_open,
            false,
            (ArtAnchorX::Right, ArtAnchorY::Top),
        );
        layout.inline_image_rect = Some(img_rect);
    }

    /// Renders a 2x2 (or fewer) collage of an artist's album covers in
    /// `area`, for the selected artist header's block. Each tile is fetched
    /// center-cropped to a square (a `:sq`-suffixed cache key, distinct from
    /// the standalone album image) so the covers form an even grid.
    ///
    /// Fill behavior: 1 album fills the whole box; 2 split into left/right
    /// halves; 3+ use a 2x2 grid (top-left, top-right, bottom-left,
    /// bottom-right) with only the first 4 albums shown. When 3 albums are
    /// given, the 4th (bottom-right) cell is simply left unpainted, showing
    /// the selected-block background through.
    ///
    /// Each tile anchors toward the box center (e.g. the top-left tile pins its
    /// bottom-right corner) so the squares abut with no internal seam; any
    /// letterbox margin falls on the box's outer edges instead.
    fn render_inline_artist_collage(
        &mut self,
        f: &mut Frame,
        area: Rect,
        albums: &[mbv_core::api::MediaItem],
        layout: &mut LayoutPower,
    ) {
        if !self.images_enabled() || area.width < 4 || area.height < 2 || albums.is_empty() {
            return;
        }

        let box_rect = inline_art_box_rect(area);
        layout.inline_image_rect = Some(box_rect);

        // Each entry is `(cell, (anchor_x, anchor_y))`; anchors point toward the
        // box center so adjacent tiles meet at the seam.
        let cells: Vec<(Rect, (ArtAnchorX, ArtAnchorY))> = if albums.len() == 1 {
            vec![(box_rect, (ArtAnchorX::Center, ArtAnchorY::Center))]
        } else if albums.len() == 2 {
            let left_w = box_rect.width / 2;
            vec![
                (
                    Rect {
                        x: box_rect.x,
                        y: box_rect.y,
                        width: left_w,
                        height: box_rect.height,
                    },
                    (ArtAnchorX::Right, ArtAnchorY::Center),
                ),
                (
                    Rect {
                        x: box_rect.x + left_w,
                        y: box_rect.y,
                        width: box_rect.width - left_w,
                        height: box_rect.height,
                    },
                    (ArtAnchorX::Left, ArtAnchorY::Center),
                ),
            ]
        } else {
            let half_w = box_rect.width / 2;
            let half_h = box_rect.height / 2;
            vec![
                (
                    Rect {
                        x: box_rect.x,
                        y: box_rect.y,
                        width: half_w,
                        height: half_h,
                    },
                    (ArtAnchorX::Right, ArtAnchorY::Bottom),
                ),
                (
                    Rect {
                        x: box_rect.x + half_w,
                        y: box_rect.y,
                        width: box_rect.width - half_w,
                        height: half_h,
                    },
                    (ArtAnchorX::Left, ArtAnchorY::Bottom),
                ),
                (
                    Rect {
                        x: box_rect.x,
                        y: box_rect.y + half_h,
                        width: half_w,
                        height: box_rect.height - half_h,
                    },
                    (ArtAnchorX::Right, ArtAnchorY::Top),
                ),
                (
                    Rect {
                        x: box_rect.x + half_w,
                        y: box_rect.y + half_h,
                        width: box_rect.width - half_w,
                        height: box_rect.height - half_h,
                    },
                    (ArtAnchorX::Left, ArtAnchorY::Top),
                ),
            ]
        };

        let nav_gate_open = self.list_image_renders_allowed();
        for ((cell, anchor), album) in cells.iter().zip(albums.iter().take(4)) {
            self.render_inline_art_cell(
                f,
                *cell,
                album,
                format!("{}:sq", album.id),
                nav_gate_open,
                true,
                *anchor,
            );
        }
    }

    /// Fetches + renders a single album cover into `cell`, falling back to the
    /// `OVERLAY` loading placeholder while the image isn't yet decoded/gated.
    /// Returns the rect actually painted (image or placeholder). Shared by the
    /// single-album art path and each quadrant of the collage.
    ///
    /// When `square` is set, the cover is fetched center-cropped to a square
    /// (via `fetch_card_image_square`) — the collage mode, giving uniform grid
    /// tiles; otherwise the natural-aspect cover is fetched. Placement within
    /// `cell` follows `anchor` (the standalone path uses `(Right, Top)`;
    /// collage tiles anchor toward the box center so they abut).
    fn render_inline_art_cell(
        &mut self,
        f: &mut Frame,
        cell: Rect,
        album: &mbv_core::api::MediaItem,
        cache_key: String,
        nav_gate_open: bool,
        square: bool,
        anchor: (ArtAnchorX, ArtAnchorY),
    ) -> Rect {
        if cell.width == 0 || cell.height == 0 {
            return cell;
        }

        if square {
            self.fetch_card_image_square(
                cache_key.clone(),
                album.id.clone(),
                album.series_id.clone(),
                super::MUSIC_ALBUM_IMAGE_TYPES,
            );
        } else {
            self.fetch_card_image(
                cache_key.clone(),
                album.id.clone(),
                album.series_id.clone(),
                super::MUSIC_ALBUM_IMAGE_TYPES,
            );
        }

        let mut img_rect = cell;
        let mut use_placeholder = true;

        if nav_gate_open {
            if let Some(Some(state)) = self.card_image_states.get_mut(&cache_key) {
                if let Some(actual) = state.size_for(
                    ratatui_image::Resize::Scale(Some(super::POWER_RENDER_FILTER)),
                    ratatui::layout::Size {
                        width: cell.width,
                        height: cell.height,
                    },
                ) {
                    img_rect = align_art(cell, actual.width, actual.height, anchor.0, anchor.1);
                    use_placeholder = false;
                }
            }
        }

        if use_placeholder {
            f.render_widget(
                Block::default().style(Style::default().bg(palette::OVERLAY)),
                img_rect,
            );
        } else if let Some(Some(state)) = self.card_image_states.get_mut(&cache_key) {
            type SImg = ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
            f.render_stateful_widget(
                SImg::default().resize(ratatui_image::Resize::Scale(Some(
                    super::POWER_RENDER_FILTER,
                ))),
                img_rect,
                state,
            );
        }

        img_rect
    }

    /// Renders the music album detail panel (track list) into `area` — the lib
    /// slot below the card. The card itself already shows the album art (handled
    /// in `render_power_card`). Mirrors `render_power_compact_detail` for movies.
    ///
    /// Takes `items`/`cursor` explicitly rather than reading `nav_stack`
    /// internally (#145) so it can render either the legacy drilled-in
    /// nav_stack level or the inline-album-detail cache (the currently
    /// highlighted album in the album-folder listing, fetched proactively
    /// via `fetch_album_tracks`) with the same code path.
    pub(super) fn render_power_album_detail(
        &mut self,
        f: &mut Frame,
        area: Rect,
        items: &[mbv_core::api::MediaItem],
        cursor: usize,
        focused: bool,
        show_title: bool,
        selected_region_gutter: bool,
        layout: &mut LayoutPower,
    ) {
        if area.height == 0 {
            return;
        }

        let n = items.len();
        if items.is_empty() {
            return;
        }
        let gutter_w = if selected_region_gutter { 2 } else { 1 };
        let max_y = area.y + area.height;
        let mut row = area.y;

        // — Album title (only when no separate row already shows it — the
        // drilled-in single-pane view has no Album(idx) row above this, unlike
        // the inline/grouped call site) —
        if show_title && row < max_y {
            let album_title = items[0].album.clone();
            let title = trunc_str(&album_title, (area.width as usize).saturating_sub(1));
            let title_style = if focused {
                Style::default()
                    .fg(palette::YELLOW)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette::YELLOW)
            };
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(format!(" {title}"), title_style))),
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
            );
            row += 1;
        }

        // — Inline album actions / spacer row —
        if row < max_y {
            if selected_region_gutter {
                let hint_w = (area.width as usize).saturating_sub(gutter_w);
                // `focused` mirrors `album_track_focus.is_some()` at the call
                // site: once track-selection mode is entered, swap the
                // "show tracks" hint for the exit hint.
                let trailing_hint = if focused {
                    "BACK: Exit"
                } else {
                    "ENTER: Show tracks"
                };
                let hint = trunc_str(
                    &format!("^P: Play | ^A: Enqueue | ^S: Shuffle | {trailing_hint}"),
                    hint_w,
                );
                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        super::selection_marker(true),
                        Span::raw(" "),
                        Span::styled(hint.to_string(), Style::default().fg(palette::MUTED)),
                    ])),
                    Rect {
                        x: area.x,
                        y: row,
                        width: area.width,
                        height: 1,
                    },
                );
                row += 1;
                if row + 1 < max_y {
                    f.render_widget(
                        Paragraph::new(Line::from(vec![
                            super::selection_marker(true),
                            Span::raw(" "),
                        ])),
                        Rect {
                            x: area.x,
                            y: row,
                            width: area.width,
                            height: 1,
                        },
                    );
                    row += 1;
                }
            }
            if !selected_region_gutter {
                let trailing_hint = if focused {
                    "BACK: Exit"
                } else {
                    "ENTER: Show tracks"
                };
                let hint = trunc_str(
                    &format!("^P: Play | ^A: Enqueue | ^S: Shuffle | {trailing_hint}"),
                    area.width as usize,
                );
                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::raw(" "),
                        Span::styled(hint.to_string(), Style::default().fg(palette::SOFT_WHITE)),
                    ])),
                    Rect {
                        x: area.x,
                        y: row,
                        width: area.width,
                        height: 1,
                    },
                );
                row += 2;
            }
        }

        // — Scrollable track list —
        let track_indent = if selected_region_gutter {
            0
        } else {
            INLINE_ALBUM_TRACK_EXTRA_INDENT
                .saturating_sub(INLINE_ALBUM_TITLE_EXTRA_INDENT)
                .min(area.width)
        };
        let table_area = Rect {
            x: area.x + track_indent,
            y: row,
            width: area.width.saturating_sub(track_indent),
            height: max_y.saturating_sub(row),
        };
        if table_area.height == 0 {
            return;
        }

        let playback = self.effective_playback_state();
        let now_playing_id: Option<String> = if playback.active {
            self.playback_queue()
                .items
                .get(playback.active_idx)
                .map(|i| i.id.clone())
        } else {
            None
        };

        let show_length = table_area.width > 40;
        let dur_col_w: usize = if show_length { 7 } else { 0 };
        let title_col_w = (table_area.width as usize)
            .saturating_sub(gutter_w + if show_length { dur_col_w + 1 } else { 0 });

        let rows: Vec<Row> = items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_cursor = i == cursor;
                let is_playing = now_playing_id.as_deref() == Some(item.id.as_str());
                let row_style = if selected_region_gutter && is_cursor && focused {
                    let mut style = Style::default().fg(palette::YELLOW);
                    if is_playing {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    style
                } else if is_playing {
                    Style::default()
                        .fg(palette::GREEN)
                        .add_modifier(Modifier::BOLD)
                } else if is_cursor && focused {
                    Style::default().fg(palette::YELLOW)
                } else if focused {
                    Style::default().fg(palette::WHITE)
                } else {
                    Style::default().fg(palette::SUBTLE)
                };
                let marker =
                    super::selection_marker(selected_region_gutter || (is_cursor && focused));
                let track_num = if item.index_number > 0 {
                    format!("{}. ", item.index_number)
                } else {
                    format!("{}. ", i + 1)
                };
                let mut title_spans = vec![marker];
                if selected_region_gutter {
                    title_spans.push(Span::raw(" "));
                }
                let num_w = track_num.chars().count();
                let title = trunc_str(&item.name, title_col_w.saturating_sub(num_w));
                title_spans.push(Span::styled(
                    track_num,
                    Style::default().fg(palette::SUBTLE),
                ));
                title_spans.push(Span::raw(title));
                let title_cell = Cell::from(Line::from(title_spans));
                let len_secs = item.runtime_ticks / TICKS_PER_SECOND;
                let length = if len_secs > 0 {
                    fmt_duration_approx(len_secs)
                } else {
                    "\u{2014}".to_string()
                };
                if show_length {
                    Row::new([
                        title_cell,
                        Cell::from(Line::from(length).alignment(Alignment::Right))
                            .style(Style::default().fg(palette::SUBTLE)),
                        Cell::from(""),
                    ])
                    .style(row_style)
                } else {
                    Row::new([title_cell, Cell::from(""), Cell::from("")]).style(row_style)
                }
            })
            .collect();

        let mut state = TableState::default();
        state.select(Some(cursor));
        let table = Table::new(
            rows,
            [
                Constraint::Min(10),
                Constraint::Length(if show_length { dur_col_w as u16 } else { 0 }),
                Constraint::Length(1),
            ],
        )
        .column_spacing(1)
        .row_highlight_style(Style::default());
        f.render_stateful_widget(table, table_area, &mut state);
        layout.cursor_screen_y =
            Some(table_area.y + (cursor.saturating_sub(state.offset())) as u16);

        let visible_rows = table_area.height as usize;
        if !selected_region_gutter && n > visible_rows {
            let max_offset = n.saturating_sub(visible_rows);
            super::render_power_scrollbar(
                f,
                super::right_panel_scrollbar_area(table_area),
                max_offset,
                state.offset(),
            );
        }
    }
}
