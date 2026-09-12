//! Per-frame layout geometry produced by `App::compose_base_frame` and
//! consumed by mouse hit-testing in `input.rs`.
//!
//! `App` owns a single `AppLayout` value (`app.layout`) instead of ~35
//! scattered `layout_*`/`*`/`queue_*` fields. Grouping by view
//! mirrors the boundaries `render/` and `input.rs` already use, rather than
//! inventing a new one.
//!
//! Render code does not write into `self.layout` in place. Each call to
//! `App::compose_base_frame` builds a fresh, local `AppLayout::default()` and threads it
//! (or the relevant per-view sub-struct) through the render call graph as an
//! explicit parameter; every render function that used to write
//! `self.layout.<view>.<field> = ...` now writes `layout.<field> = ...` on
//! that local value instead. Only once the full pass completes does
//! `compose_base_frame` swap it into `self.layout` in a single atomic
//! assignment. This means
//! `self.layout` (read by `input.rs`) always reflects the last frame that
//! rendered in full, or is left completely untouched by an early return
//! (e.g. the zero-area guard) -- it can never hold a mix of fields from two
//! different frames.

use ratatui::layout::Rect;

use crate::app::render::arrangements::chrome::RootFrame;

/// Seekbar rect and the mouse hit targets for the one-row playback header's
/// transport controls (play/pause glyph and next).
/// The button/track/volume/subtitle/audio rects this used to hold were
/// removed with the expanded playback view; see the "Tab bar restyle" commit
/// that zeroed them out. The status-bar pill rects
/// (`ind_vol`/`ind_mu`/`ind_rc`) moved to the mounted `StatusBarPanel`
/// (task 2.2).
#[derive(Default)]
pub(crate) struct LayoutPlayback {
    pub player_area: Rect,
    /// Status-bar area used by the shell-mounted playback prompt component.
    pub status_area: Rect,
    pub seekbar_area: Rect,
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

/// Geometry produced by the queue card's authoritative render operation.
///
/// The card renderer returns the existing `(height, width, loading)` tuple;
/// this typed checkpoint records its dimensions in the fresh frame draft so
/// downstream queue placement consumes the published dimensions instead of
/// deriving them independently.
#[derive(Clone, Default)]
pub(crate) struct CardGeometry {
    pub height: u16,
    pub width: u16,
}

/// Library panel, queue panel, and home-grid geometry.
#[derive(Clone, Default)]
pub(crate) struct LayoutMain {
    /// Card geometry published immediately after the card's authoritative
    /// cache/size/fetch render path.
    pub card: CardGeometry,
    /// Full expanded sidebar covered by an F1-F4 panel, when present.
    pub panel_area: Rect,
    /// Content bounds inside `panel_area`, shared with panel mouse hit-testing.
    pub panel_content_area: Rect,
    /// Item rows of the last-rendered flat library list (plain and
    /// letter-grouped renderers), parallel to the display-row sequence:
    /// each entry holds the item indices occupying that display row, left to
    /// right (empty for headers/fillers). Column-aware cursor movement and
    /// mouse hit-testing resolve cells from this between frames.
    pub left_item_rows: Vec<Vec<usize>>,
    pub left_area: Rect,
    /// The full area `App::render_home_list` was given (hero + pills + list,
    /// not just the inner list). The shell reads this to re-paint the
    /// mounted `HomeComponent`'s `view()` over the same area right after
    /// `App::compose_base_frame` returns (task 3.4).
    pub home_area: Rect,
    /// The selected item's hero geometry. Wide screens place it beside `left_area`;
    /// inline screens place the replacement inside the list and use it as the
    /// selected parent's activation geometry.
    pub hero_area: Rect,
    /// Selected-parent geometry only for inline replacement. Wide hero areas
    /// remain render bookkeeping and are intentionally not interactive.
    pub inline_hero_area: Rect,
    /// Screen rect of the selected row/cell in the library panel. The outer
    /// selectable renderer owns this; nested detail/hero renderers never
    /// overwrite it. Consumed by the context menu's keyboard anchor.
    pub selected_item_rect: Option<Rect>,
    /// Pill/tab hitboxes published by the owning pill painters; placement and
    /// width remain owned by the shared pill-bar component. The music
    /// group-selector publishes these before paint.
    pub selector_tabs: Vec<(Rect, usize)>,
    pub breadcrumbs: Vec<(u16, u16, u16, usize)>,
}

/// Root/chrome frame geometry computed paint-free by
/// `App::compute_frame_layout` and consumed by `App::render_main` and the
/// chrome painters. This is the partial typed subresult of the staged
/// geometry/paint split (D2, task 2.1a): it owns the root/chrome fields only.
/// The full `AppLayout` remains the aggregate shared by every surface family;
/// non-migrated fields (queue/list/card/etc.) keep their legacy computation
/// until their own family rows migrate them.
#[derive(Default)]
pub(crate) struct FrameChromeGeometry {
    /// Full expanded sidebar covered by an F1-F4 panel, when present
    /// (`LayoutMain::panel_area`).
    pub panel_area: Rect,
    /// Content bounds inside `panel_area` (`LayoutMain::panel_content_area`).
    pub panel_content_area: Rect,
    /// Left panel (card + queue) column rect.
    pub left_area: Rect,
    /// Right panel (tabs, player, library, status) rect.
    pub right_area: Rect,
    /// Full-column right-panel background rect (tabs/player/library/status).
    pub right_full_area: Rect,
    /// Inner left-column content rect with the shared horizontal padding
    /// applied (queue and card paint areas are derived from this).
    pub left_content: Rect,
    /// Tab-bar box rect at the top of the right column.
    pub tab_bar_area: Rect,
    /// Player-panel rect directly below the tab bar (right column only).
    pub player_area: Rect,
    /// Status-bar rect at the bottom of the right panel.
    pub status_area: Rect,
    /// Whether the right panel is visible this frame (`panel_mode != QueueOnly`).
    pub right_visible: bool,
    /// Whether the queue panel holds panel focus this frame.
    pub queue_focused: bool,
    /// The root frame's panel placements for the current Panel mode (design
    /// D1, task 1.3): data only, consumed by later panel slices and by the
    /// mounted `QueueBoundaryComponent` (task 1.4).
    pub root: RootFrame,
}

/// All per-frame layout geometry, grouped by the view that produces it.
/// `App` stores exactly one of these (`app.layout`); render writes into it,
/// input reads from it. See module docs for the rationale.
#[derive(Default)]
pub(crate) struct AppLayout {
    pub playback: LayoutPlayback,
    pub main: LayoutMain,
    /// The last fully rendered frame's root panel placements (`RootFrame`).
    /// Published with the rest of the chrome checkpoint in
    /// `App::compose_base_frame` and read by the sync pass (the queue
    /// boundary's mount/rect source, task 1.4).
    pub root_frame: RootFrame,
}
