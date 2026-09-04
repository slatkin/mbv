//! Mouse hit-region enums reported by Interactive Components. Split from
//! `msg.rs` (task 8.3) to keep the central `Msg` file below the 800-line cap.
//!
//! The component paints the geometry and resolves the click region; the
//! shell turns the region plus `col`/`row` into the matching App gesture
//! (single vs double-click decided there via App's 400ms window). The
//! component holds no double-click or scroll timing state of its own.

/// Region of the Home surface a click resolved to, reported by
/// `HomeComponent` (task 5.3d, home hit_test). The shell turns this plus
/// `col`/`row` into the right gesture call; the component holds no double-click
/// or scroll timing state of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeHitRegion {
    /// The Home list area (`list_area`): the component resolves the row
    /// under the click; the shell decides whether the same coordinates form
    /// a single click (focus Library) or a double-click activation of the
    /// resolved flat target.
    Row(usize),
    /// Section pill; `target` is the section index the component resolved.
    Pill(usize),
    /// Right-click → Home context menu after the row is focused.
    ContextMenu(usize),
}

/// Region of the Queue surface a click resolved to, reported by
/// `QueueComponent` (task 5.3d, queue hit_test). The shell turns this plus
/// `col`/`row` into the matching App gesture; the component holds no
/// double-click or scroll timing state of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueHitRegion {
    /// Queue list area: a single click selects/focuses via
    /// The resolved slot target is applied by the shell, while the shell decides whether the same
    /// coordinates form a double-click activation.
    Row(Option<mbv_core::playback_queue::QueueSlotId>),
    /// Local queue scope pill.
    ScopeLocal,
    /// Remote queue scope pill.
    ScopeRemote,
    /// Right-click in the queue list area.
    ContextMenu(Option<mbv_core::playback_queue::QueueSlotId>),
}

/// Pane + hit within the TV workspace a click resolved to, reported by
/// `TvWorkspaceComponent` (task 5.3d, tv_workspace hit_test). The TV
/// workspace has two focusable panes, so a click's meaning depends on which
/// pane it lands in: Episodes-pane hits (season pill, episode row, blank
/// hero space) move the component's local pane focus to `Episodes` and pull
/// App's panel focus to the Library; Series-pane hits move the component's
/// pane to `Series` and set the library cursor in App. The component painted
/// the panes, so it resolves both; the shell never re-derives the pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TvHit {
    /// Season pill in the Episodes pane; index resolved by the component.
    SeasonTab(usize),
    /// Episode row in the Episodes pane; index resolved by the component.
    EpisodeRow(usize),
    /// Blank/hero space in the Episodes pane (no tab or row under the
    /// cursor): consumed without changing the pane or panel focus.
    EpisodesPane,
    /// The Series pane (series list): the series row the component resolved
    /// from its own painted geometry. The shell sets `App`'s library cursor
    /// to `target` before any pane effect (activation, context menu).
    SeriesRow(usize),
}

/// Region of the TV workspace a click resolved to (task 5.3d, tv_workspace
/// hit_test). The component resolves the pane and the hit within it; the
/// shell turns the region into the matching App gesture — single vs
/// double-click decided there via App's 400ms window — without re-deriving
/// the pane from the click coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TvHitRegion {
    /// A left click on the carried `TvHit`.
    Hit(TvHit),
    /// A right click; the carried `TvHit` is the pane + hit the click
    /// resolved to, so the shell applies the same pane-appropriate
    /// single-click effect (panel focus for Episodes-pane hits, series
    /// cursor for Series-pane hits) before opening the context menu at the
    /// click position.
    ContextMenu(TvHit),
}
