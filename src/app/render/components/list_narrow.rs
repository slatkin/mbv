//! Narrow generic/Movies/home-video browse composition
//! (`migrate-narrow-browse-to-components` task 3.3), split out of `list.rs`
//! for the file-size cap. `render_narrow_browse_with_ctx` is the surface's
//! sole painter now that `BrowserComponent` owns it; the legacy `render_list`
//! narrow branch only publishes geometry (see its guard). `narrow_browse_extras`
//! resolves the `App`/image-cache-backed inputs the shell pushes each frame.

use super::detail::compact_banner_image_cache_key;
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

    if extras.home_video && content_area.height > 0 {
        content_area = crate::app::render::render_count_label(f, content_area, ctx.total_count);
        content_area = Rect {
            y: content_area.y + 1,
            height: content_area.height.saturating_sub(1),
            ..content_area
        };
    }

    let cols = library_column_count(content_area.width);

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

    let (pills_area, list_area) = if extras.show_letter_pills {
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
    if extras.show_letter_pills {
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
            if ctx.loading { "Loading..." } else { "(empty)" },
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
    /// ahead of / behind the cursor. Called from the legacy `render_list`
    /// body (podcasts) and the task-3.3 geometry-only early return
    /// (generic/Movies/home video); task 3.7 relocates it to the shell.
    pub(in crate::app::render) fn fetch_nearby_movie_posters(
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
    pub(in crate::app) fn narrow_browse_extras(&mut self, lib_idx: usize) -> NarrowBrowseExtras {
        let coll = self.libs[lib_idx].library.collection_type.clone();
        let home_video = self.is_home_video_view(lib_idx);
        let show_letter_pills = self.should_show_letter_pills(lib_idx);
        let use_shared_replacement_plan = matches!(coll.as_str(), "movies" | "tvshows");

        let selected_movie = self.selected_movie_item(lib_idx);
        let selected_series = if selected_movie.is_none() {
            self.selected_series_item(lib_idx)
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
            inline_hero,
        }
    }
}
