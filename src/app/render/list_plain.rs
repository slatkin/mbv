use super::super::ui_util::*;
use super::hero::{inline_display_row, inline_display_row_count, InlineDisplayRow};
use super::list_rows::{
    draw_column_selection_markers, focused_or_subtle, item_cell_spans, selected_cell_rect,
    DisplayRow, ListRenderCtx,
};
use crate::app::layout::LayoutMain;
use crate::app::library_column_width::{library_cell_width, LIBRARY_COLUMN_GAP};
use crate::app::{palette, App};
use ratatui::layout::Rect;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

impl App {
    /// Plain list kind of `render_list`: the catch-all covering the
    /// Home "Continue Watching" tab, search result sets, small libraries,
    /// and non-album music levels, all of which render identically without
    /// letter grouping. Returns the scroll offset to persist.
    pub(super) fn render_plain_rows(
        &mut self,
        f: &mut Frame,
        ctx: ListRenderCtx,
        layout: &mut LayoutMain,
    ) -> usize {
        let ListRenderCtx {
            content_area,
            items,
            cursor,
            stored_scroll,
            cols,
            focused,
            hero_rows,
        } = ctx;
        let n = items.len();
        let visible = content_area.height as usize;
        let cell_w = library_cell_width(content_area, cols) as usize;

        // Build display rows row-major: item `i` occupies column `i % cols`
        // of row `i / cols`. In one-column mode every row carries exactly
        // one index, so both modes share this single path.
        let display_rows: Vec<DisplayRow> = (0..n)
            .collect::<Vec<_>>()
            .chunks(cols.max(1))
            .map(|item_row| DisplayRow::Item(item_row.to_vec()))
            .collect();
        // `display_cursor` is the index of the *row containing* the cursor
        // item, so the scroll clamp keeps the cursor row on screen.
        let display_cursor = display_rows
            .iter()
            .position(|r| matches!(r, DisplayRow::Item(idxs) if idxs.contains(&cursor)))
            .unwrap_or(0);
        let total_display = inline_display_row_count(display_rows.len(), display_cursor, hero_rows);

        // Keep the cursor row visible: never scroll it above the viewport's
        // top.
        let (offset, detail_screen_row) = if hero_rows > 0 {
            let flow = super::hero::inline_detail_flow(
                display_cursor,
                hero_rows,
                content_area.height,
                stored_scroll,
            )
            .expect("inline detail was admitted only when its active row fits");
            (flow.offset, Some(flow.detail_screen_row))
        } else {
            let lower_bound = display_cursor.saturating_sub(visible.saturating_sub(1));
            (stored_scroll.clamp(lower_bound, display_cursor), None)
        };
        let final_offset = offset;

        let show_scrollbar = focused && total_display > visible;

        let list_items: Vec<ListItem> = (offset..total_display)
            .take(visible)
            .map(|display_row| {
                match inline_display_row(display_rows.len(), display_cursor, hero_rows, display_row)
                    .expect("display row is within the replacement flow")
                {
                    InlineDisplayRow::Replacement => ListItem::new(Line::default()),
                    InlineDisplayRow::Source(source_row) => match &display_rows[source_row] {
                        DisplayRow::Spacer | DisplayRow::LetterHeader(_) => {
                            ListItem::new(Line::default())
                        }
                        DisplayRow::Item(idxs) => {
                            // Each item renders into its own cell, truncated to the
                            // cell width; cells are padded to the cell boundary
                            // (+ inter-column gap) so the next cell starts at its
                            // own x offset. Trailing partial rows leave the empty
                            // cells as plain list background.
                            let mut spans: Vec<Span> = Vec::new();
                            for (cell_idx, &idx) in idxs.iter().enumerate() {
                                let item = &items[idx];
                                let selected = idx == cursor;

                                // Compute name and duration as separate strings so they can be styled
                                // independently: name in the normal fg, duration in OVERLAY (no parens).
                                let (item_name, dur_str) = if item.is_folder {
                                    let name = if item.item_type == "Folder" && item.total_count > 0
                                    {
                                        format!(
                                            "{} \u{b7} {} items",
                                            item.display_name(),
                                            item.total_count
                                        )
                                    } else if item.unplayed_item_count > 0
                                        && item.item_type != "Series"
                                    {
                                        format!(
                                            "{} [{}]",
                                            item.display_name(),
                                            item.unplayed_item_count
                                        )
                                    } else {
                                        item.display_name()
                                    };
                                    (name, String::new())
                                } else {
                                    let year = if item.production_year > 0 {
                                        format!(" {}", item.production_year)
                                    } else {
                                        String::new()
                                    };
                                    (item.display_name(), year)
                                };

                                // Every cell starts with a 1-column leading
                                // separator (the selected cell's highlight
                                // background, a plain space otherwise), so titles
                                // align across rows.
                                let avail = cell_w.saturating_sub(2);
                                let name_w = avail.saturating_sub(dur_str.width());
                                let (title, dur_str) = if selected && hero_rows > 0 {
                                    (String::new(), String::new())
                                } else {
                                    (trunc_str(&item_name, name_w), dur_str)
                                };
                                let fg = focused_or_subtle(focused);

                                let pad_to = if cell_idx + 1 == idxs.len() {
                                    cell_w
                                } else {
                                    cell_w + LIBRARY_COLUMN_GAP as usize
                                };
                                spans.extend(item_cell_spans(title, dur_str, selected, fg, pad_to));
                            }
                            ListItem::new(Line::from(spans))
                        }
                    },
                }
            })
            .collect();

        layout.left_row_map = (offset..total_display)
            .take(visible)
            .enumerate()
            .map(|(visible_row, _)| {
                match inline_display_row(
                    display_rows.len(),
                    display_cursor,
                    hero_rows,
                    offset + visible_row,
                )
                .expect("visible row is within the replacement flow")
                {
                    InlineDisplayRow::Replacement => {
                        (offset + visible_row == display_cursor).then_some(cursor)
                    }
                    InlineDisplayRow::Source(source_row) => match &display_rows[source_row] {
                        DisplayRow::Spacer | DisplayRow::LetterHeader(_) => None,
                        DisplayRow::Item(idxs) => idxs.first().copied(),
                    },
                }
            })
            .collect();
        // Publish the full row structure (parallel to the display rows,
        // empty entries for headers) so column-aware cursor movement and
        // mouse hit-testing can resolve cells between frames.
        layout.left_item_rows = (0..total_display)
            .map(|display_row| {
                match inline_display_row(display_rows.len(), display_cursor, hero_rows, display_row)
                    .expect("display row is within the replacement flow")
                {
                    InlineDisplayRow::Replacement => {
                        if display_row == display_cursor {
                            vec![cursor]
                        } else {
                            Vec::new()
                        }
                    }
                    InlineDisplayRow::Source(source_row) => match &display_rows[source_row] {
                        DisplayRow::Item(idxs) => idxs.clone(),
                        _ => Vec::new(),
                    },
                }
            })
            .collect();

        if let Some(detail_screen_row) = detail_screen_row {
            layout.hero_area = Rect {
                x: content_area.x,
                y: content_area.y + detail_screen_row as u16,
                width: content_area.width,
                height: hero_rows,
            };
            layout.inline_hero_area = layout.hero_area;
        }

        let mut state = ListState::default();
        state.select(Some(display_cursor.saturating_sub(offset)));
        layout.selected_item_rect = selected_cell_rect(
            content_area,
            cursor,
            &layout.left_item_rows,
            offset,
            cols,
            cell_w as u16,
            LIBRARY_COLUMN_GAP,
        );
        if hero_rows > 0 {
            layout.selected_item_rect = Some(layout.hero_area);
        }
        f.render_stateful_widget(
            List::new(list_items).highlight_style(Style::default()),
            content_area,
            &mut state,
        );

        if show_scrollbar {
            let max_off = total_display.saturating_sub(visible);
            super::render_right_scrollbar(f, content_area, max_off, offset, palette::SCROLLBAR);
        }

        if hero_rows == 0 {
            draw_column_selection_markers(f, content_area, cursor, &layout.left_item_rows, offset);
        }

        final_offset
    }
}
