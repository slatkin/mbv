//! Per-frame layout geometry produced by `render()` and consumed by mouse
//! hit-testing in `input.rs`.
//!
//! `App` owns a single `AppLayout` value (`app.layout`) instead of ~35
//! scattered `layout_*`/`*`/`queue_*` fields. Grouping by view
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
    /// Idle-feed headline; only populated when its current item has a link.
    pub idle_feed_link_area: Rect,
}

/// Geometry for the Home sub-tab,
/// separate from `LayoutHome` which belongs to the regular Home tab.
#[derive(Default)]
pub(crate) struct LayoutHome {
    pub hitmap: Vec<(Rect, usize)>,
}

/// Library panel, queue panel, and home-grid geometry.
#[derive(Default)]
pub(crate) struct LayoutMain {
    /// Full expanded sidebar covered by an F1-F4 panel, when present.
    pub panel_area: Rect,
    /// Content bounds inside `panel_area`, shared with panel mouse hit-testing.
    pub panel_content_area: Rect,
    pub left_row_map: Vec<Option<usize>>,
    /// Item rows of the last-rendered flat library list (plain and
    /// letter-grouped renderers), parallel to the display-row sequence:
    /// each entry holds the item indices occupying that display row, left to
    /// right (empty for headers/fillers). Column-aware cursor movement and
    /// mouse hit-testing resolve cells from this between frames.
    pub left_item_rows: Vec<Vec<usize>>,
    /// Screen-row offset for `left_item_rows` when the renderer packs display
    /// rows into screen rows (e.g. grouped album views with two-column
    /// layout). The mouse handler adds this (instead of `lvl.scroll`) to
    /// `click_y` to index into `left_item_rows`.
    pub left_screen_offset: usize,
    pub left_row_targets: Vec<Option<LibraryRowTarget>>,
    pub left_sorted_indices: Vec<usize>,
    pub left_area: Rect,
    /// The selected item's hero banner, pinned to the top of the content
    /// area above `left_area`, when the library list shows one; zero rect
    /// otherwise. A click here is an Enter equivalent — it opens the
    /// selected item (see `App::click_set_cursor`).
    pub hero_area: Rect,
    /// Geometry for the Home sub-tab grid. Distinct from
    /// `AppLayout::home` (`LayoutHome`), which is the regular Home-tab.
    pub home: LayoutHome,
    pub queue_row_map: Vec<Option<usize>>,
    pub queue_area: Rect,
    pub queue_scope_local_area: Rect,
    pub queue_scope_remote_area: Rect,
    pub inline_image_rect: Option<Rect>,
    pub cursor_screen_y: Option<u16>,
    pub queue_cursor_screen_y: Option<u16>,
    pub selector_tabs: Vec<(Rect, usize)>,
    pub breadcrumbs: Vec<(u16, u16, u16, usize)>,
    /// Per-track hit targets for the wide Music left pane. Each entry is
    /// `(screen_rect, track_index)` covering all wrapped physical rows of
    /// that logical track. Cleared every frame; populated only when the
    /// wide Music layout is active.
    pub wide_music_track_hitmap: Vec<(Rect, usize)>,
    /// Bounding rect of the wide Music left pane's hero artwork area.
    /// Clicks here should not activate track selection or playback.
    pub wide_music_art_area: Rect,
    /// Bounding rect of the wide Music right pane (album browser).
    /// Populated only when the wide Music layout is active.
    pub wide_music_right_area: Rect,
}

impl LayoutMain {
    /// Whether the wide Music layout rendered this frame. The right pane
    /// area is only set by the wide Music renderer, so it is a reliable
    /// signal even when the track hitmap is empty (tracks still loading
    /// or album has no tracks).
    pub(crate) fn is_wide_music_active(&self) -> bool {
        self.wide_music_right_area.width > 0 && self.wide_music_right_area.height > 0
    }

    /// Returns the track index whose hit target contains `pos`, if any.
    pub(crate) fn wide_music_track_at(&self, pos: ratatui::layout::Position) -> Option<usize> {
        self.wide_music_track_hitmap
            .iter()
            .find(|(rect, _)| rect.contains(pos))
            .map(|(_, track_idx)| *track_idx)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum LibraryRowTarget {
    Album(usize),
}

/// All per-frame layout geometry, grouped by the view that produces it.
/// `App` stores exactly one of these (`app.layout`); render writes into it,
/// input reads from it. See module docs for the rationale.
#[derive(Default)]
pub(crate) struct AppLayout {
    pub playback: LayoutPlayback,
    pub main: LayoutMain,
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
