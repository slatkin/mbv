//! The Library list section (`gui-change-driven-sections` design D8): the
//! destination tabs the top bar paints, and the active destination's list part
//! (its pills, rows or tree, and load facts). The detail part rides
//! `libraryDetail` (see `detail.rs`).

use serde::Serialize;

use super::{
    GuiAudiobooksList, GuiEmbyList, GuiFeedsList, GuiHomeList, GuiLibraryTabFrame, GuiMusicList,
    GuiPodcastsList, GuiShowsList,
};

/// The Library list section (design D8): the destination tabs, the tab the top
/// bar marks current, and the active destination's list part.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiLibraryList {
    /// The presented destination tabs in catalog order (design D2).
    pub tabs: Vec<GuiLibraryTabFrame>,
    /// The active tab's position while the active destination is one of the
    /// tabs (design D2); `None` for Home and every destination the GUI does
    /// not present yet.
    pub current_tab: Option<usize>,
    /// The active destination's list part (design D8).
    pub destination: GuiDestinationList,
}

/// The active destination's list part (design D8), tagged with the destination
/// so the window reads one shape per destination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "destination", rename_all = "camelCase")]
pub enum GuiDestinationList {
    Music(GuiMusicList),
    Home(GuiHomeList),
    Shows(GuiShowsList),
    Emby(GuiEmbyList),
    Audiobooks(GuiAudiobooksList),
    Podcasts(GuiPodcastsList),
    Feeds(GuiFeedsList),
}
