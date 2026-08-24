use crate::app::layout::LayoutMain;
use crate::app::palette;
use crate::app::render::components::album_rows::{
    render_album_row, render_artist_header_row, render_wide_selected_album_row, AlbumRowCtx,
};
use crate::app::render::components::list_rows::{
    draw_column_selection_markers, LibraryListRenderCtx,
};
use crate::app::render::screens::album_plan::{ArtistGroupHeader, GroupedAlbumDisplayRow};
use ratatui::layout::Rect;
use ratatui::Frame;

/// Paints the wide Music right rail. The legacy grouped-album plan has many
/// narrow-only detail rows; wide mode removes those rows before painting, so
/// this small App-free plan builds the same remaining header/album sequence.
pub(in crate::app) fn render_wide_right_album_browser_with_ctx(
    f: &mut Frame,
    browser_area: Rect,
    panel_area: Rect,
    album_info: &[(String, String, String)],
    order: &[usize],
    list: &LibraryListRenderCtx,
    right_focused: bool,
    layout: &mut LayoutMain,
) -> usize {
    layout.wide_music_browser_area = browser_area;
    if list.items.is_empty() {
        crate::app::render::render_placeholder(
            f,
            browser_area,
            if list.loading {
                " Loading\u{2026}"
            } else {
                " (empty)"
            },
        );
        return 0;
    }

    let rows = wide_album_display_rows(&list.items, album_info, order);
    let cursor = list.cursor;
    let display_cursor = rows
        .iter()
        .position(|row| matches!(row, GroupedAlbumDisplayRow::Album(index) if *index == cursor))
        .unwrap_or(0);
    let visible = browser_area.height as usize;
    let max_offset = rows.len().saturating_sub(visible);
    let mut offset = list.scroll.min(max_offset);
    if display_cursor < offset {
        offset = display_cursor;
    } else if display_cursor >= offset + visible {
        offset = display_cursor
            .saturating_add(1)
            .saturating_sub(visible)
            .min(max_offset);
    }

    let visible_rows: Vec<_> = rows.iter().enumerate().skip(offset).take(visible).collect();
    for (row_idx, row) in &visible_rows {
        let screen_y = (*row_idx - offset) as u16;
        let row_area = Rect {
            x: browser_area.x,
            y: browser_area.y + screen_y,
            width: browser_area.width,
            height: 1,
        };
        match row {
            GroupedAlbumDisplayRow::ArtistHeader(header) => {
                render_artist_header_row(f, row_area, header, true, None, *row_idx, 0);
            }
            GroupedAlbumDisplayRow::Album(index) => {
                let selected = *index == cursor;
                if selected {
                    layout.selected_item_rect = Some(row_area);
                }
                if selected && right_focused {
                    render_wide_selected_album_row(
                        f,
                        row_area,
                        panel_area,
                        *index,
                        album_info,
                        right_focused,
                    );
                } else {
                    render_album_row(
                        f,
                        AlbumRowCtx {
                            row_area,
                            idx: *index,
                            album_info,
                            cursor,
                            avail: row_area.width as usize,
                            selected_block_bounds: None,
                            in_music_group_view: true,
                            abs_row_idx: *row_idx,
                            selected_art_reserved_w: 0,
                            focused: right_focused,
                        },
                    );
                }
            }
            _ => {}
        }
    }

    if rows.len() > visible && right_focused {
        crate::app::render::render_right_scrollbar(
            f,
            browser_area,
            rows.len().saturating_sub(visible),
            offset,
            palette::SCROLLBAR,
        );
    }
    let item_rows: Vec<Vec<usize>> = rows
        .iter()
        .map(|row| match row {
            GroupedAlbumDisplayRow::Album(index) => vec![*index],
            _ => Vec::new(),
        })
        .collect();
    draw_column_selection_markers(f, browser_area, cursor, &item_rows, offset);

    layout.left_row_targets = vec![None; browser_area.height as usize];
    for (row_idx, row) in &visible_rows {
        let screen_y = row_idx.saturating_sub(offset);
        if let Some(slot) = layout.left_row_targets.get_mut(screen_y) {
            *slot = row.row_target();
        }
    }
    layout.left_sorted_indices = order.to_vec();
    offset
}

fn wide_album_display_rows(
    albums: &[mbv_core::api::EmbyItem],
    album_info: &[(String, String, String)],
    order: &[usize],
) -> Vec<GroupedAlbumDisplayRow> {
    let mut rows = Vec::new();
    let mut start = 0;
    while start < order.len() {
        let artist = album_info[order[start]].0.clone();
        let mut end = start + 1;
        while end < order.len() && album_info[order[end]].0 == artist {
            end += 1;
        }
        if start > 0 {
            rows.push(GroupedAlbumDisplayRow::ArtistGroupSpacer);
        }
        let first = order[start];
        rows.push(GroupedAlbumDisplayRow::ArtistHeader(ArtistGroupHeader {
            first_album_id: albums[first].id.clone(),
            artist_label: artist,
        }));
        rows.extend(
            order[start..end]
                .iter()
                .copied()
                .map(GroupedAlbumDisplayRow::Album),
        );
        start = end;
    }
    rows
}
