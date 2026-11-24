//! The Playlists screen's section: the saved playlists with their item
//! counts, the open playlist and its items, and the two loading flags.

use crate::gui_core::{
    GuiCore, GuiOpenPlaylistProjection, GuiPlaylistItemProjection, GuiPlaylistRowProjection,
};

/// The Playlists section value the Client core's open screen projects, `None`
/// while the screen is closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiPlaylistsFrame {
    pub playlists: Vec<GuiPlaylistRow>,
    pub open: Option<GuiOpenPlaylist>,
    pub loading: bool,
    pub open_loading: bool,
}

impl GuiPlaylistsFrame {
    #[must_use]
    pub fn build(core: &GuiCore) -> Option<Self> {
        let projection = core.gui_playlists()?;
        Some(Self {
            playlists: projection
                .playlists
                .iter()
                .map(GuiPlaylistRow::from)
                .collect(),
            open: projection.open.as_ref().map(GuiOpenPlaylist::from),
            loading: projection.loading,
            open_loading: projection.open_loading,
        })
    }
}

/// One saved playlist (design D9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiPlaylistRow {
    /// The playlist's stable Emby id: every playlist command addresses it.
    pub id: String,
    pub name: String,
    pub item_count: u32,
}

impl From<&GuiPlaylistRowProjection> for GuiPlaylistRow {
    fn from(row: &GuiPlaylistRowProjection) -> Self {
        Self {
            id: row.id.clone(),
            name: row.name.clone(),
            item_count: row.item_count,
        }
    }
}

/// The open playlist and its items (design D9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiOpenPlaylist {
    pub id: String,
    pub name: String,
    pub items: Vec<GuiPlaylistItem>,
}

impl From<&GuiOpenPlaylistProjection> for GuiOpenPlaylist {
    fn from(playlist: &GuiOpenPlaylistProjection) -> Self {
        Self {
            id: playlist.id.clone(),
            name: playlist.name.clone(),
            items: playlist.items.iter().map(GuiPlaylistItem::from).collect(),
        }
    }
}

/// One item of the open playlist (design D9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiPlaylistItem {
    pub id: String,
    pub name: String,
}

impl From<&GuiPlaylistItemProjection> for GuiPlaylistItem {
    fn from(item: &GuiPlaylistItemProjection) -> Self {
        Self {
            id: item.id.clone(),
            name: item.name.clone(),
        }
    }
}
