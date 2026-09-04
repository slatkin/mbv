//! `TvHit`: the resolved semantic target the TV workspace emits after
//! recognizing the gesture itself via its own `MouseGestureState`. Split from
//! `msg.rs` (task 8.3) to keep the central `Msg` file below the 800-line cap.
//! See `docs/adr/0024-mouse-events-through-component-subscriptions.md`.

/// Pane + hit within the TV workspace a click resolved to. The TV workspace
/// has two focusable panes, so a click's meaning depends on which pane it
/// lands in: Episodes-pane hits (season pill, episode row, blank hero space)
/// move the component's local pane focus to `Episodes` and pull App's panel
/// focus to the Library; Series-pane hits move the component's pane to
/// `Series` and set the library cursor in App. The component painted the
/// panes, so it resolves both; the shell never re-derives the pane.
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
