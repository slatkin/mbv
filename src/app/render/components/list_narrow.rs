//! Transitional narrow-browser composition retained for the still-unregistered
//! browser component. TV uses the Library panel's shared Narrow skeleton;
//! migrated BrowserContent owners paint through that panel as well.

use super::detail::compact_banner_image_cache_key;
use crate::app::components::browser_narrow::NarrowBrowseExtras;
use crate::app::components::media_list::{
    InlineMediaBrowser, InlineMediaBrowserPaintPolicy, SelectedRowSurface,
};
use crate::app::layout::LayoutMain;
use crate::app::render::arrangements::wide_hero;
use crate::app::render::components::hero::{
    selected_detail_shell, HERO_BLOCK_EXTRA_ROWS, HERO_PLACEHOLDER_ROWS,
};
use crate::app::render::components::list_rows::LibraryListRenderCtx;
use crate::app::render::HomeImagePaint;
use crate::app::App;
use ratatui::layout::Rect;
use ratatui::Frame;

/// Mouse-hit and context-menu-anchor geometry `render_narrow_browse_with_ctx`
/// publishes for the caller to adopt onto its own retained fields — the
/// composer's row/hero geometry never round-trips through the shell's
/// `LayoutMain` (task 12.3).
#[derive(Default)]
pub(in crate::app) struct NarrowBrowseGeometry {
    /// Selected-parent geometry for the inline replacement (mouse-hit).
    pub inline_hero_area: Rect,
    /// Screen rect of the selected row/cell, consumed by the context menu's
    /// keyboard anchor.
    pub selected_item_rect: Option<Rect>,
    /// Letter-pill hitboxes painted above the list, if any.
    pub selector_tabs: Vec<(Rect, usize)>,
}

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
) -> (usize, Option<HomeImagePaint>, NarrowBrowseGeometry) {
    let content_area = area;

    // Narrow TV season grids keep their own single-column stride
    // (`is_viewing_season_grid`, legacy `list.rs`); every other transitional
    // narrow browse surface derives the column count from the list width.
    let hero_presentation = extras.hero_placeholder;

    let mut inline_hero_rows: u16 = if extras.hero_placeholder {
        HERO_PLACEHOLDER_ROWS
    } else {
        0
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
    let selector_tabs = if extras.show_letter_pills {
        paint_letter_pills_row(
            f,
            pills_area,
            ctx.letter_filter.as_ref().map(|flt| flt.index).unwrap_or(0),
        )
    } else {
        Vec::new()
    };

    layout.left_area = list_area;

    if ctx.items.is_empty() {
        crate::app::render::render_placeholder(
            f,
            list_area,
            if ctx.loading { "Loading..." } else { "(empty)" },
        );
        return (
            0,
            None,
            NarrowBrowseGeometry {
                selector_tabs,
                ..Default::default()
            },
        );
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
    let hero_area = if hero_presentation {
        browser.current_detail_rect().unwrap_or_default()
    } else {
        Rect::default()
    };
    let selected_item_rect = browser.current_selected_row_rect();
    let final_offset = browser.current_flow_offset().unwrap_or(0);

    if hero_area.height > 0 {
        selected_detail_shell(f, hero_area, inline_hero_rows, focused);
    }

    // The transitional Browser path has no destination-owned hero painter;
    // migrated destinations use the Library panel's shared skeleton.
    (
        final_offset,
        None,
        NarrowBrowseGeometry {
            inline_hero_area: hero_area,
            selected_item_rect,
            selector_tabs,
        },
    )
}

/// Letter-range pill row above the narrow TV series list; the selected index
/// comes from the list's `letter_filter`.
fn paint_letter_pills_row(
    f: &mut Frame,
    row_area: Rect,
    selected_pos: usize,
) -> Vec<(Rect, usize)> {
    if row_area.width == 0 {
        return Vec::new();
    }
    let labels = crate::app::render::LetterFilter::labels();
    let ids: Vec<usize> = (0..labels.len()).collect();
    crate::app::render::render_pill_bar(
        f,
        row_area,
        crate::app::render::PillBar {
            labels: &labels,
            ids: &ids,
            selected_pos,
            prefix: Some(" \u{2318} "),
        },
    )
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

    /// Shell-resolved extras for the transitional narrow browser composer:
    /// the letter-pill row and placeholder replacement policy.
    pub(in crate::app) fn narrow_browse_extras(
        &mut self,
        lib_idx: usize,
        _cursor: usize,
    ) -> NarrowBrowseExtras {
        let coll = self.libs[lib_idx].library.collection_type.clone();
        let show_letter_pills = self.should_show_letter_pills(lib_idx);
        let use_shared_replacement_plan = matches!(coll.as_str(), "movies" | "tvshows");
        let season_grid = self.is_viewing_season_grid(lib_idx);

        // TV now supplies its own shared panel content. This transitional
        // helper only computes whether a hero-capable surface needs the
        // selected-row replacement placeholder.
        // Every hero-capable browse destination, including folder/channel
        // selections without a resolved leaf hero, uses the inline
        // replacement flow. Non-hero catalogs keep their width-derived grid.
        let hero_placeholder = !crate::app::render::arrangements::wide_hero::wide_hero_fits(
            self.layout.main.left_area,
        ) && matches!(
            coll.as_str(),
            "movies" | "homevideos" | "podcasts" | "tvshows" | "music"
        );

        NarrowBrowseExtras {
            show_letter_pills,
            use_shared_replacement_plan,
            hero_placeholder,
            season_grid,
        }
    }
}
