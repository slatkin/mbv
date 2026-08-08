use super::album_art::{INLINE_ALBUM_ART_RESERVED, INLINE_ALBUM_ART_ROWS};
use super::album_plan::GroupedAlbumDisplayRow;
use super::album_rows::AlbumRowCtx;
use crate::app::layout::LayoutMain;
use crate::app::{palette, App};
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use textwrap::wrap;

/// Inset of the TRACK_BLOCK_BG box from the row's own bounds, on each side.
const TRACK_BLOCK_MARGIN: u16 = 2;
/// Further inset of the track text/cursor-highlight from the block's edge,
/// on each side -- kept as its own constant so the "track text sits inside
/// the block" relationship stays explicit instead of two independently
/// hand-picked offsets.
const TRACK_TEXT_MARGIN: u16 = 2;

impl App {
    pub(super) fn render_grouped_album_rows(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        albums: &[mbv_core::api::EmbyItem],
        cursor: usize,
        stored_scroll: usize,
        focused: bool,
        hero_handles_detail: bool,
        cols: u16,
        layout: &mut LayoutMain,
    ) -> usize {
        let visible = area.height as usize;
        let avail = (area.width as usize).saturating_sub(2);
        let (album_info, order) = {
            let catalog = self.libs[lib_idx]
                .nav_stack
                .last()
                .and_then(|l| l.music_grouping.as_ref())
                .and_then(|s| s.settled.as_ref());
            match catalog {
                Some(cat) => {
                    let info = self.group_album_info(albums, Some(cat));
                    let order: Vec<usize> = cat
                        .entries
                        .iter()
                        .map(|e| e.album_index)
                        .filter(|&i| i < albums.len())
                        .collect();
                    (info, order)
                }
                None => {
                    let info = self.group_album_info(albums, None);
                    let order = super::sorted_group_album_order(&info);
                    (info, order)
                }
            }
        };

        layout.inline_image_rect = None;

        let selected = self.selected_music_artist_header(lib_idx);
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
            &album_info,
            &order,
            cursor,
            true,
            selectable_headers,
            selected.as_ref(),
            expand_selected,
            Some((
                area.width,
                if self.images_enabled() && area.width >= INLINE_ALBUM_ART_RESERVED + 20 {
                    INLINE_ALBUM_ART_RESERVED
                } else {
                    0
                },
            )),
            hero_handles_detail,
        );
        if selected.is_some() && !plan.selected_artist_header_valid {
            self.clear_artist_header_focus(lib_idx);
        }
        layout.left_sorted_indices = plan.order.clone();
        let display_cursor = plan.display_cursor;
        let display_rows = plan.rows;
        let selected_block_bounds = plan.selected_block_bounds;
        let track_detail_bounds = plan.track_detail_bounds;
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
            .map(|(top, _)| top.saturating_sub(1))
            .unwrap_or(display_cursor);
        let rows_below_block = selected_block_bounds
            .map(|(_, bottom_pad_abs)| (bottom_pad_abs + 1).saturating_sub(display_cursor))
            .unwrap_or(0);
        let lower_bound = (display_cursor + rows_below_block)
            .saturating_sub(visible.saturating_sub(1))
            .min(top_bound);
        let offset = stored_scroll.clamp(lower_bound, top_bound);

        // Build screen-row-indexed left_item_rows for column-packing-aware
        // mouse hit-testing. The rendering loop below packs display rows into
        // screen rows; left_item_rows mirrors that with one entry per screen
        // row. left_screen_offset lets the mouse handler index correctly.
        let (screen_offset, total_screen_rows) = {
            let cn = cols.max(1) as usize;
            let mut d2s = vec![0usize; display_rows.len()];
            let mut screen = 0usize;
            let mut gai = 0usize;
            let mut sr: Vec<Vec<usize>> = vec![Vec::new()];
            for (di, row) in display_rows.iter().enumerate() {
                d2s[di] = screen;
                match row {
                    GroupedAlbumDisplayRow::Album(idx) => {
                        let col = gai % cn;
                        gai += 1;
                        while sr[screen].len() <= col {
                            sr[screen].push(0);
                        }
                        sr[screen][col] = *idx;
                        if (col + 1).is_multiple_of(cn) {
                            screen += 1;
                            sr.push(Vec::new());
                        }
                    }
                    GroupedAlbumDisplayRow::AlbumWrappedContinuation if cn > 1 => {}
                    GroupedAlbumDisplayRow::ArtistHeader(_)
                    | GroupedAlbumDisplayRow::ArtistGroupSpacer => {
                        gai = 0;
                        screen += 1;
                        sr.push(Vec::new());
                    }
                    _ => {
                        screen += 1;
                        sr.push(Vec::new());
                    }
                }
            }
            while sr.last().is_some_and(|r| r.is_empty()) && sr.len() > 1 {
                sr.pop();
            }
            let so = d2s.get(offset).copied().unwrap_or(0);
            // Cursor screen Y from the packed mapping
            if self.libs[lib_idx].album_track_focus.is_none() {
                if let Some(&cs) = d2s.get(display_cursor) {
                    if cs >= so {
                        layout.cursor_screen_y = Some(area.y + (cs - so) as u16);
                    }
                }
            }
            // Visible-slice row map and targets indexed by screen row
            let vs = visible.min(sr.len().saturating_sub(so));
            layout.left_row_map = (so..).take(vs).map(|i| sr[i].first().copied()).collect();
            layout.left_row_targets = (0..vs)
                .map(|vi| {
                    let i = so + vi;
                    (0..display_rows.len())
                        .find(|&d| d2s[d] == i)
                        .and_then(|d| display_rows[d].row_target(selectable_headers))
                })
                .collect();
            let total = sr.len();
            layout.left_item_rows = sr;
            layout.left_screen_offset = so;
            (so, total)
        };

