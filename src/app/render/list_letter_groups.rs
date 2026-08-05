use super::super::ui_util::*;
use super::list_rows::{focused_or_subtle, item_cell_spans, DisplayRow, ListRenderCtx};
use super::{effective_sort_str, letter_bucket, LetterFilter};
use crate::app::layout::LayoutMain;
use crate::app::library_column_width::{library_cell_width, LIBRARY_COLUMN_GAP};
use crate::app::{palette, App};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::Rect;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

impl App {
    /// Letter-grouped list kind of `render_power_list`: non-music library
    /// lists with 50+ items (or an active letter-range pill), bucketed under
    /// `LetterHeader` rows. Returns the scroll offset to persist.
    pub(super) fn render_power_letter_grouped_rows(
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

        // Insert the inline hero just below the row containing the cursor:
        // the top section ends at the cursor's row, the hero fills the next
        // `hero_rows` display rows (blank in the List widget -- the hero is
        // painted separately over them), and the bottom section continues
        // below. `display_cursor` is unchanged because the rows are inserted
        // after it.
        if hero_rows > 0 {
            let insert_at = display_cursor + 1;
            display_rows.splice(
                insert_at..insert_at,
                (0..hero_rows).map(|_| DisplayRow::Hero),
            );
        }
        let total_display = display_rows.len();

        // Keep the cursor row and the hero below it visible: never scroll
        // the cursor row above the viewport's top, and never scroll the
        // hero's bottom row past the viewport's bottom. Without a hero
        // (`hero_rows == 0`) this is exactly the old cursor-row clamp.
        let lower_bound = (display_cursor + hero_rows as usize + 1)
            .saturating_sub(visible)
            .min(display_cursor);
        let mut offset = stored_scroll.clamp(lower_bound, display_cursor);
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
        let final_offset = offset;

        // Build row map so mouse clicks can map visual row → item index
        // (first item of each row; two-column mouse clicks resolve the
        // cell via `left_item_rows`).
        for row in display_rows.iter().skip(offset).take(visible) {
            layout.left_row_map.push(match row {
                DisplayRow::Spacer | DisplayRow::LetterHeader(_) | DisplayRow::Hero => None,
                DisplayRow::Item(idxs) => idxs.first().copied(),
            });
        }
        // Publish the full row structure (parallel to the display rows,
        // empty entries for headers) so column-aware cursor movement and
        // mouse hit-testing can resolve cells between frames.
        layout.left_item_rows = display_rows
            .iter()
            .map(|row| match row {
                DisplayRow::Item(idxs) => idxs.clone(),
                _ => Vec::new(),
            })
            .collect();

        let show_scrollbar = focused && total_display > visible;

        // Width available to title + duration on a list row: the 1-col
        // leading separator, plus the letter-grouped rows' extra indent.
        let normal_avail = cell_w.saturating_sub(4);
        let list_items: Vec<ListItem> = display_rows
            .iter()
            .enumerate()
            .skip(offset)
            .take(visible)
            .map(|(_abs_idx, row)| match row {
                DisplayRow::Spacer | DisplayRow::Hero => ListItem::new(Line::default()),
                DisplayRow::LetterHeader(label) => ListItem::new(Line::from(vec![
                    Span::raw(" "),
                    Span::styled(
                        label.clone(),
                        Style::default()
                            .fg(palette::YELLOW)
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
                            let name = if item.item_type == "Folder" && item.total_count > 0 {
                                format!("{} \u{b7} {} items", item.display_name(), item.total_count)
                            } else if item.unplayed_item_count > 0 && item.item_type != "Series" {
                                format!("{} [{}]", item.display_name(), item.unplayed_item_count)
                            } else {
                                item.display_name()
                            };
                            (name, String::new())
                        } else {
                            let dur = if item.runtime_ticks > 0 {
                                format!(
                                    " {}",
                                    fmt_duration_approx(item.runtime_ticks / TICKS_PER_SECOND)
                                )
                            } else {
                                String::new()
                            };
                            (item.display_name(), dur)
                        };
                        // The selected cell's `▌` mark + `## ` prefix take 4
                        // cols, matching the ordinary rows' indent so titles
                        // align across the row.
                        let avail = normal_avail;
                        let name_w = avail.saturating_sub(dur_str.width());
                        let title = trunc_str(&item_name, name_w);
                        let fg = focused_or_subtle(focused);
                        let pad_to = if cell_idx + 1 == idxs.len() {
                            cell_w
                        } else {
                            cell_w + LIBRARY_COLUMN_GAP as usize
                        };
                        spans.extend(item_cell_spans(
                            title, dur_str, selected, focused, fg, pad_to,
                        ));
                    }
                    ListItem::new(Line::from(spans))
                }
            })
            .collect();

        let mut state = ListState::default();
        state.select(Some(display_cursor.saturating_sub(offset)));
        layout.cursor_screen_y =
            Some(content_area.y + (display_cursor.saturating_sub(offset)) as u16);
        // Publish the inline hero's rect (below the selected row) so mouse
        // clicks on it are Enter-equivalents and the hero is painted over
        // its blank display rows afterwards.
        if hero_rows > 0 {
            layout.hero_area = Rect {
                x: content_area.x,
                y: content_area.y + (display_cursor.saturating_sub(offset)) as u16 + 1,
                width: content_area.width,
                height: hero_rows,
            };
        }
        f.render_stateful_widget(
            List::new(list_items).highlight_style(Style::default()),
            content_area,
            &mut state,
        );

        if show_scrollbar {
            let max_off = total_display.saturating_sub(visible);
            super::render_power_right_scrollbar(f, content_area, max_off, offset);
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
