//! The Library detail section (`gui-change-driven-sections` design D8): the
//! active destination's detail part, the selection, the Hero, the Workspace,
//! and the playing marks. The list part rides `libraryList` (see `list.rs`).

use super::{
    GuiAudiobooksDetail, GuiEmbyDetail, GuiFeedsDetail, GuiHomeDetail, GuiMusicDetail,
    GuiPodcastsDetail, GuiShowsDetail,
};

/// The Library detail section (design D8): the active destination's detail
/// part, tagged with the destination (`kind`) the same way `GuiDestinationList`
/// is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuiLibraryDetail {
    Music(GuiMusicDetail),
    Home(GuiHomeDetail),
    Shows(GuiShowsDetail),
    Emby(GuiEmbyDetail),
    Audiobooks(GuiAudiobooksDetail),
    Podcasts(GuiPodcastsDetail),
    Feeds(GuiFeedsDetail),
}

impl GuiLibraryDetail {
    /// Whether the detail carries a Hero, for the log (`sections::log_built`).
    #[must_use]
    pub fn has_hero(&self) -> bool {
        match self {
            Self::Music(detail) => detail.hero.is_some(),
            Self::Home(detail) => detail.hero.is_some(),
            Self::Shows(detail) => detail.hero.is_some(),
            Self::Emby(detail) => detail.hero.is_some(),
            Self::Audiobooks(detail) => detail.hero.is_some(),
            Self::Podcasts(detail) => detail.hero.is_some(),
            Self::Feeds(detail) => detail.hero.is_some(),
        }
    }
}
