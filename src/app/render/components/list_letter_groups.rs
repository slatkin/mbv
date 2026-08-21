use super::super::{effective_sort_str, letter_bucket, LetterFilter};
use super::hero::{inline_display_row, inline_display_row_count, InlineDisplayRow};
use super::list_rows::{
    draw_column_selection_markers, focused_or_subtle, item_cell_spans, selected_cell_rect,
    DisplayRow, ListRenderCtx,
};
use crate::app::layout::LayoutMain;
use crate::app::library_column_width::{library_cell_width, LIBRARY_COLUMN_GAP};
use crate::app::ui_util::*;
use crate::app::{palette, App};
use ratatui::layout::Rect;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

impl App {
    /// Letter-grouped list kind of `render_list`: non-music library
    /// lists with 50+ items (or an active letter-range pill), bucketed under
    /// `LetterHeader` rows. Returns the scroll offset to persist.
    pub(in crate::app::render) fn render_letter_grouped_rows(
        &mut self,
        f: &mut Frame,
        ctx: ListRenderCtx,
        active_letter_filter: Option<LetterFilter>,
        ungrouped_total: usize,
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

        // Build display rows: inject a Spacer+LetterHeader at each bucket boundary.
        // The spacer is omitted before the very first header.
        // Sort item indices by the same effective key used for bucketing so that
        // items within each group appear in article-stripped alphabetical order.
        let mut sorted_indices: Vec<usize> = (0..n).collect();
        sorted_indices.sort_by_key(|&i| natural_sort_key(effective_sort_str(&items[i])));
        // Publish the sorted order so cursor navigation can follow display order.
        layout.left_sorted_indices = sorted_indices.clone();

        // With a letter-range pill active, the visible slice is already
        // narrowed to one range (e.g. `A–C`) -- bucket by the individual
        // first letter within it (`A`, `B`, `C`) rather than re-deriving
        // a range bucket from the slice's own (small) size. Forcing
        // `letter_bucket`'s `total >= 250` branch reuses its existing
        // per-letter logic without a second code path.
        let bucket_total = if active_letter_filter.is_some() {
            usize::MAX
        } else {
            ungrouped_total
        };
        // Each letter bucket packs independently: a bucket always starts a
        // fresh item row, so no row mixes items from two buckets. The cost
        // is a ragged trailing cell at the end of every bucket, which is
        // correct -- a row straddling the bucket boundary would put the
        // header between its items.
        let mut display_rows: Vec<DisplayRow> = Vec::new();
        let mut last_bucket = String::new();
        let mut current_row: Vec<usize> = Vec::with_capacity(cols.max(1));
        for &idx in &sorted_indices {
            let item = &items[idx];
            let bucket = letter_bucket(item, bucket_total);
            if bucket != last_bucket {
                if !current_row.is_empty() {
                    push_item_row(&mut display_rows, &mut current_row);
                }
                if !last_bucket.is_empty() {
                    display_rows.push(DisplayRow::Spacer);
                }
                display_rows.push(DisplayRow::LetterHeader(bucket.clone()));
                last_bucket = bucket;
            }
            current_row.push(idx);
            if current_row.len() >= cols.max(1) {
                push_item_row(&mut display_rows, &mut current_row);
            }
        }
        if !current_row.is_empty() {
            push_item_row(&mut display_rows, &mut current_row);
        }

        // Find the visual row of the current cursor item for scrolling
        // (`display_cursor` is the *row containing* the cursor) and the
        // cursor's column within that row.
        let display_cursor = display_rows
            .iter()
            .position(|r| matches!(r, DisplayRow::Item(idxs) if idxs.contains(&cursor)))
            .unwrap_or(0);

        let total_display = inline_display_row_count(display_rows.len(), display_cursor, hero_rows);

        // Keep the cursor row visible: never scroll it above the viewport's
        // top.
        let (mut offset, mut detail_screen_row) = if hero_rows > 0 {
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
        // If stale scroll state would put the first item of a bucket at the
        // top of the viewport, back up so its letter header remains visible.
        if visible > 1 && offset > 0 {
            let mut run_start = offset;
            if run_start > 0
                && matches!(
                    display_rows.get(run_start - 1),
                    Some(DisplayRow::LetterHeader(_))
                )
            {
                run_start -= 1;
            }
            offset = run_start;
        }
        if hero_rows > 0 {
            detail_screen_row = Some(display_cursor.saturating_sub(offset));
        }
        let final_offset = offset;

        // Build row map so mouse clicks can map visual row → item index
        // (first item of each row; two-column mouse clicks resolve the
        // cell via `left_item_rows`).
        for visible_row in (offset..total_display).take(visible) {
            layout.left_row_map.push(
                match inline_display_row(display_rows.len(), display_cursor, hero_rows, visible_row)
                    .expect("visible row is within the replacement flow")
                {
                    InlineDisplayRow::Replacement => {
                        (visible_row == display_cursor).then_some(cursor)
                    }
                    InlineDisplayRow::Source(source_row) => match &display_rows[source_row] {
                        DisplayRow::Spacer | DisplayRow::LetterHeader(_) => None,
                        DisplayRow::Item(idxs) => idxs.first().copied(),
                    },
                },
            );
        }
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

        let show_scrollbar = focused && total_display > visible;

        // Width available to title + duration on a list row: the 1-col
        // leading separator, plus the letter-grouped rows' extra indent.
        let normal_avail = cell_w.saturating_sub(4);
        let list_items: Vec<ListItem> = (offset..total_display)
            .take(visible)
            .map(|display_row| {
                match inline_display_row(display_rows.len(), display_cursor, hero_rows, display_row)
                    .expect("display row is within the replacement flow")
                {
                    InlineDisplayRow::Replacement => ListItem::new(Line::default()),
                    InlineDisplayRow::Source(source_row) => match &display_rows[source_row] {
                        DisplayRow::Spacer => ListItem::new(Line::default()),
                        DisplayRow::LetterHeader(label) => ListItem::new(Line::from(vec![
                            Span::raw(" "),
                            Span::styled(
                                label.clone(),
                                Style::default()
                                    .fg(palette::TEXT_FOCUS_ACCENT)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ])),
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
                                // Same width budget for every row (selected or not)
                                // so titles align across the row; the selected
                                // cell's 1-column leading separator carries the
                                // highlight background rather than adding an indent.
                                let avail = normal_avail;
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

        if let Some(detail_screen_row) = detail_screen_row {
            layout.hero_area = Rect {
                x: content_area.x,
                y: content_area.y + detail_screen_row as u16,
                width: content_area.width,
                height: hero_rows,
            };
            layout.inline_hero_area = layout.hero_area;
            layout.selected_item_rect = Some(layout.hero_area);
        }
        f.render_stateful_widget(
            List::new(list_items).highlight_style(Style::default()),
            content_area,
            &mut state,
        );

        if show_scrollbar {
            let max_off = total_display.saturating_sub(visible);
            crate::app::render::render_right_scrollbar(
                f,
                content_area,
                max_off,
                offset,
                palette::SCROLLBAR,
            );
        }

        if hero_rows == 0 {
            draw_column_selection_markers(f, content_area, cursor, &layout.left_item_rows, offset);
        }

        final_offset
    }
}

/// Flushes one packed item row into `display_rows`; `current_row` is
/// emptied by the flush.
fn push_item_row(display_rows: &mut Vec<DisplayRow>, current_row: &mut Vec<usize>) {
    if current_row.is_empty() {
        return;
    }
    display_rows.push(DisplayRow::Item(std::mem::take(current_row)));
}
