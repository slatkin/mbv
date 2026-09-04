//! Mouse hit-region enums reported by Interactive Components. Split from
//! `msg.rs` (task 8.3) to keep the central `Msg` file below the 800-line cap.
//!
//! The component paints the geometry and resolves the click region; the
//! shell turns the region plus `col`/`row` into the matching App gesture
//! (single vs double-click decided there via App's 400ms window). The
//! component holds no double-click or scroll timing state of its own.

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
