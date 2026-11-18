//! The Library list section (`gui-change-driven-sections` design D8): the
//! destination tabs the top bar paints, and the active destination's list part
//! (its collection selector, rows or tree, and load facts). The detail part
//! rides `libraryDetail` (see `detail.rs`).

use super::{
    GuiAudiobooksList, GuiCollections, GuiEmbyList, GuiFeedsList, GuiHomeList, GuiLibraryTabFrame,
    GuiMusicList, GuiPodcastsList, GuiShowsList,
};

/// The Library list section (design D8): the destination tabs, the tab the top
/// bar marks current, and the active destination's list part.
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuiDestinationList {
    Music(GuiMusicList),
    Home(GuiHomeList),
    Shows(GuiShowsList),
    Emby(GuiEmbyList),
    Audiobooks(GuiAudiobooksList),
    Podcasts(GuiPodcastsList),
    Feeds(GuiFeedsList),
}

impl GuiDestinationList {
    /// The destination's kind word for the log (`sections::log_built`): the
    /// lowercase destination name the log's `kind` field carries.
    #[must_use]
    pub fn kind_word(&self) -> &'static str {
        match self {
            Self::Music(_) => "music",
            Self::Home(_) => "home",
            Self::Shows(_) => "shows",
            Self::Emby(_) => "emby",
            Self::Audiobooks(_) => "audiobooks",
            Self::Podcasts(_) => "podcasts",
            Self::Feeds(_) => "feeds",
        }
    }

    /// The destination's collection selector, or `None` when the destination
    /// paints none.
    #[must_use]
    pub fn collections(&self) -> Option<&GuiCollections> {
        match self {
            Self::Music(list) => Some(&list.collections),
            Self::Home(_) | Self::Shows(_) | Self::Audiobooks(_) => None,
            Self::Emby(list) => list.collections.as_ref(),
            Self::Podcasts(list) => Some(&list.collections),
            Self::Feeds(list) => Some(&list.collections),
        }
    }

    /// The destination's row count for the log (`sections::log_built`): the
    /// rows the list paints — Music's artists, every other destination's
    /// rows.
    #[must_use]
    pub fn rows(&self) -> usize {
        match self {
            Self::Music(list) => list.artists.len(),
            Self::Home(list) => list.rows.len(),
            Self::Shows(list) => list.rows.len(),
            Self::Emby(list) => list.rows.len(),
            Self::Audiobooks(list) => list.rows.len(),
            Self::Podcasts(list) => list.rows.len(),
            Self::Feeds(list) => list.rows.len(),
        }
    }
}
