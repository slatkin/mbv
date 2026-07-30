use super::detail::compact_banner_image_cache_key;
use super::list_rows::{
    ListRenderCtx, COMPACT_BANNER_GAP_ROWS, COMPACT_BANNER_INDENT, COMPACT_BANNER_RULE_ROWS,
    COMPACT_MOVIE_BANNER_INDENT, SELECTED_BLOCK_SIDE_PADDING,
};
use crate::app::layout::LayoutMain;
use crate::app::{palette, App};
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;

impl App {
    /// Filler-row count to reserve around the selected movie's row in
    /// `lib_idx`'s display-row sequence: the colored block's top/bottom
    /// padding rows plus the banner's actual content height
    /// (meta/overview/director wrapped to `panel_width`, computed by
    /// `compact_banner_layout` — #263 replaced the old fixed content-row
    /// constant with this, so a longer overview grows the reserved space and
    /// a shorter one shrinks it) when a leaf movie is selected, else 0 (no
    /// banner — ordinary list rendering). One of the reserved rows is the
    /// top padding placed immediately *before* the selected item's row; the
    /// rest (content + bottom padding) follow it.
    ///
    /// `panel_width` matches the banner's eventual `Rect` width
    /// (`content_area.width - 2 * COMPACT_BANNER_INDENT` — see
    /// `render_power_compact_detail`'s inner padding), so the row count the
    /// layout reserves and the rows the banner actually renders stay in
    /// lockstep.
    fn compact_banner_rows(&mut self, lib_idx: usize, panel_width: u16) -> usize {
        let Some(item) = self.power_selected_movie_item(lib_idx) else {
            return 0;
        };
        let content_rows = self
            .compact_banner_layout(&item, panel_width)
            .content_rows();
        COMPACT_BANNER_RULE_ROWS + content_rows + COMPACT_BANNER_GAP_ROWS
    }

