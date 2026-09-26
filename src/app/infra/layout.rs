//! Per-frame layout geometry published by root frame composition for render,
//! sizing, and test consumers.
//!
//! `App` owns a single `AppLayout` value (`app.layout`) instead of ~35
//! scattered `layout_*`/`*`/`queue_*` fields. Grouping by view mirrors the
//! boundaries `render/` already uses, rather than inventing a new one.
//!
//! Render code does not write into `self.layout` in place. Each call to root
//! frame composition builds a fresh, local `AppLayout::default()` and threads
//! it (or the relevant per-view sub-struct) through the render call graph as
//! an explicit parameter; every render function that used to write
//! `self.layout.<view>.<field> = ...` now writes `layout.<field> = ...` on
//! that local value instead. Only once the full pass completes does it swap
//! the value into `self.layout` in a single atomic assignment. This means
//! `self.layout` always reflects the last frame rendered in full, or is left
//! completely untouched by an early return (e.g. the zero-area guard) -- it
//! can never hold a mix of fields from two different frames.

use ratatui::layout::Rect;

pub(in crate::app) const LEFT_WIDTH_DEFAULT: u16 = 40;
pub(in crate::app) const LEFT_WIDTH_STEP: u16 = 5;
/// The single wide/narrow breakpoint. Minimum list-pane / Home-pane width at
/// which the view switches to a two-column layout. Every screen and
/// arrangement reads this one constant instead of testing width itself; the
/// library list's column count derives from it (`library_column_count`), not
/// the other way around.
pub(in crate::app) const TWO_COLUMN_THRESHOLD: u16 = 82;
/// The narrow-terminal breakpoint below which the Power View uses the
/// two-state "mini view" (`x` toggles library-only <-> queue-only) instead of
/// the three-state both/queue-only/library-only cycle. Independent of and
/// unrelated to `TWO_COLUMN_THRESHOLD` (82), which governs the library
/// panel's internal list-column layout (see design.md).
pub(in crate::app) const MINI_VIEW_THRESHOLD: u16 = 80;
/// Left margin for the tab row. The control pill used to live here (hence
/// the old, larger reservation); it now renders in the status bar (see
/// `render_status_bar`) and the tabs are left-aligned flush with the left
/// edge instead.
pub(in crate::app) const TABBAR_LEFT_RESERVE: u16 = 0;

pub(in crate::app) const PAGE_SIZE: usize = 100;
pub(in crate::app) const PREFETCH_AHEAD: usize = 25;
pub(in crate::app) const SEARCH_PANEL_W: u16 = 40;

use crate::app::render::arrangements::chrome::RootFrame;

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

/// Test-only row/hero/pill geometry shape, mirroring the fields the deleted
/// destination components (Home/TV/Music) used to publish onto legacy chrome geometry
/// (task 12.3). It exists solely so the shared characterization test helpers
/// keep one common return shape; production code never constructs it — each
/// owning component answers real requests (mouse-hit, context-menu anchor)
/// from its own retained fields instead.
#[cfg(test)]
#[derive(Clone, Default)]
pub(crate) struct PaintedRowGeometry {
    pub selected_item_rect: Option<Rect>,
}

/// Root/chrome frame geometry computed paint-free by
/// `App::compute_frame_layout` and consumed by the shell draw path and the
/// chrome painters. This is the partial typed subresult of the staged
/// geometry/paint split (D2, task 2.1a): it owns the root/chrome fields only.
/// The full `AppLayout` remains the aggregate shared by every surface family;
/// non-migrated fields (queue/list/card/etc.) keep their legacy computation
/// until their own family rows migrate them.
#[derive(Default)]
pub(crate) struct FrameChromeGeometry {
    /// Full expanded sidebar covered by an F1-F4 panel, when present
    /// (the former panel area).
    pub panel_area: Rect,
    /// Left panel (card + queue) column rect.
    pub left_area: Rect,
    /// Right panel (tabs, player, library, status) rect.
    pub right_area: Rect,
    /// Inner left-column content rect with the shared horizontal padding
    /// applied (queue and card paint areas are derived from this).
    pub left_content: Rect,
    /// Tab-bar box rect at the top of the right column.
    pub tab_bar_area: Rect,
    /// Whether the right panel is visible this frame (`panel_mode != QueueOnly`).
    pub right_visible: bool,
    /// The root frame's panel placements for the current Panel mode (design
    /// D1, task 1.3): data only, consumed by later panel slices and by the
    /// mounted `QueueBoundaryComponent` (task 1.4).
    pub root: RootFrame,
}

/// All per-frame layout geometry, grouped by the view that produces it.
/// `App` stores exactly one of these (`app.layout`); render writes into it,
/// and sizing, paint, and test consumers read from it. See module docs for the
/// rationale.
#[derive(Default)]
pub(crate) struct AppLayout {
    /// Left panel (card + queue) column rect.
    pub left_area: Rect,
    /// The queue card's own paint geometry, retained for paint and test code;
    /// unlike the removed `FrameChromeGeometry` input-path mirror, it is not
    /// chrome geometry used for click resolution.
    pub card: CardGeometry,
    /// The last fully rendered frame's root panel placements (`RootFrame`).
    /// Published with the rest of the chrome checkpoint in
    /// root frame composition and read by the sync pass (the queue
    /// boundary's mount/rect source, task 1.4).
    pub root_frame: RootFrame,
}
