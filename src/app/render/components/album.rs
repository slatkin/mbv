use crate::app::layout::LayoutMain;
use crate::app::render::components::album_art::{
    MusicImagePaint, INLINE_ALBUM_ART_RESERVED, INLINE_ALBUM_ART_ROWS,
};
use crate::app::render::components::album_rows::AlbumRowCtx;
use crate::app::render::components::list_rows::{
    draw_column_selection_markers, selected_cell_rect,
};
use crate::app::render::screens::album_plan::{
    group_album_info, GroupedAlbumDisplayRow, HeaderFocusCtx,
};
use crate::app::{palette, App};
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use std::collections::HashMap;
use textwrap::wrap;

/// Inset of the SURFACE_ACCENT_SOFT box from the row's own bounds, on each side.
const TRACK_BLOCK_MARGIN: u16 = 2;
/// Further inset of the track text/cursor-highlight from the block's edge,
/// on each side -- kept as its own constant so the "track text sits inside
/// the block" relationship stays explicit instead of two independently
/// hand-picked offsets.
const TRACK_TEXT_MARGIN: u16 = 2;

/// The cursor position and last-stored scroll offset for a grouped-album
/// rows render pass -- travel together since the scroll offset is always
/// clamped relative to the cursor's row.
pub(in crate::app::render) struct AlbumRowsCursorCtx {
    pub(in crate::app::render) cursor: usize,
    pub(in crate::app::render) stored_scroll: usize,
}

/// Explicit render inputs for the grouped-album rows painter, gathered by the
/// thin App-backed wrapper so the painter itself never reaches `App`.
pub(in crate::app::render) struct GroupedAlbumRenderCtx<'a> {
    /// `(artist, year, album_name)` display triples for every album.
    pub(in crate::app::render) album_info: Vec<(String, String, String)>,
    /// Album display order (settled catalog order, or the sorted fallback).
    pub(in crate::app::render) order: Vec<usize>,
    pub(in crate::app::render) in_music_group_view: bool,
    pub(in crate::app::render) playing_track_id: Option<String>,
    pub(in crate::app::render) images_enabled: bool,
    pub(in crate::app::render) album_tracks: &'a HashMap<String, Vec<mbv_core::api::EmbyItem>>,
}

/// Thin App-backed wrapper: pre-warms neighbouring album art (task 3.7 keeps
/// this in the shell), gathers the explicit render context, runs the App-free
/// painter, and executes the returned `MusicImagePaint` for the selected
/// album's hero art.
// TODO(interactive-surface-ledger): delete with legacy album rows (pinned by music characterization tests).
#[allow(dead_code)]
pub(in crate::app::render) fn render_grouped_album_rows(
    app: &mut App,
    f: &mut Frame,
    area: Rect,
    lib_idx: usize,
    albums: &[mbv_core::api::EmbyItem],
    cursor_ctx: AlbumRowsCursorCtx,
    focused: bool,
    hero_handles_detail: bool,
    cols: u16,
    layout: &mut LayoutMain,
) -> usize {
    let cursor = cursor_ctx.cursor;
    let (album_info, order) = {
        let catalog = app.libs[lib_idx]
            .nav_stack
            .last()
            .and_then(|l| l.music_grouping.as_ref())
            .and_then(|s| s.settled.as_ref());
        match catalog {
            Some(cat) => {
                let info = group_album_info(&app.album_artist_cache, albums, Some(cat));
                let order: Vec<usize> = cat
                    .entries
                    .iter()
                    .map(|e| e.album_index)
                    .filter(|&i| i < albums.len())
                    .collect();
                (info, order)
            }
            None => {
                let info = group_album_info(&app.album_artist_cache, albums, None);
                let order = crate::app::render::sorted_group_album_order(&info);
                (info, order)
            }
        }
    };

    // Pre-warm nearby album art while the cursor is idle so scrolling does
    // not make each neighbour wait for its image fetch. Use display order
    // rather than the source item order because grouped music lists can be
    // sorted by artist and album independently of the API response.
    if app.images_enabled() {
        if let Some(album) = albums.get(cursor) {
            app.fetch_card_image(
                crate::app::render::components::album_art::inline_album_art_cache_key(&album.id),
                album.id.clone(),
                album.series_id.clone(),
                crate::app::render::MUSIC_ALBUM_IMAGE_TYPES,
            );
        }
        const PREFETCH_AHEAD: usize = 3;
        const PREFETCH_BEHIND: usize = 1;
        if let Some(cursor_pos) = order.iter().position(|&idx| idx == cursor) {
            let start = cursor_pos.saturating_sub(PREFETCH_BEHIND);
            let end = (cursor_pos + PREFETCH_AHEAD + 1).min(order.len());
            for (offset, &idx) in order[start..end].iter().enumerate() {
                if start + offset == cursor_pos {
                    continue;
                }
                let Some(album) = albums.get(idx) else {
                    continue;
                };
                app.fetch_list_card_image_when_idle(
                    crate::app::render::components::album_art::inline_album_art_cache_key(
                        &album.id,
                    ),
                    album.id.clone(),
                    album.series_id.clone(),
                    crate::app::render::MUSIC_ALBUM_IMAGE_TYPES,
                );
            }
        }
    }

    let in_music_group_view = app.is_music_group_view(lib_idx);
    let playback = app.effective_playback_state();
    let playing_track_id = if playback.active {
        app.playback_queue()
            .emby_item_at(playback.active_idx)
            .map(|item| item.id.clone())
    } else {
        None
    };
    let images_enabled = app.images_enabled();

    let ctx = GroupedAlbumRenderCtx {
        album_info,
        order,
        in_music_group_view,
        playing_track_id,
        images_enabled,
        album_tracks: &app.album_tracks_cache,
    };
    let (offset, image_paint) = render_grouped_album_rows_with_ctx(
        f,
        area,
        albums,
        cursor_ctx,
        focused,
        hero_handles_detail,
        cols,
        layout,
        ctx,
    );
    app.paint_music_image(f, image_paint);
    offset
}

