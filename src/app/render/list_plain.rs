use super::super::ui_util::*;
use super::list_rows::{
    build_list_row_spans, focused_or_subtle, push_selected_detail_fillers_after,
    push_selected_detail_fillers_before, render_series_detail_background,
    selected_detail_lower_bound, DisplayRow, ListRenderCtx, COMPACT_BANNER_INDENT,
};
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
    /// Plain list kind of `render_power_list`: the catch-all covering the
    /// Home "Continue Watching" tab, search result sets, small libraries,
    /// and non-album music levels, all of which render identically without
    /// letter grouping. Returns the scroll offset to persist.
    pub(super) fn render_power_plain_rows(
        &mut self,
        f: &mut Frame,
        ctx: ListRenderCtx,
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

        let mut display_rows: Vec<DisplayRow> =
            Vec::with_capacity(n + banner_rows + series_detail_rows);
        for i in 0..n {
            push_selected_detail_fillers_before(
                &mut display_rows,
                i,
                cursor,
                banner_rows,
                series_detail_rows,
            );
            display_rows.push(DisplayRow::Item(i));
            push_selected_detail_fillers_after(
                &mut display_rows,
                i,
                cursor,
                banner_rows,
                series_detail_rows,
            );
        }
        let total_display = display_rows.len();
        let display_cursor = display_rows
            .iter()
            .position(|r| matches!(r, DisplayRow::Item(i) if *i == cursor))
            .unwrap_or(0);

        // Lower bound normally just keeps the cursor row visible; when a
        // banner or series detail follows it, extend the lower bound so
        // scrolling keeps pulling up until the whole block is visible too
        // (clamped to display_cursor itself if the viewport could never fit both).
        // For banners, `banner_rows` rows sit below the cursor (opening rule above).
        // For series, `series_detail_rows` rows sit below the cursor (block follows it).
        let lower_bound =
            selected_detail_lower_bound(display_cursor, banner_rows, series_detail_rows, visible);
        let mut offset = stored_scroll.clamp(lower_bound, display_cursor);
        // Walk back over any run of banner/series-detail filler rows
        // touching `offset` (whether `offset` lands inside the run or
        // right after it, on the Item row the fillers belong to) so the
        // offset lands at the run's first row instead of stranding the
        // block's top padding just out of view. See the analogous,
        // letter-header-aware version of this scan above for
        // `use_letter_groups`; this (ungrouped) branch never has
        // LetterHeader rows, so there's nothing further to include.
        if visible > 1 && offset > 0 {
            while offset > 0
                && matches!(
                    display_rows.get(offset - 1),
                    Some(DisplayRow::BannerFiller | DisplayRow::SeriesDetailFiller)
                )
            {
                offset -= 1;
            }
        }
        let final_offset = offset;

        // Absolute display-row indices of the colored block's top and
        // bottom padding rows (only meaningful when banner_rows > 0).
        // `banner_rule_top` is the padding row directly above the selected
        // item's own row; `banner_rule_bottom` is the padding row after
        // the banner content, before the next list row.
        let banner_rule_top = display_cursor.saturating_sub(1);
        let content_start = display_cursor + 1;
        let banner_rule_bottom = content_start + banner_rows.saturating_sub(2);
        let show_scrollbar = focused && total_display > visible;

        // The selected movie + banner are wrapped in a CONTINUE_BG colored
        // block (matching the home tab's Keep Watching look). Draw the
        // block first, before the list items, so the per-row spans only
        // paint their own cells and the block's background shows through
        // on the side padding cols and on the top/bottom padding rows.
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

        let list_items: Vec<ListItem> = display_rows
            .iter()
            .enumerate()
            .skip(offset)
            .take(visible)
            .map(|(_abs_idx, row)| match row {
                DisplayRow::Spacer | DisplayRow::LetterHeader(_) => ListItem::new(Line::default()),
                // The colored block (drawn above) frames the selected row
                // + banner, so the banner's top/bottom padding rows are
                // empty -- they show the block's background.
                DisplayRow::BannerFiller | DisplayRow::SeriesDetailFiller => {
                    ListItem::new(Line::default())
                }
                DisplayRow::Item(idx) => {
                    let item = &items[*idx];
                    let selected = *idx == cursor;

                    // Compute name and duration as separate strings so they can be styled
                    // independently: name in the normal fg, duration in OVERLAY (no parens).
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
                    } else if selected {
                        (area.width as usize).saturating_sub(1)
                    } else {
                        (area.width as usize).saturating_sub(2)
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

        layout.left_row_map = display_rows
            .iter()
            .skip(offset)
            .take(visible)
            .map(|row| match row {
                DisplayRow::Spacer
                | DisplayRow::LetterHeader(_)
                | DisplayRow::BannerFiller
                | DisplayRow::SeriesDetailFiller => None,
                DisplayRow::Item(idx) => Some(*idx),
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
                // render_power_compact_detail overwrites layout.cursor_screen_y with
                // the banner's own top row; restore the selected list row's y after,
                // since that row (not the banner) is what should host the blinking
                // cursor / mouse hit target.
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

        // White unicode borders at the block's top and bottom padding
        // rows, rendering inside the coloured block.
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
