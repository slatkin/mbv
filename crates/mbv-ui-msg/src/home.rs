//! Home-owned requests the shell handles exhaustively (`handle_home_request`).

use mbv_ui_model::targets::HomeItemTarget;

#[derive(Debug, Clone, PartialEq)]
pub enum HomeRequest {
    /// Play the Home item the owner selected (design D6): `Enter` on an item
    /// row.
    Play(HomeItemTarget),
    /// Enqueue the Home item targets the owner resolved (design D6): the
    /// marked item targets in display order, or the single selected item
    /// target. The shell applies the targets in the carried order.
    Enqueue(Vec<HomeItemTarget>),
    /// Remove the Home Continue Watching item targets the owner resolved
    /// (design D6/D7): the marked item targets in display order, or the
    /// single selected item target. The shell keeps the cw-range guard per
    /// target and drops every non-`Continue` target.
    Delete(Vec<HomeItemTarget>),
    /// Toggle watched state for the Home Continue Watching item targets the
    /// owner resolved (design D6): the marked item targets in display order,
    /// or the single selected item target. The shell resolves each stable
    /// identity and never consults a cursor.
    ToggleWatched(Vec<HomeItemTarget>),
    /// Open the Home context menu for the resolved targets at an optional
    /// pointer anchor. The shell converts Home-target
    /// `ShellRequest::RowContextMenu` into this variant.
    ContextMenu(Vec<HomeItemTarget>, Option<(u16, u16)>),
}
