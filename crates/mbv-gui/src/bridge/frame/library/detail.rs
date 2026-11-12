//! The Library detail section (`gui-change-driven-sections` design D8): the
//! active destination's detail part, the selection, the Hero, the Workspace,
//! and the playing marks. The list part rides `libraryList` (see `list.rs`).

use serde::Serialize;

use super::{GuiHomeDetail, GuiMusicDetail};

/// The Library detail section (design D8): the active destination's detail
/// part.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiLibraryDetail {
    /// The active destination's detail part (design D8).
    pub destination: GuiDestinationDetail,
}

/// The active destination's detail part (design D8), tagged with the
/// destination the same way `GuiDestinationList` is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "destination", rename_all = "camelCase")]
pub enum GuiDestinationDetail {
    Music(GuiMusicDetail),
    Home(GuiHomeDetail),
}
