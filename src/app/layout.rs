//! Per-frame layout geometry produced by `render()` and consumed by mouse
//! hit-testing in `input.rs`.
//!
//! `App` owns a single `AppLayout` value (`app.layout`) instead of ~35
//! scattered `layout_*`/`power_*`/`playlist_*` fields. Grouping by view
//! mirrors the boundaries `render/` and `input.rs` already use, rather than
//! inventing a new one.
//!
//! Render code does not write into `self.layout` in place. Each call to
//! `App::render` builds a fresh, local `AppLayout::default()` and threads it
//! (or the relevant per-view sub-struct) through the render call graph as an
//! explicit parameter; every render function that used to write
//! `self.layout.<view>.<field> = ...` now writes `layout.<field> = ...` on
//! that local value instead. Only once the full pass completes does `render`
//! swap it into `self.layout` in a single atomic assignment. This means
//! `self.layout` (read by `input.rs`) always reflects the last frame that
//! rendered in full, or is left completely untouched by an early return
//! (e.g. the zero-area guard) -- it can never hold a mix of fields from two
//! different frames.

use ratatui::layout::Rect;

/// Geometry of one power-home section card in the two-column grid, computed at render
/// time and reused by keyboard navigation (column jumps).
#[derive(Clone, Default)]
pub(crate) struct PowerHomeSectionMeta {
    pub flat_start: usize, // first flat item index in this section
    pub len: usize,        // number of items (0 for the empty Keep Watching card)
    pub row: usize,        // grid row
    pub col: usize,        // grid column
}

/// Seekbar rect and the two divider status indicators that still have a
/// click target (remote-session and mute). The button/track/volume/
/// subtitle/audio rects this used to hold were removed with the expanded
/// playback view; see the "Tab bar restyle" commit that zeroed them out.
#[derive(Default)]
pub(crate) struct LayoutPlayback {
    pub seekbar_area: Rect,
    pub ind_rc: Rect,
    pub ind_mu: Rect,
}

/// Home-tab section/carousel geometry.
#[derive(Default)]
pub(crate) struct LayoutHome {
    pub home_rect: Rect,
    pub section_areas: Vec<Rect>,
    pub home_scrolls: Vec<usize>,
    pub home_scrollbar: Rect,
    pub home_card_strips: Vec<(usize, Rect)>,
    pub carousel_slots: [(Option<usize>, Rect); 3],
    pub carousel_left_arrow: Option<Rect>,
    pub carousel_right_arrow: Option<Rect>,
    pub carousel_up_arrow: Option<Rect>,
    pub carousel_down_arrow: Option<Rect>,
}

/// Playlist list/filmstrip/card view geometry.
#[derive(Default)]
pub(crate) struct LayoutPlaylist {
    pub row_map: Vec<Option<usize>>,
    pub rect: Rect,
    pub inner: Rect,
}

/// Geometry for the power-view's own Home sub-tab (the two-column card grid),
/// separate from `LayoutHome` which belongs to the regular Home tab.
#[derive(Default)]
pub(crate) struct LayoutPowerHome {
    pub hitmap: Vec<(Rect, usize)>,
    pub layout: Vec<PowerHomeSectionMeta>,
}

/// Power-view left panel, queue panel, and home-grid geometry.
#[derive(Default)]
pub(crate) struct LayoutPower {
    pub left_row_map: Vec<Option<usize>>,
    pub left_sorted_indices: Vec<usize>,
    pub left_area: Rect,
    /// Geometry for the power-view's own Home sub-tab grid. Distinct from
    /// `AppLayout::home` (`LayoutHome`), which is the regular Home-tab.
    pub home: LayoutPowerHome,
    pub queue_row_map: Vec<Option<usize>>,
    pub queue_area: Rect,
    pub queue_scope_local_area: Rect,
    pub queue_scope_remote_area: Rect,
    pub inline_image_rect: Option<Rect>,
    pub cursor_screen_y: Option<u16>,
    pub queue_cursor_screen_y: Option<u16>,
    pub selector_tabs: Vec<(Rect, usize)>,
    pub breadcrumbs: Vec<(u16, u16, u16, usize)>,
    /// Max valid scroll for the detail-view overview text, set each render frame.
    pub detail_max_scroll: usize,
    /// Visible overview line count for the detail view, set each render frame.
    pub detail_page_h: usize,
}

/// Library table/breadcrumb geometry.
#[derive(Default)]
pub(crate) struct LayoutLibrary {
    pub breadcrumbs: Vec<(u16, u16, u16, usize)>,
    pub lib_scroll: Vec<usize>,
    pub lib_row_heights: Vec<Vec<u16>>,
    pub lib_table_area: Vec<Rect>,
}

/// All per-frame layout geometry, grouped by the view that produces it.
/// `App` stores exactly one of these (`app.layout`); render writes into it,
/// input reads from it. See module docs for the rationale.
#[derive(Default)]
pub(crate) struct AppLayout {
    pub playback: LayoutPlayback,
    pub home: LayoutHome,
    pub playlist: LayoutPlaylist,
    pub power: LayoutPower,
    pub library: LayoutLibrary,
    pub tabs_area: Rect,
    pub tabbar_vol_area: Rect,
    pub settings_area: Rect,
    /// Mouse-row -> settings-line mapping, set each time the settings panel renders.
    pub settings_line_of_cursor: Vec<usize>,
    /// Bounding rect of the open context menu, if any, for click-outside dismissal.
    pub context_menu_rect: Option<Rect>,
}
