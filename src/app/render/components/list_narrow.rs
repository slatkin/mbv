//! Canonical narrow TV composition (series list). Generic/Movies/HomeVideos
//! paint through the embedded `BrowserContent` owner instead (task 6.1, design
//! D2). `BrowserComponent` owns inputs; this module paints and publishes
//! replacement-flow geometry for its caller.

use super::detail::compact_banner_image_cache_key;
use crate::app::components::browser_narrow::{NarrowBrowseExtras, NarrowInlineHero};
use crate::app::components::media_list::{
    InlineMediaBrowser, InlineMediaBrowserPaintPolicy, SelectedRowSurface,
};
use crate::app::images::series_image_cache_key;
use crate::app::layout::LayoutMain;
use crate::app::library_column_width::library_column_count;
use crate::app::render::arrangements::{library, wide_hero};
use crate::app::render::components::hero::{
    selected_detail_shell, HERO_BLOCK_EXTRA_ROWS, HERO_PLACEHOLDER_ROWS,
};
use crate::app::render::components::list_rows::{
    LibraryListRenderCtx, SELECTED_BLOCK_SIDE_PADDING,
};
use crate::app::render::HomeImagePaint;
use crate::app::App;
use ratatui::layout::Rect;
use ratatui::Frame;

/// Full narrow TV series-list composition (`migrate-narrow-browse-to-components`
/// task 3.3; Movies/HomeVideos/Generic moved to the embedded `BrowserContent`
/// owner, task 6.1): the letter pill row, the browse row list with an inline
/// series hero reserved in flow, and the empty-state placeholder — the picture
/// the legacy `render_list` narrow branch painted, now owned by
/// `BrowserComponent` via `browser_narrow.rs`. Pure: `layout` is the
/// component's own geometry and the poster image is returned as a
/// `HomeImagePaint` for the shell to execute (no `App`, cache, or fetch).
pub(in crate::app) fn render_narrow_browse_with_ctx(
    f: &mut Frame,
    area: Rect,
    ctx: &LibraryListRenderCtx,
    extras: &NarrowBrowseExtras,
    focused: bool,
    layout: &mut LayoutMain,
    control: &mut InlineMediaBrowser<String>,
) -> (usize, Option<HomeImagePaint>) {
    let content_area = area;

    // Narrow TV season grids keep their own single-column stride
    // (`is_viewing_season_grid`, legacy `list.rs`); every other narrow browse
    // surface derives the column count from the list width.
    let hero_presentation = extras.inline_hero.is_some() || extras.hero_placeholder;
    let cols = if extras.season_grid || hero_presentation {
        1
    } else {
        library_column_count(content_area.width)
    };

    let mut inline_hero_rows: u16 = match &extras.inline_hero {
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
        let areas = wide_hero::pill_bar_areas(content_area);
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

    // The Inline presentation paints every narrow browse surface now that
    // the Grid presentation is deleted as unreachable (design D13): rows,
    // selection, and scroll are authoritative in the shared owner; this
    // function only paints and exports the control's flow geometry.
    let desired_detail_rows = if hero_presentation {
        inline_hero_rows as usize
    } else {
        0
    };
    let browser = control;
    super::media_list::render_inline_media_browser_component(
        f,
        list_area,
        browser,
        InlineMediaBrowserPaintPolicy::new(
            focused,
            SelectedRowSurface::ListBackdrop,
            desired_detail_rows,
        ),
    );
    layout.hero_area = if hero_presentation {
        browser.current_detail_rect().unwrap_or_default()
    } else {
        Rect::default()
    };
    layout.inline_hero_area = layout.hero_area;
    layout.selected_item_rect = browser.current_selected_row_rect();
    let final_offset = browser.current_flow_offset().unwrap_or(0);

    let mut image_paint = None;
    if layout.hero_area.height > 0 {
        selected_detail_shell(f, layout.hero_area, inline_hero_rows, focused);
        let content_rect = library::selected_detail_content_area(
            layout.hero_area,
            SELECTED_BLOCK_SIDE_PADDING,
            HERO_BLOCK_EXTRA_ROWS,
        );
        image_paint = match &extras.inline_hero {
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

/// Letter-range pill row above the narrow TV series list; the selected index
/// comes from the list's `letter_filter`.
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
    /// Poster-prefetch window for the migrated Movies/HomeVideos/Generic
    /// embedded owner (#287): pre-warm the Primary images of movies just
    /// ahead of / behind the cursor. Called from
    /// `shell_browser_content.rs::push_browser_owner_content` after the owner
    /// has established its authoritative cursor. Prefetches only when the
    /// selected row is a non-folder `Movie` leaf, matching the legacy
    /// draw-path gate (the candidate-window filter below screens the window,
    /// not the selection).
    pub(in crate::app) fn fetch_nearby_movie_posters(
        &mut self,
        items: &[mbv_core::api::EmbyItem],
        cursor: usize,
    ) {
        if !items
            .get(cursor)
            .is_some_and(|item| item.item_type == "Movie" && !item.is_folder)
        {
            return;
        }
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

    /// Shell-resolved extras for the narrow TV series-list composer
    /// (`migrate-narrow-browse-to-components` task 3.3; Generic/Movies/
    /// HomeVideos moved to the embedded `BrowserContent` owner, task 6.1):
    /// the letter-pill row and the inline series hero — everything that needs
    /// `App`/image-cache authority, resolved here and pushed to
    /// `BrowserComponent` each frame.
    pub(in crate::app) fn narrow_browse_extras(
        &mut self,
        lib_idx: usize,
        cursor: usize,
    ) -> NarrowBrowseExtras {
        let coll = self.libs[lib_idx].library.collection_type.clone();
        let show_letter_pills = self.should_show_letter_pills(lib_idx);
        let use_shared_replacement_plan = matches!(coll.as_str(), "movies" | "tvshows");
        let season_grid = self.is_viewing_season_grid(lib_idx);

        // This composer now paints only narrow TV (task 6.1 moved
        // Generic/Movies/HomeVideos to the embedded `BrowserContent` owner,
        // which derives its inline hero generically from `HeroContent`), so
        // the only inline hero this function still resolves is a selected
        // Series.
        let selected_series = self.selected_series_item(lib_idx, cursor);

        let inline_hero = if let Some(item) = selected_series {
            let images_enabled = self.images_enabled();
            // Narrow keeps its own `Primary`-chain entry (the chain
            // `detail_series_view` paints); it must never read Wide's
            // Thumb-first entry, whose bytes differ.
            let image_cache_key = series_image_cache_key(&item.id, &["Primary"]);
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

        // Every hero-capable browse destination, including folder/channel
        // selections without a resolved leaf hero, uses the inline
        // replacement flow. Non-hero catalogs keep their width-derived grid.
        let hero_placeholder = inline_hero.is_none()
            && !crate::app::render::arrangements::wide_hero::wide_hero_fits(
                self.layout.main.left_area,
            )
            && matches!(
                coll.as_str(),
                "movies" | "homevideos" | "podcasts" | "tvshows" | "music"
            );

        NarrowBrowseExtras {
            show_letter_pills,
            use_shared_replacement_plan,
            hero_placeholder,
            season_grid,
            inline_hero,
        }
    }
}