pub(in crate::app::render) fn render_grouped_album_rows_with_ctx(
    f: &mut Frame,
    area: Rect,
    albums: &[mbv_core::api::EmbyItem],
    cursor_ctx: AlbumRowsCursorCtx,
    focused: bool,
    hero_handles_detail: bool,
    cols: u16,
    layout: &mut LayoutMain,
    ctx: GroupedAlbumRenderCtx<'_>,
) -> (usize, Option<MusicImagePaint>) {
    let GroupedAlbumRenderCtx {
        album_info,
        order,
        in_music_group_view,
        playing_track_id,
        images_enabled,
        album_tracks,
    } = ctx;
    layout.wide_music_track_hitmap.clear();
    let AlbumRowsCursorCtx {
        cursor,
        stored_scroll,
    } = cursor_ctx;
    let visible = area.height as usize;
    let avail = (area.width as usize).saturating_sub(2);
    // Inline track expansion for the selected album. Narrow keeps inline
    // track focus explicitly off (`MusicWorkspaceComponent` is mounted
    // there with inline track focus disabled, and narrow activation opens
    // the selection modal), so in the music-group (pill selector) view the
    // expansion is never entered from this renderer; elsewhere (plain
    // album-folder browsing) the existing always-expand behavior is
    // unchanged.
    let expand_selected = !in_music_group_view;
    let plan_ctx = crate::app::render::screens::album_plan::GroupedAlbumDisplayPlanCtx {
        images_enabled,
        playing_track_id: playing_track_id.clone(),
        album_tracks,
    };
    let mut plan =
        crate::app::render::screens::album_plan::build_grouped_album_display_plan_with_ctx(
            albums,
            &album_info,
            &order,
            cursor,
            true,
            HeaderFocusCtx {
                in_music_group_view,
                expand_selected,
            },
            Some((
                area.width,
                if images_enabled && area.width >= INLINE_ALBUM_ART_RESERVED + 20 {
                    INLINE_ALBUM_ART_RESERVED
                } else {
                    0
                },
            )),
            hero_handles_detail,
            plan_ctx,
        );
    if hero_handles_detail {
        let (offset, image_paint) = super::album_inline::render_grouped_album_rows_inline_plan(
            f,
            area,
            albums,
            album_info,
            cursor_ctx,
            focused,
            plan,
            images_enabled,
            layout,
        );
        return (offset, image_paint);
    }
    if !hero_handles_detail
        && plan
            .selected_block_bounds
            .is_some_and(|(top, bottom)| bottom.saturating_sub(top).saturating_add(3) >= visible)
    {
        plan = crate::app::render::screens::album_plan::build_grouped_album_display_plan_with_ctx(
            albums,
            &album_info,
            &order,
            cursor,
            true,
            HeaderFocusCtx {
                in_music_group_view,
                expand_selected,
            },
            Some((
                area.width,
                if images_enabled && area.width >= INLINE_ALBUM_ART_RESERVED + 20 {
                    INLINE_ALBUM_ART_RESERVED
                } else {
                    0
                },
            )),
            true,
            crate::app::render::screens::album_plan::GroupedAlbumDisplayPlanCtx {
                images_enabled,
                playing_track_id,
                album_tracks,
            },
        );
    }
    layout.left_sorted_indices = plan.order.clone();
    let display_cursor = plan.display_cursor;
    let display_rows = plan.rows;
    let selected_block_bounds = plan.selected_block_bounds;
    let track_detail_bounds = plan.track_detail_bounds;
    let selected_art_reserved_w = if images_enabled
        && selected_block_bounds.is_some()
        && area.width >= INLINE_ALBUM_ART_RESERVED + 20
    {
        INLINE_ALBUM_ART_RESERVED
    } else {
        0
    };
    let selected_art_abs_rows = selected_block_bounds.and_then(|(top_pad_abs, bottom_pad_abs)| {
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

    // Build the same display-row -> screen-row/column mapping used by the
    // rendering loop. Keeping this mapping in one place is important in
    // two-column mode: a viewport can begin halfway through a packed row,
    // so resetting the packing state at `offset` makes the visible cells,
    // selection marker, and cursor highlight disagree.
    let cn = cols.max(1) as usize;
    let mut display_screen_rows = vec![0usize; display_rows.len()];
    let mut display_columns = vec![None; display_rows.len()];
    let mut screen = 0usize;
    let mut group_album_idx = 0usize;
    let mut sr: Vec<Vec<usize>> = vec![Vec::new()];
    for (di, row) in display_rows.iter().enumerate() {
        match row {
            GroupedAlbumDisplayRow::Album(idx)
            | GroupedAlbumDisplayRow::AlbumInlineDetailStart(idx) => {
                let col = group_album_idx % cn;
                display_screen_rows[di] = screen;
                display_columns[di] = Some(col);
                group_album_idx += 1;
                while sr[screen].len() <= col {
                    sr[screen].push(0);
                }
                sr[screen][col] = *idx;
                if (col + 1).is_multiple_of(cn) {
                    screen += 1;
                    sr.push(Vec::new());
                }
            }
            GroupedAlbumDisplayRow::AlbumWrappedContinuation if cn > 1 => {
                display_screen_rows[di] = screen;
            }
            GroupedAlbumDisplayRow::ArtistHeader(_) | GroupedAlbumDisplayRow::ArtistGroupSpacer => {
                if !group_album_idx.is_multiple_of(cn) {
                    screen += 1;
                    sr.push(Vec::new());
                }
                group_album_idx = 0;
                display_screen_rows[di] = screen;
                screen += 1;
                sr.push(Vec::new());
            }
            _ => {
                display_screen_rows[di] = screen;
                screen += 1;
                sr.push(Vec::new());
            }
        }
    }
    while sr.last().is_some_and(|r| r.is_empty()) && sr.len() > 1 {
        sr.pop();
    }
    let screen_offset = display_screen_rows.get(offset).copied().unwrap_or(0);
    // Authoritative selected-cell rect (two-column packed cells use
    // `cell_w = area.width / cols`; the last column takes the rest).
    // Narrow never holds inline track focus, so this rect is always the
    // selected album cell.
    {
        let cn = cols.max(1) as usize;
        let cw = area.width / cn as u16;
        layout.selected_item_rect = selected_cell_rect(
            area,
            cursor,
            &layout.left_item_rows,
            screen_offset,
            cn,
            cw,
            0,
        );
    }
    if !hero_handles_detail
        && matches!(
            display_rows.get(display_cursor),
            Some(GroupedAlbumDisplayRow::AlbumInlineDetailStart(_))
        )
    {
        let detail_rows = display_rows[display_cursor..]
            .iter()
            .take_while(|row| {
                matches!(
                    row,
                    GroupedAlbumDisplayRow::AlbumInlineDetailStart(_)
                        | GroupedAlbumDisplayRow::AlbumDetailContinuation
                        | GroupedAlbumDisplayRow::AlbumLoading
                )
            })
            .count();
        let screen_y = display_screen_rows[display_cursor].saturating_sub(screen_offset);
        if screen_y < visible && detail_rows > 0 {
            layout.hero_area = Rect {
                x: area.x,
                y: area.y + screen_y as u16,
                width: area.width,
                height: detail_rows.min(visible - screen_y) as u16,
            };
            layout.inline_hero_area = layout.hero_area;
            layout.selected_item_rect = Some(layout.hero_area);
        }
    }
    // Visible-slice row map and targets indexed by screen row
    let vs = visible.min(sr.len().saturating_sub(screen_offset));
    layout.left_row_map = (screen_offset..)
        .take(vs)
        .map(|i| sr[i].first().copied())
        .collect();
    layout.left_row_targets = (0..vs)
        .map(|vi| {
            let screen_row = screen_offset + vi;
            (0..display_rows.len())
                .find(|&d| display_screen_rows[d] == screen_row)
                .and_then(|d| display_rows[d].row_target())
        })
        .collect();
    let total_screen_rows = sr.len();
    layout.left_item_rows = sr;
    layout.left_screen_offset = screen_offset;

    // Paint the colored background block before rendering row content
    if let Some((top_pad_abs, bottom_pad_abs)) = selected_block_bounds {
        let bg = palette::resolve_surface_focus(focused);
        crate::app::render::render_selected_block_background(
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
                Block::default().style(Style::default().bg(palette::SURFACE_ACCENT_SOFT)),
                Rect {
                    x: block_x,
                    y: block_y,
                    width: block_w,
                    height: block_h,
                },
            );
        }
    }

    let visible_screen_end = screen_offset + visible;
    let visible_rows: Vec<(usize, &GroupedAlbumDisplayRow)> = display_rows
        .iter()
        .enumerate()
        .filter(|(idx, _)| {
            display_screen_rows[*idx] >= screen_offset
                && display_screen_rows[*idx] < visible_screen_end
        })
        .collect();

    // Produce the area rect for a full-width row (headers, spacers, and
    // non-album filler rows). The `Album` branch replaces the width/height
    // and potentially the x coordinate.
    let full_row_rect = |screen_y: u16| Rect {
        x: area.x,
        y: area.y + screen_y,
        width: area.width,
        height: 1,
    };

    for (row_idx, &(abs_row_idx, row)) in visible_rows.iter().enumerate() {
        let screen_y = display_screen_rows[abs_row_idx] - screen_offset;

        // Determine if this row should start a new terminal row or continue
        // in the current row (for two-column packing).
        let row_area = match row {
            GroupedAlbumDisplayRow::Album(_)
            | GroupedAlbumDisplayRow::AlbumInlineDetailStart(_) => {
                let col = display_columns[abs_row_idx].unwrap_or(0);
                let col_width = area.width / cn as u16;
                let col_x = area.x + (col as u16 * col_width);
                // Last column gets remaining width to avoid rounding errors
                let actual_width = if col == cn - 1 {
                    area.width.saturating_sub(col as u16 * col_width)
                } else {
                    col_width
                };

                Rect {
                    x: col_x,
                    y: area.y + screen_y as u16,
                    width: actual_width,
                    height: 1,
                }
            }
            _ => full_row_rect(screen_y as u16),
        };

        match row {
            GroupedAlbumDisplayRow::ArtistHeader(header) => {
                super::album_rows::render_artist_header_row(
                    f,
                    row_area,
                    header,
                    in_music_group_view,
                    selected_block_bounds,
                    abs_row_idx,
                    selected_art_reserved_w,
                );
            }
            GroupedAlbumDisplayRow::ArtistGroupSpacer => {}
            GroupedAlbumDisplayRow::AlbumDetailRule => {
                // Padding rows for the colored block; the background is painted separately.
                // This row renders as empty, letting the background block show through.
            }
            GroupedAlbumDisplayRow::AlbumWrappedContinuation => {}
            GroupedAlbumDisplayRow::Album(idx) => {
                super::album_rows::render_album_row(
                    f,
                    AlbumRowCtx {
                        row_area,
                        idx: *idx,
                        album_info: &album_info,
                        cursor,
                        avail,
                        selected_block_bounds,
                        in_music_group_view,
                        abs_row_idx,
                        selected_art_reserved_w,
                        focused,
                    },
                );
            }
            GroupedAlbumDisplayRow::AlbumInlineDetailStart(idx) => {
                let height = visible_rows[row_idx..]
                    .iter()
                    .take_while(|(_, r)| {
                        matches!(
                            *r,
                            &GroupedAlbumDisplayRow::AlbumInlineDetailStart(_)
                                | &GroupedAlbumDisplayRow::AlbumDetailContinuation
                                | &GroupedAlbumDisplayRow::AlbumLoading
                        )
                    })
                    .count() as u16;
                super::album_rows::render_album_row(
                    f,
                    AlbumRowCtx {
                        row_area,
                        idx: *idx,
                        album_info: &album_info,
                        cursor,
                        avail,
                        selected_block_bounds,
                        in_music_group_view,
                        abs_row_idx,
                        selected_art_reserved_w,
                        focused,
                    },
                );
                if height > 1 {
                    if let Some(tracks) = album_tracks.get(&albums[*idx].id).cloned() {
                        // Narrow keeps inline track focus explicitly off:
                        // no focused track cursor and no focus highlight.
                        super::album_detail::render_album_detail(
                            f,
                            Rect {
                                y: row_area.y + 1,
                                height: height - 1,
                                ..row_area
                            },
                            &tracks,
                            0,
                            false,
                            false,
                            false,
                            true,
                            false,
                            selected_art_reserved_w,
                            layout,
                        );
                    }
                }
            }
            GroupedAlbumDisplayRow::AlbumActionHint => {
                super::album_rows::render_album_action_hint(
                    f,
                    row_area,
                    in_music_group_view,
                    selected_block_bounds,
                    abs_row_idx,
                    selected_art_reserved_w,
                    focused,
                );
            }
            GroupedAlbumDisplayRow::AlbumDetailStart(idx) => {
                let height = visible_rows[row_idx..]
                    .iter()
                    .take_while(|(_, r)| {
                        matches!(
                            *r,
                            &GroupedAlbumDisplayRow::AlbumDetailStart(_)
                                | &GroupedAlbumDisplayRow::AlbumDetailContinuation
                        )
                    })
                    .count() as u16;
                if let Some(tracks) = album_tracks.get(&albums[*idx].id).cloned() {
                    // Narrow keeps inline track focus explicitly off: the
                    // track block paints unfocused (cursor 0, no focus
                    // highlight).
                    let cursor: usize = 0;
                    let detail_focused = false;
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
                            Block::default().style(Style::default().bg(palette::SURFACE_FOCUSED)),
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
                    super::album_detail::render_album_detail(
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
                            crate::app::render::selection_marker(
                                true,
                                crate::app::render::MarkerEdge::Left,
                            ),
                            Span::raw(" "),
                            Span::styled(
                                line.into_owned(),
                                Style::default().fg(palette::TEXT_MUTED),
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
    }

    if focused && total_screen_rows > visible {
        let max_off = total_screen_rows.saturating_sub(visible);
        crate::app::render::render_right_scrollbar(
            f,
            area,
            max_off,
            screen_offset,
            palette::SCROLLBAR,
        );
    }

    // The selected album's hero art is emitted as a typed `MusicImagePaint`
    // for the shell to execute (image-cache authority stays in `App` during
    // the migration); this painter never touches the image cache itself.
    let mut image_paint = None;
    if let Some((art_top, art_bottom)) = selected_art_abs_rows {
        if art_top >= offset && art_top < offset + visible {
            let visible_bottom = art_bottom.min(offset + visible);
            let art_rect = Rect {
                x: area.x,
                y: area.y + (art_top - offset) as u16,
                width: area.width,
                height: (visible_bottom - art_top) as u16,
            };
            if let Some(album) = albums.get(cursor) {
                image_paint = Some(MusicImagePaint::Album {
                    area: art_rect,
                    album: Box::new(album.clone()),
                    centered: false,
                });
            }
        }
    }

    // Paint the ▁/▔ border rows around the colored block (after content/scrollbar)
    if let Some((top_pad_abs, bottom_pad_abs)) = selected_block_bounds {
        crate::app::render::render_selected_block_borders(
            f,
            area,
            offset,
            visible,
            top_pad_abs,
            bottom_pad_abs,
            crate::app::render::SelectedBlockBorderStyle::Framed,
        );
    }

    // Draw the unified edge selection marker (design.md decision 2):
    // every list shows it regardless of column count, matching every
    // other `draw_column_selection_markers` caller (movies/TV,
    // audiobooks, feeds).
    draw_column_selection_markers(f, area, cursor, &layout.left_item_rows, screen_offset);

    (offset, image_paint)
}
