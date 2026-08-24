//! Overlay sync/render module declarations for the shell `Model` (design D2/D9).

#[path = "shell_overlays_menus.rs"]
mod menus;
#[path = "shell_overlays_modals.rs"]
mod modals;
#[path = "shell_overlays_sidebars.rs"]
mod sidebars;

#[cfg(test)]
use super::components::{
    ComponentId, FeedsManageComponent, LibraryRoutesComponent, OverlayId, PopupId, ShellRequest,
};
#[cfg(test)]
use super::shell::Model;

include!("shell_overlays_tests.rs");
include!("shell_selection_modal_tests.rs");
