use super::album::AlbumRowsCursorCtx;
use super::album_art::INLINE_ALBUM_ART_RESERVED;
use super::detail::compact_banner_image_cache_key;
use super::hero::{
    hero_block_shell, top_hero_layout, HERO_BLOCK_EXTRA_ROWS, HERO_PLACEHOLDER_ROWS,
    HERO_TITLE_ROWS,
};
use super::list_rows::{ListRenderCtx, SELECTED_BLOCK_SIDE_PADDING};
use crate::app::layout::LayoutMain;
use crate::app::{palette, App, TWO_COLUMN_THRESHOLD};
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;

impl App {
    /// Renders the Continue/library list items into `area`.
    /// The title header is now drawn in the top-of-screen FOAM bar.
    pub(super) fn render_list(
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
                && area.width >= TWO_COLUMN_THRESHOLD
            {
                self.render_wide_music_group(f, area, lib_idx, focused, layout);
                return;
            }
        }

        let mut content_area = area;

        // Search is active for the focused Emby library. Its 3-row input box
        // is placed *below* the hero in hero-on-top view (see the block after
        // `top_hero_layout`), so here we only record that it's on.
        let search_active = focused
            && self.tab.emby_library_index().is_some()
            && self.libs[self.tab.emby_library_index().unwrap()]
                .search
                .is_some();

