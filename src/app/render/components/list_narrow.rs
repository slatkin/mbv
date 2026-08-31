//! Narrow generic/Movies/home-video browse composition
//! (`migrate-narrow-browse-to-components` task 3.3), split out of `list.rs`
//! for the file-size cap. `render_narrow_browse_with_ctx` is the surface's
//! sole painter now that `BrowserComponent` owns it; the legacy `render_list`
//! narrow branch only publishes geometry (see its guard). `narrow_browse_extras`
//! resolves the `App`/image-cache-backed inputs the shell pushes each frame.

use super::detail::compact_banner_image_cache_key;
use super::home_video::render_home_video_item;
use crate::app::components::browser_narrow::{NarrowBrowseExtras, NarrowInlineHero};
use crate::app::layout::LayoutMain;
use crate::app::library_column_width::library_column_count;
use crate::app::render::arrangements::{hero_left, library};
use crate::app::render::components::hero::{
    selected_detail_shell, HERO_BLOCK_EXTRA_ROWS, HERO_PLACEHOLDER_ROWS, HERO_TITLE_ROWS,
};
use crate::app::render::components::list_rows::{
    LibraryListRenderCtx, SELECTED_BLOCK_SIDE_PADDING,
};
use crate::app::render::HomeImagePaint;
use crate::app::App;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Full narrow generic/Movies/home-video browse composition
/// (`migrate-narrow-browse-to-components` task 3.3): the count label, letter
/// pill row, the browse row list with an inline movie/series hero reserved in
/// flow, and the empty-state placeholder — the picture the legacy
/// `render_list` narrow branch painted, now owned by `BrowserComponent` via
/// `browser_narrow.rs`. Pure: `layout` is the component's own geometry and
/// the poster image is returned as a `HomeImagePaint` for the shell to
/// execute (no `App`, cache, or fetch).
pub(in crate::app) fn render_narrow_browse_with_ctx(
    f: &mut Frame,
    area: Rect,
    ctx: &LibraryListRenderCtx,
    extras: &NarrowBrowseExtras,
    focused: bool,
    layout: &mut LayoutMain,
) -> (usize, Option<HomeImagePaint>) {
    let mut content_area = area;

    // Feed/home-video group pickers share the browser composer, but their
    // cursor and rows live in FeedHomeVideoState rather than BrowseLevel.
    let feed_ctx = extras
        .feed_items
        .as_ref()
        .map(|items| LibraryListRenderCtx {
            items: items.clone(),
            cursor: ctx.cursor.min(items.len().saturating_sub(1)),
            scroll: ctx.scroll,
            total_count: items.len(),
            library_total: Some(items.len()),
            letter_filter: None,
            loading: ctx.loading,
            search_query: None,
            search_loading: false,
            group_pills: false,
        });
    let ctx = feed_ctx.as_ref().unwrap_or(ctx);

    if extras.home_video && extras.feed_items.is_none() && content_area.height > 0 {
        content_area = crate::app::render::render_count_label(f, content_area, ctx.total_count);
        content_area = Rect {
            y: content_area.y + 1,
            height: content_area.height.saturating_sub(1),
            ..content_area
        };
    }

    // Narrow TV season grids keep their own single-column stride
    // (`is_viewing_season_grid`, legacy `list.rs`); every other narrow browse
    // surface derives the column count from the list width.
    let cols = if extras.season_grid || extras.feed_items.is_some() {
        1
    } else {
        library_column_count(content_area.width)
    };

    let mut inline_hero_rows: u16 = match &extras.inline_hero {
        Some(NarrowInlineHero::Movie { layout: banner, .. }) => {
            banner.content_rows_with_title(HERO_TITLE_ROWS.saturating_mul((cols > 1) as u16)) as u16
                + HERO_BLOCK_EXTRA_ROWS
        }
        Some(NarrowInlineHero::Series {
            item,
            images_enabled,
            ..
        }) => {
            crate::app::render::screens::detail_series::series_inline_detail_rows(
                *images_enabled,
                item,
                content_area.width,
                cols > 1,
            ) as u16
                + HERO_BLOCK_EXTRA_ROWS
        }
        None => {
            if extras.hero_placeholder {
                HERO_PLACEHOLDER_ROWS
            } else {
                0
            }
        }
    };
    if !extras.use_shared_replacement_plan {
        inline_hero_rows =
            if inline_hero_rows > HERO_BLOCK_EXTRA_ROWS && inline_hero_rows < content_area.height {
                inline_hero_rows
            } else {
                0
            };
    }

    // Feed group pickers retain their dedicated legacy geometry: two rows after
    // the pills hold the count/divider, then the selected video expands inline.
    if let Some(items) = extras.feed_items.as_ref() {
        return render_feed_group_picker_content(
            f,
            content_area,
            ctx,
            extras,
            focused,
            layout,
            items,
        );
    }

    let (pills_area, list_area) = if extras.feed_items.is_some() {
        let areas = hero_left::pill_bar_areas(content_area);
        (areas.pills_area, areas.content_area)
    } else if extras.show_letter_pills {
        let areas = hero_left::pill_bar_areas(content_area);
        (areas.pills_area, areas.content_area)
    } else {
        (Rect::default(), content_area)
    };
    if !extras.use_shared_replacement_plan {
        inline_hero_rows =
            if inline_hero_rows > HERO_BLOCK_EXTRA_ROWS && inline_hero_rows < list_area.height {
                inline_hero_rows
            } else {
                0
            };
    }
    if extras.feed_items.is_some() {
        let ids: Vec<usize> = (0..=extras.feed_groups.len()).collect();
        let mut labels = vec!["All".to_string()];
        labels.extend(
            extras
                .feed_groups
                .iter()
                .map(|s| crate::app::ui_util::trunc_str(s, 12).to_string()),
        );
        layout.selector_tabs = crate::app::render::render_pill_bar(
            f,
            pills_area,
            crate::app::render::PillBar {
                labels: &labels,
                ids: &ids,
                selected_pos: extras.feed_group_cursor,
                prefix: Some(" ⌘ "),
            },
        );
    } else if extras.show_letter_pills {
        paint_letter_pills_row(
            f,
            pills_area,
            ctx.letter_filter.as_ref().map(|flt| flt.index).unwrap_or(0),
            layout,
        );
    }

    layout.left_area = list_area;
    layout.hero_area = Rect::default();

    if ctx.items.is_empty() {
        crate::app::render::render_placeholder(
            f,
            list_area,
            if extras.feed_items.is_some() {
                if ctx.loading {
                    " Loading…"
                } else {
                    " (empty)"
                }
            } else if ctx.loading {
                "Loading..."
            } else {
                "(empty)"
            },
        );
        return (0, None);
    }

    let use_letter_groups =
        !ctx.is_search_active() && (ctx.true_total() >= 50 || ctx.letter_filter.is_some());
    let row_ctx = ctx.rows(list_area, cols, focused, inline_hero_rows);
    let final_offset = if use_letter_groups {
        super::list_letter_groups::render_letter_grouped_rows(
            f,
            row_ctx,
            ctx.letter_filter.clone(),
            ctx.true_total(),
            layout,
        )
    } else {
        super::list_plain::render_plain_rows(f, row_ctx, layout)
    };

    let mut image_paint = None;
    if layout.hero_area.height > 0 {
        selected_detail_shell(f, layout.hero_area, inline_hero_rows, focused);
        let content_rect = library::selected_detail_content_area(
            layout.hero_area,
            SELECTED_BLOCK_SIDE_PADDING,
            HERO_BLOCK_EXTRA_ROWS,
        );
        image_paint = match &extras.inline_hero {
            Some(NarrowInlineHero::Movie {
                item,
                layout: banner,
            }) => super::detail::render_compact_detail_with_ctx(
                super::detail::CompactDetailCtx {
                    item,
                    layout: banner.clone(),
                },
                f,
                content_rect,
                focused,
                true,
            ),
            Some(NarrowInlineHero::Series {
                item,
                images_enabled,
                image_loading,
            }) => super::detail_series_view::render_series_inline_detail(
                super::detail_series_view::SeriesInlineDetailCtx {
                    item,
                    images_enabled: *images_enabled,
                    image_loading: *image_loading,
                },
                f,
                content_rect,
                focused,
                true,
            ),
            None => None,
        };
    }

    (final_offset, image_paint)
}

