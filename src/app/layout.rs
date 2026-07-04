//! Per-frame layout geometry produced by `render()` and consumed by mouse
//! hit-testing in `input.rs`.
//!
//! `App` owns a single `AppLayout` value (`app.layout`) instead of ~35
//! scattered `layout_*`/`power_*`/`playlist_*` fields. Render code writes
//! into `self.layout.<view>.<field>` in place; input code reads from the
//! same paths. Grouping by view mirrors the boundaries `render/` and
//! `input.rs` already use, rather than inventing a new one.

use ratatui::layout::Rect;

use super::PowerHomeSectionMeta;

/// Seekbar/button/track/volume/subtitle/audio rects and their divider
/// status-indicator rects, shared across playback views.
#[derive(Default)]
pub(crate) struct LayoutPlayback {
    pub seekbar_area: Rect,
    pub button_area: Rect,
    pub tracks_area: Rect,
    pub vol_area: Rect,
    pub sub_area: Rect,
    pub audio_area: Rect,
    pub ind_au: Rect,
    pub ind_sub: Rect,
    pub ind_rc: Rect,
    pub ind_mu: Rect,
    pub ind_pb: Rect,
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

/// Power-view left panel, queue panel, and home-grid geometry.
#[derive(Default)]
pub(crate) struct LayoutPower {
    pub left_row_map: Vec<Option<usize>>,
    pub left_sorted_indices: Vec<usize>,
    pub left_area: Rect,
    pub home_hitmap: Vec<(Rect, usize)>,
    pub home_layout: Vec<PowerHomeSectionMeta>,
    pub queue_row_map: Vec<Option<usize>>,
    pub queue_area: Rect,
    pub queue_scope_local_area: Rect,
    pub queue_scope_remote_area: Rect,
    pub inline_image_rect: Option<Rect>,
    pub cursor_screen_y: Option<u16>,
    pub queue_cursor_screen_y: Option<u16>,
    pub selector_tabs: Vec<(Rect, usize)>,
    pub breadcrumbs: Vec<(u16, u16, u16, usize)>,
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
}
