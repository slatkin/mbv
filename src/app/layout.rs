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

use super::{PanelFocus, PanelMode};
use ratatui::layout::Rect;

/// Seekbar rect, the two divider status indicators that still have a click
/// target (remote-session and mute), the volume pill's scroll target, and
/// the mouse hit targets for the one-row playback header's transport
/// controls (play/pause glyph and next).
/// The button/track/volume/subtitle/audio rects this used to hold were
/// removed with the expanded playback view; see the "Tab bar restyle" commit
/// that zeroed them out.
#[derive(Default)]
pub(crate) struct LayoutPlayback {
    pub player_area: Rect,
    /// Status-bar area used by the shell-mounted playback prompt component.
    pub status_area: Rect,
    pub seekbar_area: Rect,
    pub ind_rc: Rect,
    pub ind_mu: Rect,
    /// Status-bar volume pill; scroll-wheel hit test.
    pub ind_vol: Rect,
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
#[derive(Default)]
pub(crate) struct CardGeometry {
    pub height: u16,
    pub width: u16,
}

/// Library panel, queue panel, and home-grid geometry.
#[derive(Default)]
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
    /// The exact full-height trailing column reserved for Queue boundary resizing.
    pub queue_boundary_area: Rect,
    /// The full area `App::render_home_list` was given (hero + pills + list,
    /// not just the inner list). The shell reads this to re-paint the
    /// mounted `HomeComponent`'s `view()` over the same area right after
    /// `App::compose_base_frame` returns (task 3.4).
    pub home_area: Rect,
    /// The full area passed to the Feeds renderer. The shell uses this to
    /// repaint the mounted `FeedsComponent` over the legacy frame.
    pub feeds_area: Rect,
    /// The selected item's hero geometry. Wide screens place it beside `left_area`;
    /// inline screens place the replacement inside the list and use it as the
    /// selected parent's activation geometry.
    pub hero_area: Rect,
    /// Selected-parent geometry only for inline replacement. Wide hero areas
    /// remain render bookkeeping and are intentionally not interactive.
    pub inline_hero_area: Rect,
    /// Queue placement and scope areas published independently of mounted
    /// component-local queue geometry.
    pub queue_area: Rect,
    pub queue_title_area: Option<Rect>,
    /// Screen rect of the selected row/cell in the library panel. The outer
    /// selectable renderer owns this; nested detail/hero renderers never
    /// overwrite it. Consumed by the context menu's keyboard anchor.
    pub selected_item_rect: Option<Rect>,
    /// Screen rect of the selected queue row. Owned by the queue renderer.
    pub queue_selected_item_rect: Option<Rect>,
    /// Pill/tab hitboxes published by the owning pill painters; placement and
    /// width remain owned by the shared pill-bar component.
    /// Pill/tab hitboxes published by the owning pill painters; placement and
    /// width remain owned by the shared pill-bar component. The music
    /// group-selector publishes these before paint.
    pub selector_tabs: Vec<(Rect, usize)>,
    pub breadcrumbs: Vec<(u16, u16, u16, usize)>,
    /// Per-tab hit targets published by `render_tabs` (task 6.5). Each entry
    /// is `(screen_rect, tab_position)` for a visible tab, using the tab's
    /// real position (`all_names` index), not its visible-slot index. Does
    /// not include the `«`/`»` scroll-indicator glyphs. Cleared and
    /// repopulated every frame; paint-coupled by design.
    pub tabs_hitmap: Vec<(Rect, usize)>,
    /// Bounding rect of the wide Music right pane's hero artwork area.
    /// Clicks here should not activate track selection or playback.
    pub wide_music_art_area: Rect,
    /// Full area passed to the grouped Music component after legacy layout.
    pub wide_music_area: Rect,
    /// Bounding rect of the wide Music left pane (album browser).
    /// Populated only when the wide Music layout is active.
    pub wide_music_right_area: Rect,
    /// Bounding rect of the wide Movies right rail (pills + list).
    /// Populated only when the wide Movies Wide hero layout is active.
    pub movies_wide_right_area: Rect,
    // TV-wide geometry (2.1i): `tv_wide_area`/`tv_wide_left_area`/
    // `tv_wide_right_area`/`tv_wide_list_area` are published at their
    // natural checkpoint before `render_list`, gated by the
    // `wide_hero_presentation` breakpoint, with loading preserved
    // component-side. Shared widget geometry (`selector_tabs`, left row
    // maps, hero/selected rects) was already published pre-paint by rows
    // 2.1a–2.1f. Component-local `tv_wide_episode_rows`/
    // `tv_wide_season_tabs` are paint-coupled by design (2.1b carve-out,
    // component-internal only).
    pub tv_wide_right_area: Rect,
    pub tv_wide_list_area: Rect,
    /// Paint area of the embedded episode `WideMediaList` (task 4.2d): the
    /// canonical control resolves its own row hits against this rect, so no
    /// per-row hit map is published here.
    pub tv_wide_episode_list_area: Rect,
    pub tv_wide_season_tabs: Vec<(Rect, usize)>,
    pub tv_wide_left_area: Rect,
    pub tv_wide_area: Rect,
    /// Bounding rect of the grouped-album browser itself, the sub-rect of
    /// `wide_music_right_area` below the pill row. The embedded canonical
    /// control owns row identity and hit geometry within this rect.
    pub wide_music_browser_area: Rect,
    /// Full area passed to the Audiobookshelf podcast component after the
    /// legacy frame computes the current library layout.
    pub audiobookshelf_podcast_area: Rect,
    /// Full area passed to the Audiobookshelf book component after the legacy
    /// frame computes the current library layout.
    pub audiobookshelf_book_area: Rect,
}

