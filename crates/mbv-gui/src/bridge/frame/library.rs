//! The Library sections (add-gui-library-panel design D2, split by
//! `gui-change-driven-sections` design D8): `libraryList` (the destination tabs
//! and the active destination's list part), `libraryDetail` (its detail part:
//! the selection, the Hero, the Workspace, and the playing marks), and
//! `rowMenu` (the right-click item menu). Each section keeps its own revision
//! and is sent only when its value changes. The Hero's artwork rides as the
//! `GuiArtRef` image references the window paints (design D7).
//!
//! A Library list can hold thousands of rows, so the window keeps each section
//! in its own store. The art is part of its section's value, so a landed image
//! bumps that section's revision.

use mbv::{GuiArtShape, GuiLibraryArtwork, GuiLibraryMenuItem, GuiLibraryTab};
use serde::Serialize;

use crate::art::{GuiArtRef, PLACEHOLDER_PATH, art_url, card_path};

mod audiobooks;
mod detail;
mod emby;
mod entry_list;
mod feeds;
mod home;
mod list;
mod meta;
mod music;
mod podcasts;
mod search;
mod shows;

pub use self::audiobooks::{
    GuiAudiobooksChapterRowFrame, GuiAudiobooksDetail, GuiAudiobooksHeroFrame, GuiAudiobooksList,
    GuiAudiobooksRowFrame, GuiAudiobooksWorkspaceFrame,
};
pub use self::detail::GuiLibraryDetail;
pub use self::emby::{
    GuiEmbyDetail, GuiEmbyFeedGroupFrame, GuiEmbyHeroCreditFrame, GuiEmbyHeroFrame, GuiEmbyList,
    GuiEmbyPillFrame, GuiEmbyRowFrame, GuiEmbyRowKindFrame,
};
pub use self::entry_list::{GuiEntryRowFrame, GuiPillFrame};
pub use self::feeds::{GuiFeedsDetail, GuiFeedsHeroFrame, GuiFeedsList};
pub use self::home::{
    GuiHomeDetail, GuiHomeHeroFrame, GuiHomeItemRowFrame, GuiHomeList, GuiHomeRowFrame,
    GuiHomeRowIdFrame, GuiHomeRowKindFrame, GuiHomeRowStateFrame,
};
pub use self::list::{GuiDestinationList, GuiLibraryList};
pub use self::meta::{GuiMetaKindFrame, GuiMetaSegmentFrame, gui_meta_rows};
pub use self::music::{
    GuiArtistKey, GuiMusicAlbumFrame, GuiMusicArtistFrame, GuiMusicDetail, GuiMusicHeroFrame,
    GuiMusicList, GuiMusicPillFrame, GuiMusicTrackFrame, GuiMusicTrackTotalFrame,
};
pub use self::podcasts::{GuiPodcastsDetail, GuiPodcastsHeroFrame, GuiPodcastsList};
pub use self::search::{GuiSearchFrame, GuiSearchRowFrame};
pub use self::shows::{
    GuiShowsDetail, GuiShowsEpisodeRowFrame, GuiShowsHeroFrame, GuiShowsList, GuiShowsModeFrame,
    GuiShowsPillFrame, GuiShowsRowFrame, GuiShowsSeasonPillFrame, GuiShowsWorkspaceFrame,
};

/// One destination tab of the top bar (design D2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiLibraryTabFrame {
    /// The tab position the open-destination command takes, where 0 is Home.
    pub position: usize,
    /// The tab's label: the library's display name.
    pub label: String,
}

impl From<GuiLibraryTab> for GuiLibraryTabFrame {
    fn from(tab: GuiLibraryTab) -> Self {
        Self {
            position: tab.position,
            label: tab.label,
        }
    }
}

/// The Library's right-click item menu (the row-menu command): the item the
/// entries are about, and the TUI menu builder's entries in the builder's
/// order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiLibraryRowMenuFrame {
    /// The item's stable Service id the command named.
    pub item_id: String,
    /// The TUI menu builder's entries for the item.
    pub entries: Vec<GuiMusicMenuEntryFrame>,
}

/// One entry of the picked item's menu, as the window paints it (design D8):
/// the label, the key hint the row menu shows when the entry has one, and the
/// entry's index in the shared builder's order — the index a chosen entry
/// sends back, so a front end that paints only some entries still executes
/// the one it painted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiMusicMenuEntryFrame {
    pub label: String,
    pub key_hint: Option<String>,
    pub index: usize,
}

impl From<&mbv::GuiLibraryRowMenu> for GuiLibraryRowMenuFrame {
    fn from(menu: &mbv::GuiLibraryRowMenu) -> Self {
        Self {
            item_id: menu.item_id.clone(),
            entries: menu
                .entries
                .iter()
                .map(GuiMusicMenuEntryFrame::from)
                .collect(),
        }
    }
}

impl From<&GuiLibraryMenuItem> for GuiMusicMenuEntryFrame {
    fn from(entry: &GuiLibraryMenuItem) -> Self {
        Self {
            label: entry.label.clone(),
            key_hint: entry.key_hint.clone(),
            index: entry.index,
        }
    }
}

impl From<&GuiLibraryArtwork> for GuiArtRef {
    fn from(artwork: &GuiLibraryArtwork) -> Self {
        match artwork {
            GuiLibraryArtwork::Image { key, .. } => Self::Image {
                src: art_url(&card_path(key)),
            },
            GuiLibraryArtwork::Placeholder => Self::Placeholder {
                src: art_url(PLACEHOLDER_PATH),
            },
            GuiLibraryArtwork::Loading => Self::Loading,
        }
    }
}

/// The Hero artwork's box shape, mirroring `mbv::GuiArtShape` (the artwork
/// policy's arms).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiArtShapeFrame {
    Landscape,
    Square,
    Portrait,
}

impl From<GuiArtShape> for GuiArtShapeFrame {
    fn from(shape: GuiArtShape) -> Self {
        match shape {
            GuiArtShape::Landscape => Self::Landscape,
            GuiArtShape::Square => Self::Square,
            GuiArtShape::Portrait => Self::Portrait,
        }
    }
}