        // Paint the colored background block before rendering row content
        if let Some((top_pad_abs, bottom_pad_abs)) = selected_block_bounds {
            let bg = if focused {
                palette::MEDIA_SELECTED_BG
            } else {
                palette::PLAYBACK_PANEL_BG
            };
            super::render_selected_block_background(
                f,
                area,
                offset,
                visible,
                top_pad_abs,
                bottom_pad_abs,
                bg,
            );
        }

        // Paint the track detail block background
        if let Some((track_start, track_end)) = track_detail_bounds {
            let vis_top = track_start.max(offset);
            let vis_bot = (track_end.saturating_sub(1)).min(offset + visible.saturating_sub(1));
            if vis_top <= vis_bot {
                let block_y = area.y + (vis_top - offset) as u16;
                let block_h = (vis_bot - vis_top + 1) as u16;
                let block_x = area.x + TRACK_BLOCK_MARGIN;
                let block_w = area
                    .width
                    .saturating_sub(2 * TRACK_BLOCK_MARGIN)
                    .saturating_sub(selected_art_reserved_w);
                f.render_widget(
                    Block::default().style(Style::default().bg(palette::TRACK_BLOCK_BG)),
                    Rect {
                        x: block_x,
                        y: block_y,
                        width: block_w,
                        height: block_h,
                    },
                );
            }
        }

        let visible_rows: Vec<&GroupedAlbumDisplayRow> =
            display_rows.iter().skip(offset).take(visible).collect();

        // Two-column packing state: track album position within artist groups
        // to pack consecutive Album rows into columns.
        let cn = cols.max(1) as usize;
        let mut current_y = 0u16;
        let mut group_album_idx = 0usize;

        // Produce the area rect for a full-width row (headers, spacers, and
        // non-album filler rows). The `Album` branch replaces the width/height
        // and potentially the x coordinate.
        let full_row_rect = |screen_y: u16| Rect {
            x: area.x,
            y: area.y + screen_y,
            width: area.width,
            height: 1,
        };

