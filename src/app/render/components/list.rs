use super::detail::compact_banner_image_cache_key;
use crate::app::layout::LayoutMain;
use crate::app::library_column_width::library_column_count;
use crate::app::render::arrangements::hero_left;
use crate::app::render::arrangements::library;
use crate::app::render::components::album::AlbumRowsCursorCtx;
use crate::app::render::components::hero::{
    selected_detail_shell, HERO_BLOCK_EXTRA_ROWS, HERO_PLACEHOLDER_ROWS, HERO_TITLE_ROWS,
};
use crate::app::render::components::list_rows::{
    LibraryListRenderCtx, ListRenderCtx, SELECTED_BLOCK_SIDE_PADDING,
};
use crate::app::App;
use ratatui::layout::Rect;
use ratatui::Frame;

pub(in crate::app) fn render_generic_movies_home_video_rows_with_ctx(
    f: &mut Frame,
    list_area: Rect,
    ctx: &LibraryListRenderCtx,
    focused: bool,
    layout: &mut LayoutMain,
) -> usize {
    layout.left_area = list_area;
    if ctx.items.is_empty() {
        crate::app::render::render_placeholder(
            f,
            list_area,
            if ctx.loading {
                " Loading…"
            } else {
                " (empty)"
            },
        );
        return 0;
    } else {
        let row_ctx = ctx.rows(list_area, library_column_count(list_area.width), focused, 0);
        if !ctx.is_search_active() && (ctx.true_total() >= 50 || ctx.letter_filter.is_some()) {
            super::list_letter_groups::render_letter_grouped_rows(
                f,
                row_ctx,
                ctx.letter_filter.clone(),
                ctx.true_total(),
                layout,
            )
        } else {
            super::list_plain::render_plain_rows(f, row_ctx, layout)
        }
    }
}

impl App {
    /// Renders the Continue/library list items into `area`.
    /// The title header is now drawn in the top-of-screen FOAM bar.
    pub(in crate::app::render) fn render_list(
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
        if let Some(lib_idx) = self.tab.emby_library_index() {
            self.ensure_lib_loaded_for(lib_idx);
        }

        // Wide grouped Music uses a Music-specific horizontal split at or
        // above the shared 82-column breakpoint. The narrow path
        // (below breakpoint) continues unchanged through the existing hero-
        // above-list renderer.
        if let Some(lib_idx) = self.tab.emby_library_index() {
            if self.is_music_group_view(lib_idx)
                && self.is_viewing_album_folders(lib_idx)
                && crate::app::render::arrangements::hero_left::shared_hero_presentation(area)
                    .is_some()
            {
                if let Some(album) = self.selected_album_item(lib_idx) {
                    if !self.album_tracks_cache.contains_key(&album.id)
                        && !self.album_tracks_loading.contains(&album.id)
                    {
                        self.fetch_album_tracks(album.id);
                    }
                }
                let ctx = self.wide_music_render_ctx(lib_idx);
                let output =
                    super::music_wide::render_wide_music_group_with_ctx(f, area, &ctx, layout);
                if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
                    level.scroll = output.final_scroll;
                }
                self.paint_music_image(f, output.image_paint);
                return;
            }
        }

        // Wide Movies: the dedicated Movies library renders hero-on-left
        // (right-panel-arrangements spec) at or above the shared breakpoint:
        // read-only shared selected-Emby hero card on the left, letter pills
        // and one-column list in the right rail. Below the breakpoint the
        // inline presentation below runs unchanged. Height floor mirrors
        // the other hero-on-left screens.
        if let Some(lib_idx) = self.tab.emby_library_index() {
            if (self.is_wide_movies_library(lib_idx) || self.is_home_video_view(lib_idx))
                && crate::app::render::arrangements::hero_left::shared_hero_presentation(area)
                    .is_some()
            {
                let ctx = self.library_list_render_ctx(lib_idx, false);
                let selected_movie = self.selected_wide_movie(lib_idx, &ctx);
                self.render_wide_movies_with_ctx(
                    f,
                    area,
                    lib_idx,
                    focused,
                    &ctx,
                    selected_movie.as_ref(),
                    layout,
                );
                return;
            }
        }