        // Home videos' declared element-presence difference (design.md
        // decision 6): a count label row instead of the letter-pill row
        // every other hero-on-top screen may show (`should_show_letter_pills`
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
            content_area = super::render_count_label(f, content_area, total);
            // Leave the blank gap row `top_hero_layout`'s `hero_shift`
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
        let selected_album_item = if selected_series_item.is_none() {
            self.tab
                .emby_library_index()
                .and_then(|lib_idx| self.selected_album_hero_item(lib_idx))
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

        // ── Fixed hero area pinned to the top of content_area, letter pills
        //    below it ──────────────────────────────────────────────────────
        // The selected item's banner (poster + meta + overview, or -- for a
        // selected Series -- the season pills + episode table) is painted
        // into a fixed-height rect at the top of the content area, followed
        // by a blank separator row. Below that, the letter-range pill row
        // (large non-music libraries' top browse level) gets its own row
        // plus a blank gap -- the same reservation `mod.rs` used to carve
        // out of `lib_area` before calling into this renderer; it lives
        // here now so the pills land below the hero, not above it. The
        // list below everything (`list_area`) is a plain grid that never
        // reflows as the cursor moves. No hero when the level has no
        // hero-capable content (folders, music, non-hero collections) --
        // the list then takes the whole remaining area.
        //
        let hero_rows: u16 = if self.tab.emby_library_index().is_some() {
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
                    .content_rows() as u16;
                content_rows
                    + HERO_TITLE_ROWS.saturating_mul((cols > 1) as u16)
                    + HERO_BLOCK_EXTRA_ROWS
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
            } else if let Some(item) = &selected_album_item {
                // Album hero: art + optional tracks + metadata + block chrome.
                // Only include tracks when track-selection mode is active.
                let in_track_mode = self.libs[lib_idx].album_track_focus.is_some();
                let track_count = if in_track_mode {
                    self.album_tracks_cache
                        .get(&item.id)
                        .map(|t| t.len())
                        .unwrap_or(0)
                } else {
                    0
                };
                let art_rows = if self.images_enabled() {
                    super::album_art::INLINE_ALBUM_ART_ROWS
                } else {
                    0
                };
                let panel_width = content_area
                    .width
                    .saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING);
                super::album_plan::album_hero_content_rows(
                    track_count,
                    art_rows,
                    panel_width,
                    self.images_enabled(),
                ) + HERO_BLOCK_EXTRA_ROWS
            } else {
                // No banner content to size to. If we're at the top browse
                // level of a hero-capable collection (movies/homevideos/
                // podcasts/tvshows/music), keep the fixed placeholder panel
                // reserved instead of collapsing to zero -- a letter-pill
                // switch clears the slice before its replacement loads, and
                // this keeps the slot from jumping away and back. The
                // placeholder size is just the stand-in; once content lands
                // the block sizes to it.
                let top_hero_level = self.libs[lib_idx].nav_stack.len() == 1
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
                if top_hero_level || music_hero_placeholder {
                    HERO_PLACEHOLDER_ROWS
                } else {
                    0
                }
            }
        } else {
            0
        };

        // Pill row below the hero: letter-range pills for large non-music
        // libraries, or the music-group selector while browsing a group's
        // albums (narrow fallback -- wide grouped Music places its pills in
        // the right rail instead, `render_wide_music_group`). Both share one
        // slot since a library is never both at once. Reserves 1 row for the
        // pills plus 1 blank gap row below them.
        let show_letter_pills = self
            .tab
            .emby_library_index()
            .is_some_and(|lib_idx| self.should_show_letter_pills(lib_idx));
        let show_music_pills = self
            .tab
            .emby_library_index()
            .is_some_and(|lib_idx| self.is_music_group_view(lib_idx));
        // The pill row and the search box occupy the same slot directly below
        // the hero; the search box just takes precedence while filtering.
        let show_pills = show_letter_pills || show_music_pills;
        let hero_layout = top_hero_layout(content_area, hero_rows, show_pills);
        let hero_area = hero_layout.hero_area;
        let pills_area = hero_layout.pills_area;
        let mut list_area = hero_layout.list_area;
        let hero_rows = hero_layout.hero_rows;

        if show_letter_pills && !search_active {
            let lib_idx = self.tab.emby_library_index().unwrap();
            self.render_letter_pills_row(f, pills_area, lib_idx, layout);
        } else if show_music_pills && !search_active {
            let lib_idx = self.tab.emby_library_index().unwrap();
            self.render_music_group_pills_row(f, pills_area, lib_idx, layout);
        }

        // The search input box replaces the pill row, occupying the exact
        // slot the pill bar would sit in: flush below the hero with no blank
        // gap. It is one row taller than the pill+gap reservation, so the
        // list recomputes from the search box's bottom to the content bottom.
        if search_active && hero_area.y + hero_rows <= content_area.y + content_area.height {
            let lib_idx = self.tab.emby_library_index().unwrap();
            let search_top = hero_area.y + hero_rows;
            let search_area = Rect {
                x: content_area.x,
                y: search_top,
                width: content_area.width,
                height: 3,
            };
            let s = self.libs[lib_idx].search.as_ref().unwrap();
            super::hero::render_search_box(f, search_area, &s.query, s.loading);
            list_area = Rect {
                x: content_area.x,
                y: search_top + 3,
                width: content_area.width,
                height: content_area
                    .y
                    .saturating_add(content_area.height)
                    .saturating_sub(search_top.saturating_add(3)),
            };
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
            let lib_idx = self.tab.emby_library_index().unwrap();
            let lib = &self.libs[lib_idx];
            let (items, cur, scroll, total) = if let Some(s) = &lib.search {
                let items: Vec<mbv_core::api::EmbyItem> = s
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
            self.is_viewing_album_folders(lib_idx) && self.libs[lib_idx].search.is_none()
        });

        let n = items.len();

        // Letter grouping: applies to non-music library lists with 50+ items (not during search).
        // Gated on the true library total (`LibraryTab.library_total` when known,
        // e.g. a letter-range pill has scoped the fetch to a smaller slice),
        // not the fetched-so-far/filtered count, so the grouping style (ranges
        // vs. individual letters) doesn't change out from under the user as
        // more pages lazily load in, and a small filtered slice (< 50 items)
        // still shows headers.
        let active_letter_filter = self.tab.emby_library_index().and_then(|lib_idx| {
            self.libs[lib_idx]
                .nav_stack
                .last()
                .and_then(|l| l.letter_filter.as_ref())
                .cloned()
        });
        let ungrouped_total = self
            .tab
            .emby_library_index()
            .map_or(total_count, |lib_idx| {
                self.libs[lib_idx].library_total.unwrap_or(total_count)
            });
        let use_letter_groups = !show_grouped
            && self.tab.emby_library_index().is_some()
            && (ungrouped_total >= 50 || active_letter_filter.is_some())
            && {
                let lib_idx = self.tab.emby_library_index().unwrap();
                self.libs[lib_idx].library.collection_type != "music"
                    && self.libs[lib_idx].search.is_none()
            };

        layout.left_area = list_area;

        if n == 0 {
            layout.hero_area = hero_area;
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
            // Reserve and paint the hero's placeholder panel so the block
            // stays put even while a letter-pill switch has the slice
            // cleared and loading (see `top_hero_level` above), then show
            // the status message in the list area below it.
            if hero_rows > 0 {
                hero_block_shell(f, hero_area, hero_rows, focused);
            }
            super::render_placeholder(f, list_area, msg);
            return;
        }

        layout.hero_area = hero_area;

        let final_offset: usize;

        if show_grouped && show_music_pills {
            // Music-group view: route through the exact same one-column
            // grouped-album browser the wide hero-on-left layout uses for
            // its right pane (`render_wide_right_album_browser`), rather
            // than the hero-on-top-only packed/wrapped row renderer below
            // -- narrow's own scroll/selection handling had drifted from
            // wide's since the two were unified.
            let lib_idx = self.tab.emby_library_index().unwrap();
            self.render_wide_right_album_browser(f, list_area, list_area, lib_idx, focused, layout);
            final_offset = self.libs[lib_idx]
                .nav_stack
                .last()
                .map(|lvl| lvl.scroll)
                .unwrap_or(0);
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
                true, // hero_handles_detail: the hero panel renders the detail
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
            };
            final_offset = self.render_letter_grouped_rows(
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
            final_offset = self.render_plain_rows(f, ctx, layout);
        }

        // Paint the hero into its fixed top-edge rect, after the list has
        // rendered: the outer shell (colored bg + `▁`/`▔` borders), then the
        // content offset 2 rows down past the top border + top padding. The
        // row renderer set `cursor_screen_y` to the selected list row; the
        // hero paint doesn't touch it.
        if hero_rows > 0 {
            hero_block_shell(f, hero_area, hero_rows, focused);
            // Content, offset 2 rows down past the top border + top
            // padding, and inset 2 cols on each side like music/homevideo's
            // selected blocks; the banner layout is a pure function of the
            // panel width, so this paints the same content as before.
            let content_rect = Rect {
                x: hero_area.x + SELECTED_BLOCK_SIDE_PADDING,
                y: hero_area.y + 2,
                width: hero_area
                    .width
                    .saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING),
                height: hero_rows - HERO_BLOCK_EXTRA_ROWS,
            };
            let lib_idx = self.tab.emby_library_index().unwrap();
            // Same movie/Series branch as the row spacing above: a selected
            // Series renders its season pills + episode table instead of the
            // generic compact banner. When neither is selected (e.g. a
            // letter-pill switch has the slice loading), the panel stays as
            // its empty placeholder -- reserved but not painted over.
            if selected_movie_item.is_some() {
                self.render_compact_detail(f, content_rect, lib_idx, focused, cols > 1, layout);
            } else if selected_series_item.is_some() {
                self.render_series_inline_detail(
                    f,
                    content_rect,
                    lib_idx,
                    focused,
                    cols > 1,
                    layout,
                );
            } else if let Some(item) = &selected_album_item {
                // Album hero: art + optional tracks + metadata, mirroring the
                // movie/series branch -- reserved and painted here instead of
                // in `render_power_grouped_album_rows`' inline block. Art
                // sits right-aligned in `content_rect`; the detail reserves
                // its width so the track table never overlaps it.
                // Tracks are only shown when track-selection mode is active.
                let art_reserved_w = if self.images_enabled()
                    && content_rect.width >= INLINE_ALBUM_ART_RESERVED + 20
                {
                    INLINE_ALBUM_ART_RESERVED
                } else {
                    0
                };
                let in_track_mode = self.libs[lib_idx].album_track_focus.is_some();
                // The hero title belongs to the selected album, not to the
                // fetched track records (which may not carry an Album field).
                // Render it before either mode's content so it remains visible
                // when track-selection mode is entered.
                let inner_w = content_rect.width.saturating_sub(art_reserved_w);
                if inner_w >= 3 {
                    let title = item.display_name();
                    let title_trunc = super::super::ui_util::trunc_str(
                        &title,
                        (inner_w as usize).saturating_sub(1),
                    );
                    if content_rect.height > 0 {
                        f.render_widget(
                            Paragraph::new(Line::from(Span::styled(
                                format!(" {title_trunc}"),
                                Style::default()
                                    .fg(palette::YELLOW)
                                    .add_modifier(Modifier::BOLD),
                            ))),
                            Rect {
                                x: content_rect.x,
                                y: content_rect.y,
                                width: inner_w,
                                height: 1,
                            },
                        );
                    }
                    let detail_area = Rect {
                        y: content_rect.y.saturating_add(1),
                        height: content_rect.height.saturating_sub(1),
                        ..content_rect
                    };
                    if in_track_mode {
                        let track_cursor = self.libs[lib_idx].album_track_focus.unwrap_or(0);
                        if let Some(tracks) = self.album_tracks_cache.get(&item.id).cloned() {
                            self.render_album_detail(
                                f,
                                detail_area,
                                &tracks,
                                track_cursor,
                                true,  // focused: track-selection mode is active
                                false, // show_title: rendered above from the album item
                                true,  // selected_region_gutter: hero block context
                                false, // flush_left
                                true,  // show_hint: show the action hint
                                art_reserved_w,
                                layout,
                            );
                        } else {
                            // Tracks not fetched yet: kick off the fetch and show a
                            // loading stand-in (art still reserved on the right).
                            self.fetch_album_tracks(item.id.clone());
                            let loading_rect = Rect {
                                width: detail_area.width.saturating_sub(art_reserved_w),
                                ..detail_area
                            };
                            super::render_placeholder(f, loading_rect, " Loading…");
                        }
                    } else {
                        // Not in track-selection mode: show the action hint only;
                        // the track list is entered with ENTER.
                        let row = detail_area.y;
                        // Hint row
                        if detail_area.height > 0 {
                            let hint_w = (inner_w as usize).saturating_sub(2).max(1);
                            let hint = super::super::ui_util::trunc_str(
                                "^P: Play | ^A: Enqueue | ^S: Shuffle | ENTER: Show tracks",
                                hint_w,
                            );
                            f.render_widget(
                                Paragraph::new(Line::from(vec![
                                    super::selection_marker(false, super::MarkerEdge::Left),
                                    Span::raw(" "),
                                    Span::styled(
                                        hint.to_string(),
                                        Style::default().fg(palette::MUTED),
                                    ),
                                ])),
                                Rect {
                                    x: content_rect.x,
                                    y: row,
                                    width: inner_w,
                                    height: 1,
                                },
                            );
                        }
                    }
                }
                if art_reserved_w > 0 {
                    let art_rect = Rect {
                        x: content_rect.x + content_rect.width.saturating_sub(art_reserved_w),
                        y: content_rect.y,
                        width: art_reserved_w,
                        height: content_rect.height,
                    };
                    self.render_inline_album_art(f, art_rect, item, layout);
                }
            }
        }

        // Persist the scroll offset so the viewport is remembered across frames.
        // tab is always a Library here (tab == Home uses render_home_list).
        if let Some(lib_idx) = self.tab.emby_library_index() {
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
