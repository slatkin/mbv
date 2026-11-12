//! The Library section (add-gui-library-panel design D2): the active
//! destination's content, carried whole only when its revision changes,
//! with the Hero's artwork as the `GuiArtRef` image references the window
//! paints (design D7).
//!
//! A Library list can hold thousands of rows, so the section carries a
//! revision and a rebuild re-carries the content only when that revision
//! changes; the front end keeps the last carried value. The art is part of the
//! content, so a landed image bumps the revision and rides the same batch as
//! the facts it belongs to.
//!
//! Each destination splits into a list part and a detail part
//! (`gui-change-driven-sections` design D8). `GuiLibraryList` and
//! `GuiLibraryDetail` (in `list` and `detail`) hold the split. `GuiLibraryContent`
//! below carries the same parts flattened until task 6.3 replaces it.

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
    GuiAudiobooksChapterRowFrame, GuiAudiobooksFrame, GuiAudiobooksHeroFrame,
    GuiAudiobooksRowFrame, GuiAudiobooksWorkspaceFrame,
};
pub use self::detail::{GuiDestinationDetail, GuiLibraryDetail};
pub use self::emby::{
    GuiEmbyFeedGroupFrame, GuiEmbyFrame, GuiEmbyHeroCreditFrame, GuiEmbyHeroFrame,
    GuiEmbyPillFrame, GuiEmbyRowFrame, GuiEmbyRowKindFrame,
};
pub use self::entry_list::{GuiEntryRowFrame, GuiPillFrame};
pub use self::feeds::{GuiFeedsFrame, GuiFeedsHeroFrame};
pub use self::home::{
    GuiHomeDetail, GuiHomeFrame, GuiHomeHeroFrame, GuiHomeItemRowFrame, GuiHomeList,
    GuiHomeRowFrame, GuiHomeRowIdFrame, GuiHomeRowKindFrame, GuiHomeRowStateFrame,
};
pub use self::list::{GuiDestinationList, GuiLibraryList};
pub use self::meta::{GuiMetaKindFrame, GuiMetaSegmentFrame, gui_meta_rows};
pub use self::music::{
    GuiArtistKey, GuiMusicAlbumFrame, GuiMusicArtistFrame, GuiMusicDetail, GuiMusicFrame,
    GuiMusicHeroFrame, GuiMusicList, GuiMusicPillFrame, GuiMusicTrackFrame,
    GuiMusicTrackTotalFrame,
};
pub use self::podcasts::{GuiPodcastsFrame, GuiPodcastsHeroFrame};
pub use self::search::{GuiSearchFrame, GuiSearchRowFrame};
pub use self::shows::{
    GuiShowsEpisodeRowFrame, GuiShowsFrame, GuiShowsHeroFrame, GuiShowsModeFrame,
    GuiShowsPillFrame, GuiShowsRowFrame, GuiShowsSeasonPillFrame, GuiShowsWorkspaceFrame,
};

/// The content of the Library panel (design D2): the destination tabs the
/// top bar paints, and the active destination's pills, list rows, Hero facts,
/// and Hero artwork.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiLibraryContent {
    /// The presented destination tabs in catalog order (design D2): one per
    /// Grouped Music library, phase 1's presented set. The top bar paints
    /// only these — no stub for a destination the GUI does not present yet.
    pub tabs: Vec<GuiLibraryTabFrame>,
    /// The active tab's position while the active destination is one of the
    /// tabs (design D2): the tab the top bar marks current. `None` for Home,
    /// Feeds, and every destination the GUI does not present yet.
    pub current_tab: Option<usize>,
    /// The Grouped Music library's content while Music is the active
    /// destination; `None` for a destination that presents no content yet.
    pub music: Option<GuiMusicFrame>,
    /// The Home destination's content while Home is the active destination
    /// (add-gui-library-panel design D12, the USER-APPROVED GUI Home
    /// design); `None` while any other destination is active. The top bar
    /// marks the Home icon current exactly while this field is present.
    pub home: Option<GuiHomeFrame>,
    /// The Shows destination's content while a Shows library is the active
    /// destination (add-gui-library-panel design D12, the USER-APPROVED GUI
    /// Shows design); `None` while any other destination is active.
    pub shows: Option<GuiShowsFrame>,
    /// The plain Emby-library destination's content while a plain Emby
    /// library is the active destination (add-gui-library-panel design D12,
    /// row 8.1, the USER-APPROVED GUI Emby-library design); `None` while any
    /// other destination is active.
    pub emby: Option<GuiEmbyFrame>,
    /// The right-click item menu the window content last opened (the row-menu
    /// command), whichever destination's row it named (design D8, D15): the
    /// item it named with the TUI menu builder's full entries for it. `None`
    /// while no right-click has named an item. It rides the content, not a
    /// destination's section, because every destination's rows open the same
    /// menu.
    pub row_menu: Option<GuiLibraryRowMenuFrame>,
    /// The Audiobooks destination's content while an Audiobookshelf book
    /// library is the active destination (add-gui-library-panel design D12,
    /// row 9.1); `None` while any other destination is active.
    pub audiobooks: Option<GuiAudiobooksFrame>,
    /// The Podcasts destination's content while an Audiobookshelf podcast
    /// library is the active destination (add-gui-library-panel design D12,
    /// row 10.1); `None` while any other destination is active.
    pub podcasts: Option<GuiPodcastsFrame>,
    /// The Feeds destination's content while Feeds is the active destination
    /// (add-gui-library-panel design D12, row 11.1); `None` while any other
    /// destination is active. A Client holds one feed tab, so no library index
    /// names it.
    pub feeds: Option<GuiFeedsFrame>,
}

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