        for (row_idx, row) in visible_rows.iter().enumerate() {
            let abs_row_idx = offset + row_idx;

            // Determine if this row should start a new terminal row or continue
            // in the current row (for two-column packing).
            let (row_area, advance_after) = match row {
                GroupedAlbumDisplayRow::Album(_) => {
                    let col = group_album_idx % cn;
                    let col_width = area.width / cn as u16;
                    let col_x = area.x + (col as u16 * col_width);
                    // Last column gets remaining width to avoid rounding errors
                    let actual_width = if col == cn - 1 {
                        area.width.saturating_sub(col as u16 * col_width)
                    } else {
                        col_width
                    };

                    let row_area = Rect {
                        x: col_x,
                        y: area.y + current_y,
                        width: actual_width,
                        height: 1,
                    };
                    let advance_after = (col + 1).is_multiple_of(cn);
                    group_album_idx += 1;
                    (row_area, advance_after)
                }
                GroupedAlbumDisplayRow::ArtistHeader(_)
                | GroupedAlbumDisplayRow::ArtistGroupSpacer => {
                    // These always get full width and start a new row
                    group_album_idx = 0;
                    (full_row_rect(current_y), true)
                }
                GroupedAlbumDisplayRow::AlbumWrappedContinuation => {
                    // Phantom title-wrap rows: skip advancing Y in
                    // multi-column mode so paired albums stay on the same
                    // screen row.
                    (full_row_rect(current_y), cn == 1)
                }
                _ => {
                    // Other rows (AlbumDetailRule, etc.)
                    // get full width and start a new row
                    (full_row_rect(current_y), true)
                }
            };

            match row {
                GroupedAlbumDisplayRow::ArtistHeader(selection) => {
                    self.render_artist_header_row(
                        f,
                        row_area,
                        selection,
                        selectable_headers,
                        selected_block_bounds,
                        abs_row_idx,
                        selected_art_reserved_w,
                        focused,
                        lib_idx,
                    );
                }
                GroupedAlbumDisplayRow::ArtistGroupSpacer => {}
                GroupedAlbumDisplayRow::AlbumDetailRule => {
                    // Padding rows for the colored block; the background is painted separately.
                    // This row renders as empty, letting the background block show through.
                }
                GroupedAlbumDisplayRow::AlbumWrappedContinuation => {}
                GroupedAlbumDisplayRow::Album(idx) => {
                    self.render_album_row(
                        f,
                        AlbumRowCtx {
                            row_area,
                            idx: *idx,
                            album_info: &album_info,
                            cursor,
                            header_selected,
                            avail,
                            selected_block_bounds,
                            selectable_headers,
                            abs_row_idx,
                            selected_art_reserved_w,
                            focused,
                        },
                    );
                }
                GroupedAlbumDisplayRow::AlbumActionHint => {
                    self.render_album_action_hint(
                        f,
                        row_area,
                        selectable_headers,
                        selected_block_bounds,
                        abs_row_idx,
                        selected_art_reserved_w,
                        lib_idx,
                        focused,
                    );
                }
                GroupedAlbumDisplayRow::ArtistActionHint => {
                    Self::render_artist_action_hint(
                        f,
                        row_area,
                        selectable_headers,
                        selected_block_bounds,
                        abs_row_idx,
                        selected_art_reserved_w,
                        focused,
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
                        let track_area = Rect {
                            x: row_area.x + TRACK_BLOCK_MARGIN + TRACK_TEXT_MARGIN,
                            y: row_area.y,
                            width: row_area
                                .width
                                .saturating_sub(2 * (TRACK_BLOCK_MARGIN + TRACK_TEXT_MARGIN))
                                .saturating_sub(selected_art_reserved_w),
                            height,
                        };
                        if detail_focused && height > 0 {
                            let scroll_offset = cursor.saturating_sub(height as usize - 1);
                            f.render_widget(
                                Block::default().style(Style::default().bg(palette::BG_GREEN)),
                                Rect {
                                    x: row_area.x + TRACK_BLOCK_MARGIN,
                                    y: row_area.y + cursor.saturating_sub(scroll_offset) as u16,
                                    width: row_area
                                        .width
                                        .saturating_sub(2 * TRACK_BLOCK_MARGIN)
                                        .saturating_sub(selected_art_reserved_w),
                                    height: 1,
                                },
                            );
                        }
                        self.render_album_detail(
                            f,
                            track_area,
                            &tracks,
                            cursor,
                            detail_focused,
                            false, // show_title: Album(idx) row above already shows it
                            false,
                            true,
                            false, // show_hint: AlbumActionHint row at top already shows it
                            0,     // art_reserved_w: already accounted for in track_area
                            Some(row_area.x + TRACK_BLOCK_MARGIN),
                            layout,
                        );
                    }
                }
                GroupedAlbumDisplayRow::AlbumLoading => {
                    let loading = "Loading…";
                    let loading_width = row_area
                        .width
                        .saturating_sub(selected_art_reserved_w)
                        .saturating_sub(2)
                        .max(1) as usize;
                    let loading_lines: Vec<Line> = wrap(loading, loading_width)
                        .into_iter()
                        .map(|line| {
                            Line::from(vec![
                                super::selection_marker(true),
                                Span::raw(" "),
                                Span::styled(
                                    line.into_owned(),
                                    Style::default().fg(palette::MUTED),
                                ),
                            ])
                        })
                        .collect();
                    f.render_widget(
                        Paragraph::new(loading_lines.clone()),
                        Rect {
                            width: row_area.width.saturating_sub(selected_art_reserved_w),
                            height: loading_lines.len() as u16,
                            ..row_area
                        },
                    );
                }
                GroupedAlbumDisplayRow::AlbumDetailContinuation => {}
            }

            // Advance y after rendering if this row completes a column group
            if advance_after {
                current_y += 1;
            }
        }

        if focused && total_screen_rows > visible {
            let max_off = total_screen_rows.saturating_sub(visible);
            super::render_right_scrollbar(f, area, max_off, screen_offset);
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
                    let header_albums: Vec<mbv_core::api::EmbyItem> = layout
                        .left_sorted_indices
                        .iter()
                        .filter(|&&idx| album_info[idx].0 == selection.artist_label)
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
}
