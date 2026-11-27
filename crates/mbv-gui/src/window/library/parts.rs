//! The typed readers of the Library sections (`gui-change-driven-sections`
//! design D8, the port of `lib/library/parts.ts`): the `libraryList` and
//! `libraryDetail` stores, narrowed on the destination's kind. A reader
//! returns the part only while the active destination is that one, and `None`
//! otherwise. The core builds both parts in one pass, so a list part and its
//! detail part are `None` together.

use crate::bridge::frame::library::{
    GuiAudiobooksDetail, GuiAudiobooksList, GuiDestinationList, GuiEmbyDetail, GuiEmbyList,
    GuiFavouritesList, GuiFeedsDetail, GuiFeedsList, GuiHomeDetail, GuiHomeList, GuiLibraryDetail,
    GuiLibraryList, GuiMusicDetail, GuiMusicList, GuiPodcastsDetail, GuiPodcastsList,
    GuiShowsDetail, GuiShowsList,
};

/// The active destination's list part, narrowed on its kind: the Music list
/// part, or `None` while another destination is active.
#[must_use]
pub fn library_music_list(list: Option<&GuiLibraryList>) -> Option<&GuiMusicList> {
    match list.map(|list| &list.destination) {
        Some(GuiDestinationList::Music(destination)) => Some(destination),
        _ => None,
    }
}

/// The active destination's detail part, narrowed on its kind: the Music
/// detail part, or `None` while another destination is active.
#[must_use]
pub fn library_music_detail(detail: Option<&GuiLibraryDetail>) -> Option<&GuiMusicDetail> {
    match detail {
        Some(GuiLibraryDetail::Music(detail)) => Some(detail),
        _ => None,
    }
}

/// The active destination's list part: the Home list part, or `None` while
/// another destination is active.
#[must_use]
pub fn library_home_list(list: Option<&GuiLibraryList>) -> Option<&GuiHomeList> {
    match list.map(|list| &list.destination) {
        Some(GuiDestinationList::Home(destination)) => Some(destination),
        _ => None,
    }
}

/// The active destination's detail part: the Home detail part, or `None`
/// while another destination is active.
#[must_use]
pub fn library_home_detail(detail: Option<&GuiLibraryDetail>) -> Option<&GuiHomeDetail> {
    match detail {
        Some(GuiLibraryDetail::Home(detail)) => Some(detail),
        _ => None,
    }
}

/// The active destination's list part: the Shows list part, or `None` while
/// another destination is active.
#[must_use]
pub fn library_shows_list(list: Option<&GuiLibraryList>) -> Option<&GuiShowsList> {
    match list.map(|list| &list.destination) {
        Some(GuiDestinationList::Shows(destination)) => Some(destination),
        _ => None,
    }
}

/// The active destination's detail part: the Shows detail part, or `None`
/// while another destination is active.
#[must_use]
pub fn library_shows_detail(detail: Option<&GuiLibraryDetail>) -> Option<&GuiShowsDetail> {
    match detail {
        Some(GuiLibraryDetail::Shows(detail)) => Some(detail),
        _ => None,
    }
}

/// The active destination's list part: the plain Emby-library list part, or
/// `None` while another destination is active.
#[must_use]
pub fn library_emby_list(list: Option<&GuiLibraryList>) -> Option<&GuiEmbyList> {
    match list.map(|list| &list.destination) {
        Some(GuiDestinationList::Emby(destination)) => Some(destination),
        _ => None,
    }
}

/// The active destination's detail part: the plain Emby-library detail part,
/// or `None` while another destination is active.
#[must_use]
pub fn library_emby_detail(detail: Option<&GuiLibraryDetail>) -> Option<&GuiEmbyDetail> {
    match detail {
        Some(GuiLibraryDetail::Emby(detail)) => Some(detail),
        _ => None,
    }
}

/// The active destination's list part: the Audiobooks list part, or `None`
/// while another destination is active.
#[must_use]
pub fn library_audiobooks_list(list: Option<&GuiLibraryList>) -> Option<&GuiAudiobooksList> {
    match list.map(|list| &list.destination) {
        Some(GuiDestinationList::Audiobooks(destination)) => Some(destination),
        _ => None,
    }
}

/// The active destination's detail part: the Audiobooks detail part, or `None`
/// while another destination is active.
#[must_use]
pub fn library_audiobooks_detail(
    detail: Option<&GuiLibraryDetail>,
) -> Option<&GuiAudiobooksDetail> {
    match detail {
        Some(GuiLibraryDetail::Audiobooks(detail)) => Some(detail),
        _ => None,
    }
}

