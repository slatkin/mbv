//! Overlay sync/render module declarations for the shell `Model` (design D2/D9).

mod menus;
mod modals;
mod sidebars;

#[cfg(test)]
use super::components::{ComponentId, OverlayId, PopupId};
#[cfg(test)]
use super::Model;

#[cfg(test)]
mod tests;
