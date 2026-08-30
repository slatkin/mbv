use super::album::AlbumRowsCursorCtx;
use super::album_rows::AlbumRowCtx;
use super::hero::{selected_detail_shell, InlineDisplayRow, HERO_BLOCK_EXTRA_ROWS};
use super::list_rows::{
    draw_column_selection_markers, selected_cell_rect, DisplayRow, InlineReplacementPlan,
};
use crate::app::layout::{LayoutMain, LibraryRowTarget};
use crate::app::palette;
use crate::app::render::components::album_detail::album_hero_detail_rows;
use crate::app::render::screens::album_plan::{GroupedAlbumDisplayPlan, GroupedAlbumDisplayRow};
use ratatui::layout::Rect;
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

pub(in crate::app::render) fn render_grouped_album_rows_inline_plan(
    f: &mut Frame,
    area: Rect,
    _albums: &[mbv_core::api::EmbyItem],
    album_info: Vec<(String, String, String)>,
    cursor_ctx: AlbumRowsCursorCtx,
    focused: bool,
    plan: GroupedAlbumDisplayPlan,
    images_enabled: bool,
    layout: &mut LayoutMain,
) -> usize {
    let AlbumRowsCursorCtx {
        cursor,
        stored_scroll,
    } = cursor_ctx;
    let grouped_rows = plan.rows;
    let display_rows: Vec<DisplayRow> = grouped_rows
        .iter()
        .map(|row| match row {
            GroupedAlbumDisplayRow::ArtistHeader(header) => {
                DisplayRow::LetterHeader(header.artist_label.clone())
            }
            GroupedAlbumDisplayRow::ArtistGroupSpacer
            | GroupedAlbumDisplayRow::AlbumWrappedContinuation => DisplayRow::Spacer,
            GroupedAlbumDisplayRow::Album(idx) => DisplayRow::Item(vec![*idx]),
            _ => DisplayRow::Spacer,
        })
        .collect();
    let selected_row = display_rows
        .iter()
        .position(|row| matches!(row, DisplayRow::Item(items) if items.contains(&cursor)))
        .unwrap_or(display_rows.len());
    let hero_rows =
        (album_hero_detail_rows(images_enabled) + HERO_BLOCK_EXTRA_ROWS as usize) as u16;
    let replacement = InlineReplacementPlan::new(
        &display_rows,
        selected_row,
        cursor,
        hero_rows,
        area.height,
        stored_scroll,
    );
    let offset = replacement.offset();
    let visible = area.height as usize;
    let item_rows = replacement.item_rows();
    let total_display = item_rows.len();
    let row_targets = replacement.row_targets();

    layout.left_sorted_indices = plan.order.clone();
    layout.left_item_rows = item_rows.clone();
    layout.left_screen_offset = 0;
    layout.left_row_map = row_targets
        .iter()
        .skip(offset)
        .take(visible)
        .copied()
        .collect();
    layout.left_row_targets = row_targets
        .iter()
        .skip(offset)
        .take(visible)
        .map(|target| target.map(LibraryRowTarget::Album))
        .collect();

    let hero_area = replacement.hero_area(area);
    layout.selected_item_rect = hero_area
        .or_else(|| selected_cell_rect(area, cursor, &item_rows, offset, 1, area.width, 0));
    if let Some(hero_area) = hero_area {
        layout.hero_area = hero_area;
        layout.inline_hero_area = hero_area;
    }

    let visible_rows: Vec<usize> = (offset..total_display).take(visible).collect();
    let mut state = ListState::default();
    state.select(Some(selected_row.saturating_sub(offset)));
    f.render_stateful_widget(
        List::new(
            visible_rows
                .iter()
                .map(|_| ListItem::new(""))
                .collect::<Vec<_>>(),
        )
        .highlight_style(ratatui::style::Style::default()),
        area,
        &mut state,
    );
    for display_row in visible_rows {
        let Some(InlineDisplayRow::Source(source_row)) = replacement.display_row(display_row)
        else {
            continue;
        };
        match &grouped_rows[source_row] {
            GroupedAlbumDisplayRow::ArtistHeader(header) => {
                super::album_rows::render_artist_header_row(
                    f,
                    Rect {
                        y: area.y + (display_row - offset) as u16,
                        ..area
                    },
                    header,
                    true,
                    None,
                    source_row,
                    0,
                );
            }
            GroupedAlbumDisplayRow::Album(idx) => {
                super::album_rows::render_album_row(
                    f,
                    AlbumRowCtx {
                        row_area: Rect {
                            y: area.y + (display_row - offset) as u16,
                            ..area
                        },
                        idx: *idx,
                        album_info: &album_info,
                        cursor,
                        avail: area.width.saturating_sub(2) as usize,
                        selected_block_bounds: None,
                        in_music_group_view: true,
                        abs_row_idx: source_row,
                        selected_art_reserved_w: 0,
                        focused,
                    },
                );
            }
            _ => {}
        }
    }

    if let Some(hero_area) = hero_area {
        selected_detail_shell(f, hero_area, hero_rows, focused);
        if let Some(album_idx) = plan.order.iter().find(|&&idx| idx == cursor) {
            if let Some((artist, year, title)) = album_info.get(*album_idx) {
                let meta = if year.is_empty() {
                    artist.clone()
                } else {
                    format!("{artist} • {year}")
                };
                let content = vec![
                    ratatui::text::Line::from(ratatui::text::Span::styled(
                        format!(" {title}"),
                        ratatui::style::Style::default()
                            .fg(crate::app::palette::TEXT_FOCUS_ACCENT)
                            .add_modifier(ratatui::style::Modifier::BOLD),
                    )),
                    ratatui::text::Line::from(ratatui::text::Span::styled(
                        format!(" {meta}"),
                        ratatui::style::Style::default().fg(crate::app::palette::TEXT_DETAIL_META),
                    )),
                ];
                f.render_widget(Paragraph::new(content), hero_area);
            }
        }
    }
    if focused && total_display > visible {
        crate::app::render::render_right_scrollbar(
            f,
            area,
            total_display.saturating_sub(visible),
            offset,
            palette::SCROLLBAR,
        );
    }
    if replacement.should_draw_selection_markers() {
        draw_column_selection_markers(f, area, cursor, &item_rows, offset);
    }
    offset
}