/// The active destination's list part: the Podcasts list part, or `None`
/// while another destination is active.
#[must_use]
pub fn library_podcasts_list(list: Option<&GuiLibraryList>) -> Option<&GuiPodcastsList> {
    match list.map(|list| &list.destination) {
        Some(GuiDestinationList::Podcasts(destination)) => Some(destination),
        _ => None,
    }
}

/// The active destination's detail part: the Podcasts detail part, or `None`
/// while another destination is active.
#[must_use]
pub fn library_podcasts_detail(detail: Option<&GuiLibraryDetail>) -> Option<&GuiPodcastsDetail> {
    match detail {
        Some(GuiLibraryDetail::Podcasts(detail)) => Some(detail),
        _ => None,
    }
}

/// The active destination's list part: the Feeds list part, or `None` while
/// another destination is active.
#[must_use]
pub fn library_feeds_list(list: Option<&GuiLibraryList>) -> Option<&GuiFeedsList> {
    match list.map(|list| &list.destination) {
        Some(GuiDestinationList::Feeds(destination)) => Some(destination),
        _ => None,
    }
}

/// The active destination's detail part: the Feeds detail part, or `None`
/// while another destination is active.
#[must_use]
pub fn library_feeds_detail(detail: Option<&GuiLibraryDetail>) -> Option<&GuiFeedsDetail> {
    match detail {
        Some(GuiLibraryDetail::Feeds(detail)) => Some(detail),
        _ => None,
    }
}

/// One destination's list part and detail part, narrowed together on the
/// destination's kind: the destination's painters take both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryPair<'a> {
    /// The Music destination's parts.
    Music(&'a GuiMusicList, &'a GuiMusicDetail),
    /// The Home destination's parts.
    Home(&'a GuiHomeList, &'a GuiHomeDetail),
    /// The Shows destination's parts.
    Shows(&'a GuiShowsList, &'a GuiShowsDetail),
    /// The plain Emby-library destination's parts.
    Emby(&'a GuiEmbyList, &'a GuiEmbyDetail),
    /// The Audiobooks destination's parts.
    Audiobooks(&'a GuiAudiobooksList, &'a GuiAudiobooksDetail),
    /// The Podcasts destination's parts.
    Podcasts(&'a GuiPodcastsList, &'a GuiPodcastsDetail),
    /// The Feeds destination's parts.
    Feeds(&'a GuiFeedsList, &'a GuiFeedsDetail),
}

impl<'a> LibraryPair<'a> {
    /// The destination's Favourites list part (emby-favourites design D6), or
    /// `None` while the destination's view is not Favourites: the list the
    /// panel paints in place of the destination's own, with its own pick and
    /// Hero.
    #[must_use]
    pub fn favourites(self) -> Option<&'a GuiFavouritesList> {
        match self {
            Self::Music(list, _) => list.favourites.as_ref(),
            Self::Shows(list, _) => list.favourites.as_ref(),
            Self::Emby(list, _) => list.favourites.as_ref(),
            Self::Home(..) | Self::Audiobooks(..) | Self::Podcasts(..) | Self::Feeds(..) => None,
        }
    }
}

/// The pair of the two sections' parts, or `None` while either is absent or
/// the two name different destinations.
#[must_use]
pub fn library_pair<'a>(
    list: Option<&'a GuiLibraryList>,
    detail: Option<&'a GuiLibraryDetail>,
) -> Option<LibraryPair<'a>> {
    let (Some(list), Some(detail)) = (list, detail) else {
        return None;
    };
    match (&list.destination, detail) {
        (GuiDestinationList::Music(list), GuiLibraryDetail::Music(detail)) => {
            Some(LibraryPair::Music(list, detail))
        }
        (GuiDestinationList::Home(list), GuiLibraryDetail::Home(detail)) => {
            Some(LibraryPair::Home(list, detail))
        }
        (GuiDestinationList::Shows(list), GuiLibraryDetail::Shows(detail)) => {
            Some(LibraryPair::Shows(list, detail))
        }
        (GuiDestinationList::Emby(list), GuiLibraryDetail::Emby(detail)) => {
            Some(LibraryPair::Emby(list, detail))
        }
        (GuiDestinationList::Audiobooks(list), GuiLibraryDetail::Audiobooks(detail)) => {
            Some(LibraryPair::Audiobooks(list, detail))
        }
        (GuiDestinationList::Podcasts(list), GuiLibraryDetail::Podcasts(detail)) => {
            Some(LibraryPair::Podcasts(list, detail))
        }
        (GuiDestinationList::Feeds(list), GuiLibraryDetail::Feeds(detail)) => {
            Some(LibraryPair::Feeds(list, detail))
        }
        _ => None,
    }
}