pub(in crate::app) fn render_wide_feed_layer(
    f: &mut Frame,
    area: Rect,
    extras: &NarrowBrowseExtras,
    layout: &mut LayoutMain,
) {
    let pills = crate::app::render::arrangements::hero_left::pill_bar_areas(area);
    let labels: Vec<String> = std::iter::once("All".into())
        .chain(
            extras
                .feed_groups
                .iter()
                .map(|s| crate::app::ui_util::trunc_str(s, 12)),
        )
        .collect();
    let ids: Vec<usize> = (0..labels.len()).collect();
    layout.selector_tabs = crate::app::render::render_pill_bar(
        f,
        pills.pills_area,
        crate::app::render::PillBar {
            labels: &labels,
            ids: &ids,
            selected_pos: extras.feed_group_cursor,
            prefix: Some(" ⌘ "),
        },
    );
    let divider = Rect {
        x: area.x,
        y: pills.content_area.y,
        width: area.width,
        height: 1,
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![Span::raw(
            "▁".repeat(divider.width as usize),
        )])),
        divider,
    );
    layout.left_area = divider;
    if let Some(items) = extras.feed_items.as_ref() {
        let content_area = Rect {
            y: divider.y,
            height: area.bottom().saturating_sub(divider.y),
            ..area
        };
        let text_w = crate::app::render::content_width(content_area.width, false);
        let selected = extras.feed_video_cursor.min(items.len().saturating_sub(1));
        let mut y = content_area.y;
        for (idx, item) in items.iter().enumerate() {
            if y >= content_area.bottom() {
                break;
            }
            let selected_row = idx == selected;
            let row_height = if selected_row { 5 } else { 1 };
            render_home_video_item(
                f,
                item,
                y,
                row_height,
                content_area,
                text_w,
                selected_row,
                true,
            );
            if selected_row {
                layout.selected_item_rect = Some(Rect {
                    x: content_area.x,
                    y,
                    width: text_w as u16,
                    height: row_height,
                });
            }
            y = y.saturating_add(if selected_row { row_height } else { 2 });
        }
    }
}

