//! Narrow browse inputs for the canonical inline media-browser composition.
//!
//! `BrowserComponent` owns the narrow generic/Movies/home-video surface's
//! paint. The shell resolves everything that needs `App`/image-cache
//! authority into this plain [`NarrowBrowseExtras`] bundle (count label,
//! letter-pill row, inline movie/series hero sizing) and pushes it to the
//! component each frame; the component's `view` then hands it, plus the
//! mirrored [`LibraryListRenderCtx`], to the pure composition helper
//! [`crate::app::render::render_narrow_browse_with_ctx`] and forwards the
//! returned [`HomeImagePaint`] to the shell (mirrors `HomeComponent`).

use crate::app::render::CompactBannerLayout;

/// The selected item's inline hero, already resolved by the shell.
pub(in crate::app) enum NarrowInlineHero {
    /// A leaf movie/home-video/podcast item: its compact banner layout was
    /// computed shell-side (the only image-cache-touching step).
    Movie {
        item: mbv_core::api::EmbyItem,
        layout: CompactBannerLayout,
    },
    /// A selected Series on a `tvshows` library.
    Series {
        item: mbv_core::api::EmbyItem,
        images_enabled: bool,
        image_loading: bool,
    },
}

/// Shell-resolved extras the narrow composer needs beyond the mirrored
/// [`LibraryListRenderCtx`]. Built by `App::narrow_browse_extras`.
#[derive(Default)]
pub(in crate::app) struct NarrowBrowseExtras {
    /// Render the " N items" count label above the list (home-video tab).
    pub(in crate::app) home_video: bool,
    /// Render the letter-range pill row above the list.
    pub(in crate::app) show_letter_pills: bool,
    /// `collection_type` is `movies`/`tvshows`: the inline-hero rows are
    /// always reserved rather than dropped when they don't fit the flow.
    pub(in crate::app) use_shared_replacement_plan: bool,
    /// No hero item is selected but the surface is a hero-capable collection
    /// at its top browse level: keep the fixed placeholder panel reserved.
    pub(in crate::app) hero_placeholder: bool,
    /// Narrow TV season grid (`is_viewing_season_grid`): force a single-column
    /// stride instead of the width-derived column count (legacy `list.rs`).
    pub(in crate::app) season_grid: bool,
    /// Feed/home-video group-picker content projected by the shell.
    pub(in crate::app) feed_items: Option<Vec<mbv_core::api::EmbyItem>>,
    pub(in crate::app) feed_groups: Vec<String>,
    pub(in crate::app) feed_group_cursor: usize,
    /// Feed picker selected-row replacement height: `banner.content_rows() + 5`.
    /// Computed shell-side because banner layout may access the image cache;
    /// components must not perform effects while painting.
    pub(in crate::app) feed_selected_height: u16,
    pub(in crate::app) inline_hero: Option<NarrowInlineHero>,
}