impl LayoutMain {
    /// Returns the tab position whose hit target contains `pos`, if any.
    pub(crate) fn tab_at(&self, pos: ratatui::layout::Position) -> Option<usize> {
        self.tabs_hitmap
            .iter()
            .find(|(rect, _)| rect.contains(pos))
            .map(|(_, tab_pos)| *tab_pos)
    }
}

/// Which column holds panel focus for one frame, and whether the right
/// (library) column is on screen at all.
///
/// One value per frame, derived by `App::focus_state` from the effective
/// panel focus and the effective panel mode. Deliberately carries no
/// sub-surface or cursor position: where the selection sits inside a column
/// never changes that column's colour.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FocusState {
    column: Column,
    right_visible: bool,
}

/// The column holding panel focus this frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Column {
    /// The queue column (left).
    Left,
    /// The library column (right).
    Right,
}

impl FocusState {
    /// Derive the frame's focus state from the effective panel focus and the
    /// effective panel mode.
    pub(in crate::app) fn new(panel_focus: PanelFocus, panel_mode: PanelMode) -> Self {
        let column = match panel_focus {
            PanelFocus::Queue => Column::Left,
            PanelFocus::Library => Column::Right,
        };
        Self {
            column,
            right_visible: panel_mode != PanelMode::QueueOnly,
        }
    }

    /// Whether the queue (left) column holds panel focus this frame.
    pub(crate) fn queue_column_focused(&self) -> bool {
        self.column == Column::Left
    }

    /// Whether the right (library) column holds panel focus this frame.
    ///
    /// False whenever the right column is not visible, even if a stale
    /// `Library` focus says otherwise.
    pub(crate) fn library_column_focused(&self) -> bool {
        self.right_visible && self.column == Column::Right
    }
}

impl Default for FocusState {
    fn default() -> Self {
        // The pre-`FocusState` zero value of `FrameChromeGeometry`: no visible
        // right column and neither column reporting focus.
        Self {
            column: Column::Right,
            right_visible: false,
        }
    }
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
    /// Exact full-height trailing column reserved for the boundary component.
    pub queue_boundary_area: Rect,
    /// Right panel (tabs, player, library, status) rect.
    pub right_area: Rect,
    /// Full-column right-panel background rect (tabs/player/library/status).
    pub right_full_area: Rect,
    /// Inner left-column content rect with the shared horizontal padding
    /// applied (queue and card paint areas are derived from this).
    pub left_content: Rect,
    /// Tab-bar box rect at the top of the right column.
    pub tab_bar_area: Rect,
    /// Tab-bar hit targets (`AppLayout::tabs_area`), published only when the
    /// right panel is visible; `Rect::default()` otherwise.
    pub tabs_area: Rect,
    /// Player-panel rect directly below the tab bar (right column only).
    pub player_area: Rect,
    /// Status-bar rect at the bottom of the right panel.
    pub status_area: Rect,
    /// Whether the right panel is visible this frame (`panel_mode != QueueOnly`).
    pub right_visible: bool,
    /// Which column holds panel focus this frame, and whether the right
    /// (library) column is on screen at all. The one value both column
    /// surfaces resolve from, so neither fact is re-derived per screen:
    /// `focus.queue_column_focused()` is the queue column's focus, and
    /// `focus.library_column_focused()` is the right column's (false whenever
    /// the column is not visible, even under a stale `Library` focus).
    pub focus: FocusState,
}

/// All per-frame layout geometry, grouped by the view that produces it.
/// `App` stores exactly one of these (`app.layout`); render writes into it,
/// input reads from it. See module docs for the rationale.
#[derive(Default)]
pub(crate) struct AppLayout {
    pub playback: LayoutPlayback,
    pub main: LayoutMain,
    pub tabs_area: Rect,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_state_reproduces_the_column_focus_facts() {
        // Wide LibraryOnly: the right column is the whole frame and the
        // library panel holds focus, so the column surface follows it.
        let library_only = FocusState::new(PanelFocus::Library, PanelMode::LibraryOnly);
        assert!(library_only.library_column_focused());
        assert!(!library_only.queue_column_focused());

        // Wide Both with library focus: the column is the surface the focused
        // library panel sits on.
        let both_library = FocusState::new(PanelFocus::Library, PanelMode::Both);
        assert!(both_library.library_column_focused());
        assert!(!both_library.queue_column_focused());

        // Wide Both with queue focus: the column is still visible but resting.
        let both_queue = FocusState::new(PanelFocus::Queue, PanelMode::Both);
        assert!(!both_queue.library_column_focused());
        assert!(both_queue.queue_column_focused());

        // QueueOnly: no right column exists, so it never reports focus even
        // though the stored focus bit is Library.
        let queue_only = FocusState::new(PanelFocus::Library, PanelMode::QueueOnly);
        assert!(!queue_only.library_column_focused());
        assert!(!queue_only.queue_column_focused());
    }

    /// Narrow mini-view halves. Below `MINI_VIEW_THRESHOLD` `App` collapses
    /// `mini_view_focus` into one of two effective shapes before deriving this
    /// state: the library half supplies `LibraryOnly`/`Library` (the right
    /// column occupies the full frame width and is therefore visible and
    /// focused), while the queue half supplies `QueueOnly`/`Queue` (no right
    /// column at all).
    #[test]
    fn focus_state_reports_the_two_mini_view_halves() {
        let library_half = FocusState::new(PanelFocus::Library, PanelMode::LibraryOnly);
        assert!(library_half.library_column_focused());
        assert!(!library_half.queue_column_focused());

        let queue_half = FocusState::new(PanelFocus::Queue, PanelMode::QueueOnly);
        assert!(!queue_half.library_column_focused());
        assert!(queue_half.queue_column_focused());
    }
}
