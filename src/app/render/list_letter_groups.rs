use super::super::ui_util::*;
use super::list_rows::{
    build_list_row_spans, focused_or_subtle, push_selected_detail_fillers_after,
    push_selected_detail_fillers_before, render_series_detail_background,
    selected_detail_lower_bound, DisplayRow, ListRenderCtx, COMPACT_BANNER_INDENT,
};
use super::{effective_sort_str, letter_bucket, LetterFilter};
use crate::app::layout::LayoutMain;
use crate::app::{palette, App};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::*;
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
            area,
            content_area,
            items,
            cursor,
            stored_scroll,
            banner_rows,
            banner_content_rows,
            series_detail_rows,
            focused,
        } = ctx;
        let n = items.len();
        let visible = content_area.height as usize;

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
        let mut display_rows: Vec<DisplayRow> = Vec::new();
        let mut last_bucket = String::new();
        for &idx in &sorted_indices {
            let item = &items[idx];
            let bucket = letter_bucket(item, bucket_total);
            if bucket != last_bucket {
                if !last_bucket.is_empty() {
                    display_rows.push(DisplayRow::Spacer);
                }
                display_rows.push(DisplayRow::LetterHeader(bucket.clone()));
                last_bucket = bucket;
            }
            push_selected_detail_fillers_before(
                &mut display_rows,
                idx,
                cursor,
                banner_rows,
                series_detail_rows,
            );
            display_rows.push(DisplayRow::Item(idx));
            push_selected_detail_fillers_after(
                &mut display_rows,
                idx,
                cursor,
                banner_rows,
                series_detail_rows,
            );
        }
        let total_display = display_rows.len();

        // Find the visual row of the current cursor item for scrolling.
        let display_cursor = display_rows
            .iter()
            .position(|r| matches!(r, DisplayRow::Item(i) if *i == cursor))
            .unwrap_or(0);
        // For banners, `banner_rows` rows sit below the cursor (opening rule above).
        // For series, `series_detail_rows` rows sit below the cursor (block follows it).
        let lower_bound =
            selected_detail_lower_bound(display_cursor, banner_rows, series_detail_rows, visible);
        let mut offset = stored_scroll.clamp(lower_bound, display_cursor);
        // If stale scroll state would put the first item of a bucket at the
        // top of the viewport, back up so its letter header remains visible.
        // When that item is also the selected/bannered one, the banner's
        // opening rule sits between the header and the item, so back up an
        // extra row to clear the rule too.
        // Also, if a colored-padding BannerFiller (from a selected block) is at
        // the top, back up one row to keep the border-space BannerFiller visible.
        if visible > 1 && offset > 0 {
            // Walk back over any run of banner/series-detail filler rows
            // touching `offset` (whether `offset` lands inside the run or
            // right after it, on the Item row the fillers belong to) so
            // `run_start` points at the run's first row. Then, if a
            // LetterHeader immediately precedes that run, include it too
            // -- otherwise the header (and/or the block's top padding)
            // can be scrolled just out of view with no way back except
            // scrolling further. This generalizes the old fixed-offset
            // special cases to any filler-run length.
            let mut run_start = offset;
            while run_start > 0
                && matches!(
                    display_rows.get(run_start - 1),
                    Some(DisplayRow::BannerFiller | DisplayRow::SeriesDetailFiller)
                )
            {
                run_start -= 1;
            }
            offset = if run_start > 0
                && matches!(
                    display_rows.get(run_start - 1),
                    Some(DisplayRow::LetterHeader(_))
                ) {
                run_start - 1
            } else {
                run_start
            };
        }
        let final_offset = offset;

        // Build row map so mouse clicks can map visual row → item index.
        for row in display_rows.iter().skip(offset).take(visible) {
            layout.left_row_map.push(match row {
                DisplayRow::Spacer
                | DisplayRow::LetterHeader(_)
                | DisplayRow::BannerFiller
                | DisplayRow::SeriesDetailFiller => None,
                DisplayRow::Item(idx) => Some(*idx),
            });
        }

        // Absolute display-row indices of the colored block's top and
        // bottom padding rows (only meaningful when banner_rows > 0).
        // `banner_rule_top` is the padding row directly above the selected
        // item's own row; `banner_rule_bottom` is the padding row after
        // the banner content, before the next list row. Together they
        // frame the selected row + banner as a single CONTINUE_BG block
        // instead of `─` rules around it.
        let banner_rule_top = display_cursor.saturating_sub(1);
        let content_start = display_cursor + 1;
        let banner_rule_bottom = content_start + banner_rows.saturating_sub(2);
        let show_scrollbar = focused && total_display > visible;

        // The selected movie + banner are wrapped in a colored block
        // (matching the home tab's Keep Watching look). Draw the block
        // first, before the list items, so the per-row spans only paint
        // their own cells and the block's background shows through on the
        // side padding cols and on the top/bottom padding rows.
        if banner_rows > 0 {
            let bg = if focused {
                palette::MEDIA_SELECTED_BG
            } else {
                palette::PLAYBACK_PANEL_BG
            };
            super::render_selected_block_background(
                f,
                content_area,
                offset,
                visible,
                banner_rule_top,
                banner_rule_bottom,
                bg,
            );
        }

        render_series_detail_background(
            f,
            content_area,
            offset,
            visible,
            display_cursor,
            series_detail_rows,
            focused,
        );

        // Width available to title + duration on a normal list row (with a
        // 1-col leading separator before the title). For the selected row
        // with an inline banner, the colored block's 2-col side padding
        // + render_power_compact_detail's own internal 1-col pad reserve
        // `2 * COMPACT_BANNER_INDENT + 2` cols off both sides, so the
        // title aligns with the banner's `inner_x` exactly.
        let avail = (area.width as usize).saturating_sub(2 + COMPACT_BANNER_INDENT as usize);
        let list_items: Vec<ListItem> = display_rows
            .iter()
            .enumerate()
            .skip(offset)
            .take(visible)
            .map(|(_abs_idx, row)| match row {
                DisplayRow::Spacer => ListItem::new(Line::default()),
                // The colored block (drawn above) frames the selected row
                // + banner, so the banner's top/bottom padding rows are
                // empty -- they show the block's background.
                DisplayRow::BannerFiller | DisplayRow::SeriesDetailFiller => {
                    ListItem::new(Line::default())
                }
                DisplayRow::LetterHeader(label) => ListItem::new(Line::from(vec![
                    Span::raw(" "),
                    Span::styled(
                        label.clone(),
                        Style::default()
                            .fg(palette::YELLOW)
                            .add_modifier(Modifier::BOLD),
                    ),
                ])),
                DisplayRow::Item(idx) => {
                    let item = &items[*idx];
                    let selected = *idx == cursor;
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
                    let selected_has_banner = selected && banner_rows > 0;
                    let avail = if selected_has_banner {
                        // 2-col left pad + 2-col right pad inside the
                        // colored block: title+dur share area.width - 4.
                        (area.width as usize).saturating_sub(2 + 2 * COMPACT_BANNER_INDENT as usize)
                    } else {
                        avail
                    };
                    let name_w = avail.saturating_sub(dur_str.width());
                    let title = trunc_str(&item_name, name_w);
                    let fg = focused_or_subtle(focused);
                    let is_series = item.item_type == "Series";
                    let spans = build_list_row_spans(
                        title,
                        dur_str,
                        selected,
                        selected_has_banner,
                        is_series,
                        focused,
                        fg,
                    );
                    ListItem::new(Line::from(spans))
                }
            })
            .collect();

        let mut state = ListState::default();
        state.select(Some(display_cursor.saturating_sub(offset)));
        layout.cursor_screen_y =
            Some(content_area.y + (display_cursor.saturating_sub(offset)) as u16);
        f.render_stateful_widget(
            List::new(list_items).highlight_style(Style::default()),
            content_area,
            &mut state,
        );

        if banner_rows > 0 && content_start >= offset && content_start < offset + visible {
            let banner_y = content_area.y + (content_start - offset) as u16;
            let bottom = content_area.y + content_area.height;
            let banner_h = (banner_content_rows as u16).min(bottom.saturating_sub(banner_y));
            if banner_h > 0 {
                // The banner content sits inside the colored block with
                // `COMPACT_BANNER_INDENT` cols of external side padding on
                // each side (and render_power_compact_detail's own
                // internal 1-col pad), so the poster image — right-anchored
                // inside `banner_rect` — never renders under the scrollbar
                // (which is drawn on the rightmost col afterwards).
                let banner_rect = Rect {
                    x: content_area.x + COMPACT_BANNER_INDENT,
                    y: banner_y,
                    width: content_area.width.saturating_sub(2 * COMPACT_BANNER_INDENT),
                    height: banner_h,
                };
                let want_cursor_y = layout.cursor_screen_y;
                self.render_power_compact_detail(
                    f,
                    banner_rect,
                    self.library_tab - 1,
                    focused,
                    layout,
                );
                layout.cursor_screen_y = want_cursor_y;
            }
        }

        self.render_series_detail_if_visible(
            f,
            content_area,
            offset,
            visible,
            display_cursor,
            series_detail_rows,
            self.library_tab - 1,
            focused,
            layout,
        );

        if show_scrollbar {
            let max_off = total_display.saturating_sub(visible);
            super::render_power_right_scrollbar(f, content_area, max_off, offset);
        }

        if banner_rows > 0 {
            super::render_selected_block_borders(
                f,
                content_area,
                offset,
                visible,
                banner_rule_top,
                banner_rule_bottom,
            );
        }

        Self::render_series_detail_top_border(
            f,
            content_area,
            offset,
            visible,
            display_cursor,
            series_detail_rows,
        );

        final_offset
    }
}
