use super::detail::compact_banner_image_cache_key;
use super::list_rows::{ListRenderCtx, SELECTED_BLOCK_SIDE_PADDING};
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
/// Row budget for the selected item's title on the hero's top row, rendered
/// in yellow. Reserved only in two-column lists (`show_title`), where the
/// list row's own title is truncated to a narrow cell; one-column lists
/// skip it since the full-width row title right above the hero already shows
/// the name.
const HERO_TITLE_ROWS: u16 = 1;
/// Rows the hero *block* adds beyond the content rows, matching the
/// selected-block look of music/homevideo: a `▁` top border row and a `▔`
/// bottom border row (painted in `palette::SEEK_TRACK`) plus one bare
/// colored-bg padding row just inside each border. The borders are part of
/// the hero block's reserved rows (the list makes room), not painted over
/// list content like `render_selected_block_borders` does.
const HERO_BLOCK_EXTRA_ROWS: u16 = 4;

/// Height of the top hero banner for a content area `width` columns wide:
/// the poster image at 16:9 in terminal cells (cells are roughly twice as
/// tall as they are wide, so 9 rows per 32 columns — the home view's
/// formula), capped at `HERO_IMAGE_CAP_ROWS`, plus (in two-column lists) a
/// 1-row yellow title, a 1-row gap, and the meta block. The hero grows with
/// the terminal until it hits the cap.
fn hero_height_for_width(width: u16, show_title: bool) -> u16 {
    let image_height = (width as u32 * 9)
        .div_ceil(32)
        .max(1)
        .min(HERO_IMAGE_CAP_ROWS as u32) as u16;
    image_height
        + HERO_TITLE_ROWS.saturating_mul(show_title as u16)
        + HERO_GAP_ROWS
        + HERO_META_ROWS
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

        let content_area = area;

        // Selected movie/Series item, computed once and reused below for the
        // prefetch gate, the hero row-count calc, and the hero paint --
        // `power_selected_movie_item`/`power_selected_series_item` each clone
        // the whole `MediaItem`, so one call keeps that to a single clone per
        // frame instead of three.
        let selected_movie_item = if self.library_tab > 0 {
            self.power_selected_movie_item(self.library_tab - 1)
        } else {
            None
        };
        let selected_series_item = if selected_movie_item.is_none() && self.library_tab > 0 {
            self.power_selected_series_item(self.library_tab - 1)
        } else {
            None
        };

        // Column count for the two-column list layout, derived from the list
        // pane width -- the content area this renderer already receives,
        // which excludes the queue column and widens when the panel mode is
        // not `Both`, so both feed through with no
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
            let (items, cur, scroll, total) = match lib.nav_stack.last() {
                // `total_count` comes from Emby's TotalRecordCount, not
                // `items.len()` -- with lazy pagination `items` may only hold
                // a subset of the library until the user scrolls further.
                Some(lvl) => (lvl.items.clone(), lvl.cursor, lvl.scroll, lvl.total_count),
                None => (vec![], 0, 0, 0),
            };
            (items, cur, scroll, total)
        };

        // Pre-warm nearby movies' poster images so they're already cached by
        // the time the cursor reaches them (#287) -- mirrors the prefetch
        // window `render_power_card` already uses for the home-card
        // carousel. Only applies when a movie banner is actually showing
        // (i.e. this is a movies library with a leaf Movie selected); if
        // there's no banner, there's nothing to prefetch for.
        if selected_movie_item.is_some() {
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
            };

        layout.left_area = content_area;

        if n == 0 {
            let msg = if self.library_tab > 0 {
                let lib_idx = self.library_tab - 1;
                if self.recursive_album_search_enabled(lib_idx) {
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

        // ── Hero inline, list wraps around it ──────────────────────────
        // The selected item's banner (poster + meta + overview, or --  for a
        // selected Series -- the season pills + episode table) is painted
        // full-width just below the row containing the selected item; the
        // list renderer packs rows above and below it (the hero occupies
        // `hero_rows` blank `DisplayRow::Hero` rows, painted over
        // afterwards). The block grows the content height by 4 so the
        // `▁`/`▔` SEEK_TRACK borders and the two bare colored-bg padding
        // rows are *inside* the hero block's reserved rows (the list makes
        // room; nothing gets painted over). No hero when nothing is
        // selected (e.g. an empty list) or when the selected item has no
        // banner (folders, music) — the list then takes the whole content
        // area.
        //
        // Movies get the poster/meta/overview content sized by the image's
        // 16:9 aspect, capped so the list keeps a few rows in wide
        // terminals (design decision 3, option a). A selected Series keeps
        // its own inline detail (season pills + episode table,
        // `series_inline_detail_rows` / `render_series_inline_detail`) --
        // that's a distinct, taller, interactive content shape the generic
        // compact banner can't represent, so it isn't folded into the
        // movie hero's row math.
        let hero_rows: u16 = if self.library_tab > 0 {
            let lib_idx = self.library_tab - 1;
            if selected_movie_item.is_some() {
                hero_height_for_width(content_area.width, cols > 1) + HERO_BLOCK_EXTRA_ROWS
            } else if let Some(item) = &selected_series_item {
                let (in_selection, episode_count) = self.series_selection_state(lib_idx, &item.id);
                self.series_inline_detail_rows(
                    item,
                    content_area.width,
                    cols > 1,
                    in_selection,
                    episode_count,
                ) as u16
                    + HERO_BLOCK_EXTRA_ROWS
            } else {
                0
            }
        } else {
            0
        };

        // The list renderer gets the whole content area; the hero is
        // inserted inline below the selected row, not above the list.
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
                content_area,
                items: &items,
                cursor,
                stored_scroll,
                cols,
                focused,
                hero_rows,
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
                content_area,
                items: &items,
                cursor,
                stored_scroll,
                cols,
                focused,
                hero_rows,
            };
            final_offset = self.render_power_plain_rows(f, ctx, layout);
        }

        // Paint the hero last, over the blank `DisplayRow::Hero` rows the
        // list renderer left: the colored bg (focused/unfocused pattern,
        // matching music/homevideo's selected block), then the `▁` top and
        // `▔` bottom borders in SEEK_TRACK on the block's outer rows, then
        // the content offset 2 rows down past the top border + top padding.
        // The row renderer has already overwritten `cursor_screen_y` with
        // the selected list row (the blinking cursor / mouse hit target
        // stays on the list row, not the hero), so save and restore it
        // around the hero paint.
        if hero_rows > 0 {
            let saved_cursor_y = layout.cursor_screen_y;
            let bg = if focused {
                palette::MEDIA_SELECTED_BG
            } else {
                palette::PLAYBACK_PANEL_BG
            };
            // Colored bg across the padding + content rows (inside the
            // borders, i.e. `hero_rows - 2` rows starting one row down).
            f.render_widget(
                Block::default().style(Style::default().bg(bg)),
                Rect {
                    x: layout.hero_area.x,
                    y: layout.hero_area.y + 1,
                    width: layout.hero_area.width,
                    height: hero_rows - 2,
                },
            );
            // Top `▁` / bottom `▔` borders in SEEK_TRACK, painted on the
            // hero block's own outer rows (the list above and below made
            // room for them).
            let border_style = Style::default().fg(palette::SEEK_TRACK);
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "\u{2581}".repeat(layout.hero_area.width as usize),
                    border_style,
                ))),
                Rect {
                    x: layout.hero_area.x,
                    y: layout.hero_area.y,
                    width: layout.hero_area.width,
                    height: 1,
                },
            );
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "\u{2594}".repeat(layout.hero_area.width as usize),
                    border_style,
                ))),
                Rect {
                    x: layout.hero_area.x,
                    y: layout.hero_area.y + hero_rows - 1,
                    width: layout.hero_area.width,
                    height: 1,
                },
            );
            // Content, offset 2 rows down past the top border + top
            // padding, and inset 2 cols on each side like music/homevideo's
            // selected blocks; the banner layout is a pure function of the
            // panel width, so this paints the same content as before.
            let content_rect = Rect {
                x: layout.hero_area.x + SELECTED_BLOCK_SIDE_PADDING,
                y: layout.hero_area.y + 2,
                width: layout
                    .hero_area
                    .width
                    .saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING),
                height: hero_rows - HERO_BLOCK_EXTRA_ROWS,
            };
            let lib_idx = self.library_tab - 1;
            // Same movie/Series branch as the row-count calc above: a
            // selected Series renders its season pills + episode table
            // instead of the generic compact banner.
            if selected_movie_item.is_some() {
                self.render_power_compact_detail(
                    f,
                    content_rect,
                    lib_idx,
                    focused,
                    cols > 1,
                    layout,
                );
            } else {
                self.render_series_inline_detail(
                    f,
                    content_rect,
                    lib_idx,
                    focused,
                    cols > 1,
                    layout,
                );
            }
            layout.cursor_screen_y = saved_cursor_y;
        }

        // Persist the scroll offset so the viewport is remembered across frames.
        // library_tab is always > 0 here (tab == 0 uses render_power_home_list).
        if self.library_tab > 0 {
            let lib_idx = self.library_tab - 1;
            if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut() {
                lvl.scroll = final_offset;
            }
        }
    }
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
