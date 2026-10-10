//! Home-owned requests the shell handles exhaustively (`handle_home_request`).

use mbv_ui_model::targets::HomeItemTarget;

#[derive(Debug, Clone, PartialEq)]
pub enum HomeRequest {
    /// Play the Home item the owner selected (spec `home-latest-sections`,
    /// "Home actions resolve the row's own section and Service identity"):
    /// `Enter` on an item row.
    Play(HomeItemTarget),
    /// Enqueue the Home item targets the owner resolved (same requirement):
    /// the marked item targets in display order, or the single selected item
    /// target. The shell applies the targets in the carried order.
    Enqueue(Vec<HomeItemTarget>),
    /// Remove the Home Continue Watching item targets the owner resolved
    /// (same requirement): the marked item targets in display order, or the
    /// single selected item target. The shell keeps the cw-range guard per
    /// target and drops every non-`Continue` target.
    Delete(Vec<HomeItemTarget>),
    /// Toggle watched state for the Home Continue Watching item targets the
    /// owner resolved (same requirement): the marked item targets in display
    /// order, or the single selected item target. The shell resolves each
    /// stable identity and never consults a cursor.
    ToggleWatched(Vec<HomeItemTarget>),
    /// Open the Home context menu for the resolved targets at an optional
    /// pointer anchor.
    ContextMenu(Vec<HomeItemTarget>, Option<(u16, u16)>),
    /// A row the user single-clicked in the Home list or Library Hero overlay.
    /// The component has already moved its own selection to the resolved row;
    /// the shell only pulls panel focus to the Library.
    RowClick(HomeItemTarget),
    /// A row the user double-clicked; `target` is the component-resolved item
    /// and the shell activates it.
    RowActivate(HomeItemTarget),
}