fn render_feed_group_picker_content(
    f: &mut Frame,
    area: Rect,
    ctx: &LibraryListRenderCtx,
    extras: &NarrowBrowseExtras,
    focused: bool,
    layout: &mut LayoutMain,
    items: &[mbv_core::api::EmbyItem],
) -> (usize, Option<HomeImagePaint>) {
    let pills = crate::app::render::arrangements::hero_left::pill_bar_areas(area);
    let labels: Vec<String> = std::iter::once("All".into())
        .chain(
            extras
                .feed_groups
                .iter()
                .map(|s| crate::app::ui_util::trunc_str(s, 12)),
        )
        .collect();
    let ids: Vec<usize> = (0..labels.len()).collect();
    layout.selector_tabs = crate::app::render::render_pill_bar(
        f,
        pills.pills_area,
        crate::app::render::PillBar {
            labels: &labels,
            ids: &ids,
            selected_pos: extras.feed_group_cursor,
            prefix: Some(" ⌘ "),
        },
    );
    let mut list_area = pills.content_area;
    if list_area.height > 0 {
        let count_area = Rect {
            height: 1,
            ..list_area
        };
        let label = format!(" {} items", items.len());
        let divider = "▁".repeat(
            count_area
                .width
                .saturating_sub(label.chars().count() as u16) as usize,
        );
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    label,
                    Style::default().fg(crate::app::palette::TEXT_SECONDARY),
                ),
                Span::raw(divider),
            ])),
            count_area,
        );
        list_area.y += 2;
        list_area.height = list_area.height.saturating_sub(2);
    }
    layout.left_area = list_area;
    if items.is_empty() {
        return (0, None);
    }
    let selected = extras
        .inline_hero
        .as_ref()
        .and_then(|hero| match hero {
            NarrowInlineHero::Movie { item, .. } | NarrowInlineHero::Series { item, .. } => {
                items.iter().position(|candidate| candidate.id == item.id)
            }
        })
        .unwrap_or(extras.feed_video_cursor.min(items.len() - 1));
    let text_w =
        crate::app::render::content_width(list_area.width, items.len() > list_area.height as usize);
    let panel_w = text_w.saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING as usize) as u16;
    let selected_h = 1;
    let mut row = list_area.y;
    let mut offset = ctx.scroll.min(selected);
    if selected < offset {
        offset = selected;
    }
    if selected_h > list_area.height {
        offset = selected;
    }
    let mut image = None;
    for (idx, item) in items.iter().enumerate().skip(offset) {
        if row >= list_area.bottom() {
            break;
        }
        let h = if idx == selected { selected_h } else { 1 };
        render_home_video_item(
            f,
            item,
            row,
            h,
            if idx == selected {
                Rect {
                    x: list_area.x.saturating_sub(1),
                    width: list_area.width.saturating_add(1),
                    ..list_area
                }
            } else if idx == selected.saturating_add(1) {
                Rect {
                    x: list_area.x + 1,
                    width: list_area.width.saturating_sub(1),
                    ..list_area
                }
            } else {
                list_area
            },
            text_w,
            idx == selected,
            focused,
        );
        if idx == selected {
            layout.selected_item_rect = Some(Rect {
                x: list_area.x,
                y: row,
                width: text_w as u16,
                height: h,
            });
            if let Some(NarrowInlineHero::Movie {
                item,
                layout: banner,
            }) = extras.inline_hero.as_ref()
            {
                image = super::detail::render_compact_detail_with_ctx(
                    super::detail::CompactDetailCtx {
                        item,
                        layout: banner.clone(),
                    },
                    f,
                    Rect {
                        x: list_area.x + SELECTED_BLOCK_SIDE_PADDING,
                        y: row + 3,
                        width: panel_w,
                        height: h.saturating_sub(5),
                    },
                    focused,
                    false,
                );
            }
        }
        row = row.saturating_add(h);
    }
    if let Some(item) = items.last() {
        let y = row;
        f.render_widget(
            Paragraph::new("▔".repeat(text_w)),
            Rect {
                x: list_area.x,
                y,
                width: text_w as u16,
                height: 1,
            },
        );
        f.render_widget(
            Paragraph::new(item.display_name()),
            Rect {
                x: list_area.x,
                y: y + 1,
                width: text_w as u16 - 1,
                height: 1,
            },
        );
    }
    (offset, image)
}

