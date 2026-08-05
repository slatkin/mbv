use super::detail::compact_banner_image_cache_key;
use super::list_rows::ListRenderCtx;
use crate::app::layout::LayoutMain;
use crate::app::{palette, App};
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;

/// Cap on the hero image's height in rows (design decision 3, option a): at
/// full content width the 16:9 formula would otherwise grow the hero to the
/// whole list in wide terminals. 12 image rows + 1 gap + 5 meta rows keeps
/// the hero at ≤ 18 rows and leaves the list a few rows at any width.
const HERO_IMAGE_CAP_ROWS: u16 = 12;
/// Blank row between the hero image and the meta block below it.
const HERO_GAP_ROWS: u16 = 1;
/// Row budget for the meta block under the hero image (meta line, spacer,
/// overview/director lines).
const HERO_META_ROWS: u16 = 5;

/// Height of the top hero banner for a content area `width` columns wide:
/// the poster image at 16:9 in terminal cells (cells are roughly twice as
/// tall as they are wide, so 9 rows per 32 columns — the home view's
/// formula), capped at `HERO_IMAGE_CAP_ROWS`, plus a 1-row gap and the meta
/// block. The hero grows with the terminal until it hits the cap.
fn hero_height_for_width(width: u16) -> u16 {
    let image_height = ((width as u32 * 9 + 31) / 32)
        .max(1)
        .min(HERO_IMAGE_CAP_ROWS as u32) as u16;
    image_height + HERO_GAP_ROWS + HERO_META_ROWS
}

impl App {
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

        // Column count for the two-column list layout, derived from the list
        // pane width -- the content area this renderer already receives,
        // which excludes the queue column and widens when
        // `queue_column_collapsed` is set, so both feed through with no
        // separate code path. Season grids keep their own single-column
        // stride (see `is_viewing_season_grid`).
        let cols = if self.library_tab > 0 && self.is_viewing_season_grid(self.library_tab - 1) {
            1
        } else {
            crate::app::library_column_width::library_column_count(content_area.width)
        };

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
                        Style::default().fg(palette::BG_GREEN),
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

        // ── Hero on top, list below ──────────────────────────────────────
        // The selected item's banner (poster + meta + overview) gets the top
        // of the content area at full width; the list renderer gets the rows
        // below it. The hero's height comes from the poster's 16:9 aspect
        // (capped so the list keeps a few rows in wide terminals — design
        // decision 3, option a). No hero when nothing is selected (e.g. an
        // empty list) or when the selected item has no banner (folders,
        // music) — the list then takes the whole content area.
        let hero_rows: u16 = if self.library_tab > 0 {
            let lib_idx = self.library_tab - 1;
            let hero_item = self.power_selected_movie_item(lib_idx).is_some()
                || self.power_selected_series_item(lib_idx).is_some();
            if hero_item {
                hero_height_for_width(content_area.width)
            } else {
                0
            }
        } else {
            0
        };
        let hero_area = Rect {
            x: content_area.x,
            y: content_area.y,
            width: content_area.width,
            height: hero_rows,
        };
        let list_area = Rect {
            x: content_area.x,
            y: content_area.y + hero_rows,
            width: content_area.width,
            height: content_area.height.saturating_sub(hero_rows),
        };
        layout.hero_area = hero_area;
        layout.left_area = list_area;

        // Paint the hero first; the list renderer below overwrites
        // `cursor_screen_y` with the selected row, so the blinking cursor /
        // mouse hit target stays on the list row, not the hero.
        if hero_rows > 0 {
            self.render_power_compact_detail(f, hero_area, self.library_tab - 1, focused, layout);
        }

        let final_offset: usize;

        if show_grouped {
            let lib_idx = self.library_tab - 1;
            final_offset = self.render_power_grouped_album_rows(
                f,
                list_area,
                lib_idx,
                &items,
                cursor,
                stored_scroll,
                focused,
                layout,
            );
        } else if use_letter_groups {
            let ctx = ListRenderCtx {
                content_area: list_area,
                items: &items,
                cursor,
                stored_scroll,
                cols,
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
                content_area: list_area,
                items: &items,
                cursor,
                stored_scroll,
                cols,
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