    pub(super) fn render_series_detail_if_visible(
        &mut self,
        f: &mut Frame,
        content_area: Rect,
        offset: usize,
        visible: usize,
        display_cursor: usize,
        series_detail_rows: usize,
        lib_idx: usize,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        if series_detail_rows == 0 {
            return;
        }
        let content_start = display_cursor + 1;
        if content_start < offset || content_start >= offset + visible {
            return;
        }

        let detail_y = content_area.y + (content_start - offset) as u16;
        let bottom = content_area.y + content_area.height;
        let detail_h = (series_detail_rows as u16).min(bottom.saturating_sub(detail_y));
        if detail_h == 0 {
            return;
        }

        self.render_series_inline_detail(
            f,
            Rect {
                x: content_area.x + SELECTED_BLOCK_SIDE_PADDING,
                y: detail_y,
                width: content_area
                    .width
                    .saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING),
                height: detail_h,
            },
            lib_idx,
            focused,
            layout,
        );
    }

    pub(super) fn render_series_detail_top_border(
        f: &mut Frame,
        content_area: Rect,
        offset: usize,
        visible: usize,
        display_cursor: usize,
        series_detail_rows: usize,
    ) {
        if series_detail_rows == 0
            || display_cursor < 2
            || display_cursor - 2 < offset
            || display_cursor - 2 >= offset + visible
        {
            return;
        }

        let border_y = content_area.y + (display_cursor - 2 - offset) as u16;
        f.render_widget(
            Paragraph::new(Span::styled(
                "\u{2581}".repeat(content_area.width as usize),
                Style::default().fg(palette::SEEK_TRACK),
            )),
            Rect {
                x: content_area.x,
                y: border_y,
                width: content_area.width,
                height: 1,
            },
        );
    }

    /// Renders the Continue/library list items into `area`.
    /// The title header is now drawn in the top-of-screen FOAM bar.
    pub(super) fn render_power_list(
        &mut self,
        f: &mut Frame,
        area: Rect,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        if area.height == 0 {
            return;
        }

        // Ensure the library is loaded when a library tab is selected.
        if self.library_tab > 0 {
            self.ensure_lib_loaded_for(self.library_tab - 1);
        }

        let mut content_area = area;

        // Store for click / page-size calculations.
        layout.left_area = content_area;

        // Gather items, cursor, stored scroll offset, and the *true* library total
        // (not just how many pages have been fetched so far) from the appropriate
        // source.
        let (items, cursor, stored_scroll, total_count) = if self.library_tab == 0 {
            let items = self.home.continue_items.clone();
            let cursor = self.home.continue_cursor.min(items.len().saturating_sub(1));
            let total = items.len();
            (items, cursor, 0usize, total)
        } else {
            let lib_idx = self.library_tab - 1;
            let lib = &self.libs[lib_idx];
            let (items, cur, scroll, total) = if let Some(s) = &lib.search {
                let items: Vec<mbv_core::api::MediaItem> = s
                    .results
                    .iter()
                    .filter_map(|&i| {
                        s.items
                            .get(i)
                            .map(|item| self.recursive_album_display_item(lib_idx, i, item.clone()))
                    })
                    .collect();
                // Search results are already the full locally-filtered match set,
                // not paginated, so their length is already the true total.
                let total = items.len();
                (items, s.cursor, s.scroll, total)
            } else {
                match lib.nav_stack.last() {
                    // `total_count` comes from Emby's TotalRecordCount, not
                    // `items.len()` -- with lazy pagination `items` may only hold
                    // a subset of the library until the user scrolls further.
                    Some(lvl) => (lvl.items.clone(), lvl.cursor, lvl.scroll, lvl.total_count),
                    None => (vec![], 0, 0, 0),
                }
            };
            (items, cur, scroll, total)
        };

        // Reserved filler-row count for the compact movie banner, 0 for every
        // library type/state except "leaf movie selected, detail not pinned".
        // The width estimate matches the final banner rect's width:
        // `content_area.width.saturating_sub(2 * COMPACT_BANNER_INDENT)` (= the
        // colored block's width minus the external side padding, with the right
        // external pad covering the scrollbar column when one shows up).
        let banner_rows: usize = if self.library_tab > 0 {
            let banner_panel_width = content_area
                .width
                .saturating_sub(1)
                .saturating_sub(COMPACT_MOVIE_BANNER_INDENT);
            self.compact_banner_rows(self.library_tab - 1, banner_panel_width)
        } else {
            0
        };
        // Content-only row count (banner_rows minus its top/bottom colored-pad
        // filler rows), used below to size the banner rect to the same
        // content-dependent height that was reserved for it above.
        let banner_content_rows: usize =
            banner_rows.saturating_sub(COMPACT_BANNER_RULE_ROWS + COMPACT_BANNER_GAP_ROWS);

        // Series inline detail rows: when a TV show Series is selected,
        // show its metadata/overview inline below the selected row.
        let series_detail_rows: usize = if self.library_tab > 0 && banner_rows == 0 {
            let lib_idx = self.library_tab - 1;
            if let Some(item) = self.power_selected_series_item(lib_idx) {
                let panel_width = content_area
                    .width
                    .saturating_sub(1)
                    .saturating_sub(COMPACT_BANNER_INDENT);
                let (in_selection, episode_count) = self.series_selection_state(lib_idx, &item.id);
                self.series_inline_detail_rows(&item, panel_width, in_selection, episode_count)
            } else {
                0
            }
        } else {
            0
        };

        // Pre-warm nearby movies' poster images so they're already cached by
        // the time the cursor reaches them (#287) -- mirrors the prefetch
        // window `render_power_card` already uses for the home-card
        // carousel. Only applies when a movie banner is actually showing
        // (i.e. this is a movies library with a leaf Movie selected); if
        // there's no banner, there's nothing to prefetch for.
        if self.library_tab > 0 {
            let lib_idx = self.library_tab - 1;
            if self.power_selected_movie_item(lib_idx).is_some() {
                const PREFETCH_AHEAD: usize = 3;
                const PREFETCH_BEHIND: usize = 1;
                let start = cursor.saturating_sub(PREFETCH_BEHIND);
                let end = (cursor + PREFETCH_AHEAD + 1).min(items.len());
                let prefetch: Vec<(String, String, String)> = items[start..end]
                    .iter()
                    .enumerate()
                    .filter(|(i, item)| {
                        start + i != cursor && item.item_type == "Movie" && !item.is_folder
                    })
                    .map(|(_, item)| {
                        (
                            compact_banner_image_cache_key(&item.id),
                            item.id.clone(),
                            item.series_id.clone(),
                        )
                    })
                    .collect();
                if self.images_enabled() {
                    for (cache_key, item_id, series_id) in prefetch {
                        self.fetch_list_card_image_when_idle(
                            cache_key,
                            item_id,
                            series_id,
                            &["Primary"],
                        );
                    }
                }
            }
        }

        // When at the album level of a music library, group albums under artist headers.
        let show_grouped = if self.library_tab > 0 {
            self.is_viewing_album_folders(self.library_tab - 1)
        } else {
            false
        };

        let n = items.len();

        // Letter grouping: applies to non-music library lists with 50+ items (not during search).
        // Gated on the true library total (`LibraryTab.library_total` when known,
        // e.g. a letter-range pill has scoped the fetch to a smaller slice),
        // not the fetched-so-far/filtered count, so the grouping style (ranges
        // vs. individual letters) doesn't change out from under the user as
        // more pages lazily load in, and a small filtered slice (< 50 items)
        // still shows headers.
        let active_letter_filter = if self.library_tab > 0 {
            self.libs[self.library_tab - 1]
                .nav_stack
                .last()
                .and_then(|l| l.letter_filter.as_ref())
                .cloned()
        } else {
            None
        };
        let ungrouped_total = self
            .library_tab
            .checked_sub(1)
            .map_or(total_count, |lib_idx| {
                self.libs[lib_idx].library_total.unwrap_or(total_count)
            });
        let use_letter_groups = !show_grouped
            && self.library_tab > 0
            && (ungrouped_total >= 50 || active_letter_filter.is_some())
            && {
                let lib_idx = self.library_tab - 1;
                self.libs[lib_idx].library.collection_type != "music"
                    && self.libs[lib_idx].search.is_none()
            };

        // First row area: search input box (when searching).
        if focused && self.library_tab > 0 && content_area.height > 0 {
            let lib_idx = self.library_tab - 1;
            let has_search = self.libs[lib_idx].search.is_some();
            if has_search && content_area.height >= 3 {
                // 3-row bordered search input, matching the home-search visual style.
                let search_area = Rect {
                    height: 3,
                    ..content_area
                };
                content_area = Rect {
                    y: content_area.y + 3,
                    height: content_area.height.saturating_sub(3),
                    ..content_area
                };
                let s = self.libs[lib_idx].search.as_ref().unwrap();
                let input_text = if s.loading {
                    format!("{}█ [loading…]", s.query)
                } else {
                    format!("{}█", s.query)
                };
                f.render_widget(
                    Paragraph::new(Span::styled(
                        input_text,
                        Style::default().fg(palette::GREEN),
                    ))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(palette::IRIS))
                            .title(Span::styled(
                                " Search ",
                                Style::default().fg(palette::YELLOW),
                            )),
                    ),
                    search_area,
                );
            }
        }

        if n == 0 {
            let msg = if self.library_tab > 0 {
                let lib_idx = self.library_tab - 1;
                if self.recursive_album_search_enabled(lib_idx)
                    && self.libs[lib_idx]
                        .search
                        .as_ref()
                        .is_some_and(|search| search.loading)
                {
                    "Indexing music library..."
                } else if self.libs[lib_idx]
                    .nav_stack
                    .last()
                    .map(|l| l.loading)
                    .unwrap_or(false)
                {
                    "Loading..."
                } else {
                    "(empty)"
                }
            } else {
                "(empty)"
            };
            super::render_power_placeholder(f, content_area, msg);
            return;
        }

        let final_offset: usize;

        if show_grouped {
            let lib_idx = self.library_tab - 1;
            final_offset = self.render_power_grouped_album_rows(
                f,
                content_area,
                lib_idx,
                &items,
                cursor,
                stored_scroll,
                focused,
                layout,
            );
        } else if use_letter_groups {
            let ctx = ListRenderCtx {
                area,
                content_area,
                items: &items,
                cursor,
                stored_scroll,
                banner_rows,
                banner_content_rows,
                series_detail_rows,
                focused,
            };
            final_offset = self.render_power_letter_grouped_rows(
                f,
                ctx,
                active_letter_filter,
                ungrouped_total,
                layout,
            );
        } else {
            let ctx = ListRenderCtx {
                area,
                content_area,
                items: &items,
                cursor,
                stored_scroll,
                banner_rows,
                banner_content_rows,
                series_detail_rows,
                focused,
            };
            final_offset = self.render_power_plain_rows(f, ctx, layout);
        }

        // Persist the scroll offset so the viewport is remembered across frames.
        // library_tab is always > 0 here (tab == 0 uses render_power_home_list).
        if self.library_tab > 0 {
            let lib_idx = self.library_tab - 1;
            if let Some(s) = &mut self.libs[lib_idx].search {
                s.scroll = final_offset;
            } else if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut() {
                lvl.scroll = final_offset;
            }
        }
    }
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