        if let Some(lib_idx) = self.tab.emby_library_index() {
            if (self.is_wide_tv_library(lib_idx) || self.is_podcast_library(lib_idx))
                && crate::app::render::arrangements::hero_left::shared_hero_presentation(area)
                    .is_some()
            {
                let ctx = self.wide_tv_render_ctx(lib_idx, focused);
                let final_scroll = super::tv_wide::render_wide_tv_with_ctx(f, area, &ctx, layout);
                if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
                    level.scroll = final_scroll;
                }
                return;
            }
        }

        let mut content_area = area;

        // Search is active for the focused Emby library. Its 3-row input box
        // is placed outside the selected replacement in inline view (see the block after
        // `placement-neutral geometry`), so here we only record that it's on.
        let library_ctx = self
            .tab
            .emby_library_index()
            .map(|lib_idx| self.library_list_render_ctx(lib_idx, true));
        let search_active = focused
            && library_ctx
                .as_ref()
                .is_some_and(LibraryListRenderCtx::is_search_active);

        // Home videos' declared element-presence difference (design.md
        // decision 6): a count label row instead of the letter-pill row
        // every other inline browser may show (`should_show_letter_pills`
        // already excludes home videos, so the two never both show).
        if focused
            && content_area.height > 0
            && self
                .tab
                .emby_library_index()
                .is_some_and(|lib_idx| self.is_home_video_view(lib_idx))
        {
            let lib_idx = self.tab.emby_library_index().unwrap();
            let total = self.libs[lib_idx]
                .nav_stack
                .last()
                .map(|l| l.total_count)
                .unwrap_or(0);
            content_area = crate::app::render::render_count_label(f, content_area, total);
            // Leave the blank gap row `placement-neutral geometry`'s `hero_shift`
            // expects directly above `content_area`, so the hero's top
            // border reclaims that row instead of overwriting the label.
            content_area = Rect {
                y: content_area.y + 1,
                height: content_area.height.saturating_sub(1),
                ..content_area
            };
        }

        // Selected movie/Series item, computed once and reused below for the
        // prefetch gate, the hero row-count calc, and the hero paint --
        // `selected_movie_item`/`selected_series_item` each clone
        // the whole `EmbyItem`, so one call keeps that to a single clone per
        // frame instead of three.
        let selected_movie_item = self
            .tab
            .emby_library_index()
            .and_then(|lib_idx| self.selected_movie_item(lib_idx));
        let selected_series_item = if selected_movie_item.is_none() {
            self.tab
                .emby_library_index()
                .and_then(|lib_idx| self.selected_series_item(lib_idx))
        } else {
            None
        };
        // Column count for the two-column list layout, derived from the list
        // pane width -- the content area this renderer already receives,
        // which excludes the queue column and widens when the panel mode is
        // not `Both`, so both feed through with no
        // separate code path. Season grids keep their own single-column
        // stride (see `is_viewing_season_grid`).
        let cols = if self
            .tab
            .emby_library_index()
            .is_some_and(|lib_idx| self.is_viewing_season_grid(lib_idx))
        {
            1
        } else {
            crate::app::library_column_width::library_column_count(content_area.width)
        };

        let use_shared_replacement_plan = self.tab.emby_library_index().is_some_and(|lib_idx| {
            matches!(
                self.libs[lib_idx].library.collection_type.as_str(),
                "movies" | "tvshows"
            )
        });

        // Size selected detail from the same content declarations used by the
        // painters. Inline callers replace the selected source row in flow;
        // wide callers return before this branch and paint the detail pane.
        //
        let mut inline_hero_rows: u16 = if self.tab.emby_library_index().is_some() {
            let lib_idx = self.tab.emby_library_index().unwrap();
            if let Some(item) = &selected_movie_item {
                // Actual content rows the banner will paint (meta line,
                // overview/director text, never fewer than the poster's own
                // rendered height -- `CompactBannerLayout::content_rows`),
                // not a width-derived guess: a 16:9-shaped estimate badly
                // overshot real posters, which are portrait (2:3) and sized
                // by `IMG_COLS x IMG_ROWS` (detail.rs), leaving a block full
                // of empty rows below short overviews.
                let panel_width = content_area
                    .width
                    .saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING);
                let truncate_overview =
                    self.is_home_video_view(lib_idx) || self.is_podcast_library(lib_idx);
                let content_rows = self
                    .compact_banner_layout_with_overview(item, panel_width, truncate_overview)
                    .content_rows_with_title(HERO_TITLE_ROWS.saturating_mul((cols > 1) as u16))
                    as u16;
                content_rows + HERO_BLOCK_EXTRA_ROWS
            } else if let Some(item) = &selected_series_item {
                self.series_inline_detail_rows(item, content_area.width, cols > 1) as u16
                    + HERO_BLOCK_EXTRA_ROWS
            } else {
                // No banner content to size to. If we're at the top browse
                // level of a hero-capable collection (movies/homevideos/
                // podcasts/tvshows/music), keep the fixed placeholder panel
                // reserved instead of collapsing to zero -- a letter-pill
                // switch clears the slice before its replacement loads, and
                // this keeps the slot from jumping away and back. The
                // placeholder size is just the stand-in; once content lands
                // the block sizes to it.
                let hero_placeholder_level = self.libs[lib_idx].nav_stack.len() == 1
                    && matches!(
                        self.libs[lib_idx].library.collection_type.as_str(),
                        "movies" | "homevideos" | "podcasts" | "tvshows" | "music"
                    );
                // Music with levels shows its hero at the album-browsing
                // level (nav_stack.len() >= 2) instead of the top browse
                // level, so it gets its own placeholder gate while the
                // album list is still loading.
                let music_hero_placeholder = self.is_music_group_view(lib_idx)
                    && self.libs[lib_idx]
                        .nav_stack
                        .last()
                        .map(|l| l.items.is_empty())
                        .unwrap_or(false);
                if hero_placeholder_level || music_hero_placeholder {
                    HERO_PLACEHOLDER_ROWS
                } else {
                    0
                }
            }
        } else {
            0
        };
        if !use_shared_replacement_plan {
            inline_hero_rows = if inline_hero_rows > HERO_BLOCK_EXTRA_ROWS
                && inline_hero_rows < content_area.height
            {
                inline_hero_rows
            } else {
                0
            };
        }
        // Browser-level pills and search controls stay outside the selected
        // replacement: letter-range pills for large non-music libraries, or
        // the music-group selector while browsing a group's albums. Both share
        // one slot since a library is never both at once.
        let show_letter_pills = self
            .tab
            .emby_library_index()
            .is_some_and(|lib_idx| self.should_show_letter_pills(lib_idx));
        let show_music_pills = self
            .tab
            .emby_library_index()
            .is_some_and(|lib_idx| self.is_music_group_view(lib_idx));
        // The pill row and search box occupy the same browser-control slot;
        // search takes precedence while filtering.
        let show_pills = show_letter_pills || show_music_pills || search_active;
        // Narrow library heroes belong to the scrolling list. Keep the shared
        // pill geometry without reserving an additional detail region.
        let (pills_area, list_area) = if show_pills {
            let areas = hero_left::pill_bar_areas(content_area);
            (areas.pills_area, areas.content_area)
        } else {
            (Rect::default(), content_area)
        };
        if !use_shared_replacement_plan {
            inline_hero_rows = if inline_hero_rows > HERO_BLOCK_EXTRA_ROWS
                && inline_hero_rows < list_area.height
            {
                inline_hero_rows
            } else {
                0
            };
        }
        if show_letter_pills && !search_active {
            let lib_idx = self.tab.emby_library_index().unwrap();
            self.render_letter_pills_row(f, pills_area, lib_idx, layout);
        } else if show_music_pills && !search_active {
            let lib_idx = self.tab.emby_library_index().unwrap();
            self.render_music_group_pills_row(f, pills_area, lib_idx, layout);
        }

        // The search box occupies the exact one-row slot the pill bar would
        // sit in (`pills_area`); the gap row and list below it are reserved
        // together with the pill row.
        if search_active {
            let ctx = library_ctx.as_ref().expect("library context for search");
            crate::app::render::components::hero::render_search_box(
                f,
                pills_area,
                ctx.search_query.as_deref().unwrap_or_default(),
                ctx.search_loading,
            );
        }

        // Gather items, cursor, stored scroll offset, and the *true* library total
        // (not just how many pages have been fetched so far) from the appropriate
        // source.
        let (items, cursor, stored_scroll, total_count) = if self.tab.is_home() {
            let items = self.home.continue_items.clone();
            let cursor = self.home.continue_cursor.min(items.len().saturating_sub(1));
            let total = items.len();
            (items, cursor, 0usize, total)
        } else {
            let ctx = library_ctx
                .as_ref()
                .expect("library context for library tab");
            // The context has already selected the active source and applied the
            // recursive-album display projection used by the narrow browser.
            (ctx.items.clone(), ctx.cursor, ctx.scroll, ctx.total_count)
        };

        // Pre-warm nearby movies' poster images so they're already cached by
        // the time the cursor reaches them (#287) -- mirrors the prefetch
        // window `render_card` already uses for the home-card
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
        // Suppressed while search is active: `render_grouped_album_rows` reads
        // its catalog from `nav_stack.last().music_grouping.settled`, whose `order`
        // indexes the unfiltered nav-level item vector -- `items` here is the
        // filtered search-result vector, so the catalog's positions would no longer
        // refer to the same albums.
        let show_grouped = self.tab.emby_library_index().is_some_and(|lib_idx| {
            self.is_viewing_album_folders(lib_idx)
                && !library_ctx
                    .as_ref()
                    .is_some_and(LibraryListRenderCtx::is_search_active)
        });

        let n = items.len();

        // Letter grouping: applies to non-music library lists with 50+ items (not during search).
        // Gated on the true library total (`LibraryTab.library_total` when known,
        // e.g. a letter-range pill has scoped the fetch to a smaller slice),
        // not the fetched-so-far/filtered count, so the grouping style (ranges
        // vs. individual letters) doesn't change out from under the user as
        // more pages lazily load in, and a small filtered slice (< 50 items)
        // still shows headers.
        let active_letter_filter = library_ctx
            .as_ref()
            .and_then(|ctx| ctx.letter_filter.clone());
        let ungrouped_total = library_ctx
            .as_ref()
            .map_or(total_count, LibraryListRenderCtx::true_total);
        let use_letter_groups = !show_grouped
            && self.tab.emby_library_index().is_some()
            && (ungrouped_total >= 50 || active_letter_filter.is_some())
            && {
                let lib_idx = self.tab.emby_library_index().unwrap();
                self.libs[lib_idx].library.collection_type != "music"
                    && !library_ctx
                        .as_ref()
                        .is_some_and(LibraryListRenderCtx::is_search_active)
            };

        layout.left_area = list_area;

        if n == 0 {
            layout.hero_area = Rect::default();
            let msg = if self.tab.emby_library_index().is_some() {
                let lib_idx = self.tab.emby_library_index().unwrap();
                if self.is_music_group_view(lib_idx) {
                    // Music-group view messages (moved from the deleted
                    // `render_power_music_group_view`): while the first
                    // grouping snapshot resolves (a candidate exists but no
                    // settled catalog yet), show the organizing message
                    // instead of an empty list; otherwise keep the old view's
                    // loading/empty wording.
                    if self.libs[lib_idx]
                        .nav_stack
                        .last()
                        .and_then(|l| l.music_grouping.as_ref())
                        .is_some_and(|s| s.candidate.is_some() && s.settled.is_none())
                    {
                        " Movin, doin it"
                    } else if self.libs[lib_idx]
                        .nav_stack
                        .last()
                        .map(|l| l.loading)
                        .unwrap_or(false)
                    {
                        " Loading\u{2026}"
                    } else {
                        " (empty)"
                    }
                } else if self.recursive_album_search_enabled(lib_idx)
                    && library_ctx.as_ref().is_some_and(|ctx| ctx.search_loading)
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
            crate::app::render::render_placeholder(f, list_area, msg);
            return;
        }

        layout.hero_area = Rect::default();
        let final_offset: usize;

        if show_grouped && show_music_pills {
            // Narrow music groups route the selected album's detail to the
            // Model A hero (task 3.2) instead of an inline track table. The
            // wide right-rail browser deliberately has no detail block and
            // must not be reused here.
            let lib_idx = self.tab.emby_library_index().unwrap();
            final_offset = self.render_grouped_album_rows(
                f,
                list_area,
                lib_idx,
                &items,
                AlbumRowsCursorCtx {
                    cursor,
                    stored_scroll,
                },
                focused,
                true,
                1,
                layout,
            )
        } else if show_grouped {
            let lib_idx = self.tab.emby_library_index().unwrap();
            final_offset = self.render_grouped_album_rows(
                f,
                list_area,
                lib_idx,
                &items,
                AlbumRowsCursorCtx {
                    cursor,
                    stored_scroll,
                },
                focused,
                true, // hero panel handles the selected album's detail (task 3.2)
                cols as u16,
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
                hero_rows: inline_hero_rows,
            };
            final_offset = super::list_letter_groups::render_letter_grouped_rows(
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
                hero_rows: inline_hero_rows,
            };
            final_offset = super::list_plain::render_plain_rows(f, ctx, layout);
        }

        // Paint the selected replacement after the list has established its
        // flow geometry: shell first, then content inset past its framing.
        if !show_grouped && layout.hero_area.height > 0 {
            selected_detail_shell(f, layout.hero_area, inline_hero_rows, focused);
            // Content, offset 2 rows down past the top border + top
            // padding, and inset 2 cols on each side like music/homevideo's
            // selected blocks; the banner layout is a pure function of the
            // panel width, so this paints the same content as before.
            let content_rect = library::selected_detail_content_area(
                layout.hero_area,
                SELECTED_BLOCK_SIDE_PADDING,
                HERO_BLOCK_EXTRA_ROWS,
            );
            let lib_idx = self.tab.emby_library_index().unwrap();
            // Same movie/Series branch as the row spacing above: a selected
            // Series renders its season pills + episode table instead of the
            // generic compact banner. When neither is selected (e.g. a
            // letter-pill switch has the slice loading), the panel stays as
            // its empty placeholder -- reserved but not painted over.
            if selected_movie_item.is_some() {
                self.render_compact_detail(f, content_rect, lib_idx, focused, true, layout);
            } else if selected_series_item.is_some() {
                self.render_series_inline_detail(f, content_rect, lib_idx, focused, true);
            }
        }

        // Persist the scroll offset so the viewport is remembered across frames.
        // tab is always a Library here (tab == Home uses render_home_list).
        if let Some(lib_idx) = self.tab.emby_library_index() {
            if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
                level.scroll = final_offset;
            }
        }
    }
}

#[cfg(test)]
#[path = "movies_tv_header_fit_tests.rs"]
mod movies_tv_header_fit_tests;
#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