fn paint_letter_pills_row(
    f: &mut Frame,
    row_area: Rect,
    selected_pos: usize,
    layout: &mut LayoutMain,
) {
    if row_area.width == 0 {
        layout.selector_tabs = Vec::new();
        return;
    }
    let labels = crate::app::render::LetterFilter::labels();
    let ids: Vec<usize> = (0..labels.len()).collect();
    layout.selector_tabs = crate::app::render::render_pill_bar(
        f,
        row_area,
        crate::app::render::PillBar {
            labels: &labels,
            ids: &ids,
            selected_pos,
            prefix: Some(" \u{2318} "),
        },
    );
}

impl App {
    /// Poster-prefetch window for the narrow generic/Movies/home-video and
    /// podcast browsers (#287): pre-warm the Primary images of movies just
    /// ahead of / behind the cursor. Called from `shell_browser.rs` after the
    /// mounted browser has established its authoritative cursor.
    pub(in crate::app) fn fetch_nearby_movie_posters(
        &mut self,
        items: &[mbv_core::api::EmbyItem],
        cursor: usize,
    ) {
        const PREFETCH_AHEAD: usize = 3;
        const PREFETCH_BEHIND: usize = 1;
        let start = cursor.saturating_sub(PREFETCH_BEHIND);
        let end = (cursor + PREFETCH_AHEAD + 1).min(items.len());
        let prefetch: Vec<(String, String, String)> = items[start..end]
            .iter()
            .enumerate()
            .filter(|(i, item)| start + i != cursor && item.item_type == "Movie" && !item.is_folder)
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
                self.fetch_list_card_image_when_idle(cache_key, item_id, series_id, &["Primary"]);
            }
        }
    }

    /// Shell-resolved extras for the narrow generic/Movies/home-video browse
    /// composer (`migrate-narrow-browse-to-components` task 3.3): the count
    /// label, letter-pill row, and the inline movie/series hero — everything
    /// that needs `App`/image-cache authority, resolved here and pushed to
    /// `BrowserComponent` each frame.
    pub(in crate::app) fn narrow_browse_extras(
        &mut self,
        lib_idx: usize,
        cursor: usize,
    ) -> NarrowBrowseExtras {
        let coll = self.libs[lib_idx].library.collection_type.clone();
        let feed_group_view = self.is_feed_home_video_group_view(lib_idx);
        let home_video = self.is_home_video_view(lib_idx) && !feed_group_view;
        let show_letter_pills = self.should_show_letter_pills(lib_idx);
        let (feed_items, feed_groups, feed_group_cursor, feed_video_cursor) = if feed_group_view {
            let items = self.feed_home_video_selected_items(lib_idx);
            let groups = self.libs[lib_idx]
                .feed_home_video
                .as_ref()
                .map(|s| s.groups.iter().map(|g| g.folder.name.clone()).collect())
                .unwrap_or_default();
            let cursor = self.feed_home_video_selected_group_index(lib_idx);
            let video_cursor = self.libs[lib_idx]
                .feed_home_video
                .as_ref()
                .map_or(0, |s| s.video_cursor);
            (Some(items), groups, cursor, video_cursor)
        } else {
            (None, Vec::new(), 0, 0)
        };
        let use_shared_replacement_plan = matches!(coll.as_str(), "movies" | "tvshows");
        let season_grid = self.is_viewing_season_grid(lib_idx);

        let selected_movie = self.selected_movie_item(lib_idx, cursor).or_else(|| {
            feed_items.as_ref().and_then(|items| {
                let cursor = self.libs[lib_idx]
                    .feed_home_video
                    .as_ref()
                    .map_or(0, |s| s.video_cursor);
                items.get(cursor).cloned()
            })
        });
        let selected_series = if selected_movie.is_none() {
            self.selected_series_item(lib_idx, cursor)
        } else {
            None
        };

        let inline_hero = if let Some(item) = selected_movie {
            let truncate_overview =
                self.is_home_video_view(lib_idx) || self.is_podcast_library(lib_idx);
            let panel_width = self
                .layout
                .main
                .left_area
                .width
                .saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING);
            let banner =
                self.compact_banner_layout_with_overview(&item, panel_width, truncate_overview);
            Some(NarrowInlineHero::Movie {
                item,
                layout: banner,
            })
        } else if let Some(item) = selected_series {
            let images_enabled = self.images_enabled();
            let image_cache_key = format!("{}:ser_primary", item.id);
            let image_loading =
                images_enabled && !self.card_image_states.contains_key(&image_cache_key);
            Some(NarrowInlineHero::Series {
                item,
                images_enabled,
                image_loading,
            })
        } else {
            None
        };

        let hero_placeholder = inline_hero.is_none()
            && crate::app::render::arrangements::hero_left::shared_hero_presentation(
                self.layout.main.left_area,
            )
            .is_none()
            && self.libs[lib_idx].nav_stack.len() == 1
            && matches!(
                coll.as_str(),
                "movies" | "homevideos" | "podcasts" | "tvshows" | "music"
            );

        NarrowBrowseExtras {
            home_video,
            show_letter_pills,
            use_shared_replacement_plan,
            hero_placeholder,
            season_grid,
            feed_items,
            feed_groups,
            feed_group_cursor,
            feed_video_cursor,
            inline_hero,
        }
    }
}
