//! The Library detail section (`gui-change-driven-sections` design D8): the
//! active destination's detail part, the selection, the Hero, the Workspace,
//! and the playing marks. The list part rides `libraryList` (see `list.rs`).

use serde::Serialize;

use super::{
    GuiAudiobooksDetail, GuiEmbyDetail, GuiFeedsDetail, GuiHomeDetail, GuiMusicDetail,
    GuiPodcastsDetail, GuiShowsDetail,
};

/// The Library detail section (design D8): the active destination's detail
/// part, tagged with the destination (`kind`) the same way `GuiDestinationList`
/// is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum GuiLibraryDetail {
    Music(GuiMusicDetail),
    Home(GuiHomeDetail),
    Shows(GuiShowsDetail),
    Emby(GuiEmbyDetail),
    Audiobooks(GuiAudiobooksDetail),
    Podcasts(GuiPodcastsDetail),
    Feeds(GuiFeedsDetail),
}
