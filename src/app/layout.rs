//! Per-frame layout geometry produced by `render()` and consumed by mouse
//! hit-testing in `input.rs`.
//!
//! `App` owns a single `AppLayout` value (`app.layout`) instead of ~35
//! scattered `layout_*`/`power_*`/`queue_*` fields. Grouping by view
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

use super::ArtistHeaderSelection;
use ratatui::layout::Rect;

/// Seekbar rect, the two divider status indicators that still have a click
/// target (remote-session and mute), and the mouse hit targets for the
/// one-row playback header's transport controls (play/pause glyph and next).
/// The button/track/volume/subtitle/audio rects this used to hold were
/// removed with the expanded playback view; see the "Tab bar restyle" commit
/// that zeroed them out.
#[derive(Default)]
pub(crate) struct LayoutPlayback {
    pub seekbar_area: Rect,
    pub ind_rc: Rect,
    pub ind_mu: Rect,
    /// Playback header play/pause glyph; always clickable when the row renders.
    pub play_pause_area: Rect,
    /// Playback header stop glyph; only wired to the action when
    /// `App::transport_stop_available()` is true.
    pub stop_area: Rect,
    /// Playback header next glyph; only wired to the action when
    /// `App::transport_prev_next_available().1` is true.
    pub next_area: Rect,
}

/// Geometry for the power-view's own Home sub-tab,
/// separate from `LayoutHome` which belongs to the regular Home tab.
#[derive(Default)]
pub(crate) struct LayoutPowerHome {
    pub hitmap: Vec<(Rect, usize)>,
}

/// Power-view left panel, queue panel, and home-grid geometry.
#[derive(Default)]
pub(crate) struct LayoutPower {
    /// Full expanded Power sidebar covered by an F1-F4 panel, when present.
    pub panel_area: Rect,
    /// Content bounds inside `panel_area`, shared with panel mouse hit-testing.
    pub panel_content_area: Rect,
    pub left_row_map: Vec<Option<usize>>,
    pub left_row_targets: Vec<Option<PowerLeftRowTarget>>,
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PowerLeftRowTarget {
    Album(usize),
    ArtistHeader(ArtistHeaderSelection),
}

/// All per-frame layout geometry, grouped by the view that produces it.
/// `App` stores exactly one of these (`app.layout`); render writes into it,
/// input reads from it. See module docs for the rationale.
#[derive(Default)]
pub(crate) struct AppLayout {
    pub playback: LayoutPlayback,
    pub power: LayoutPower,
    pub tabs_area: Rect,
    pub tabbar_vol_area: Rect,
    pub settings_area: Rect,
    /// Inset content bounds used by the Settings renderer and mouse hit-testing.
    pub settings_content_area: Rect,
    /// Mouse-row -> settings-line mapping, set each time the settings panel renders.
    pub settings_line_of_cursor: Vec<usize>,
    /// Bounding rect of the open context menu, if any, for click-outside dismissal.
    pub context_menu_rect: Option<Rect>,
}
