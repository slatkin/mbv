//! The Playlists section's content (`add-gui-settings-screen` design D9):
//! the saved playlists with their item counts, the open playlist and its items,
//! and the two loading flags.

use mbv::{
    GuiSettingsOpenPlaylistProjection, GuiSettingsPlaylistItemProjection,
    GuiSettingsPlaylistRowProjection,
};
use serde::Serialize;

/// One saved playlist (design D9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiPlaylistRow {
    /// The playlist's stable Emby id: every playlist command addresses it.
    pub id: String,
    pub name: String,
    pub item_count: u32,
}

impl From<&GuiSettingsPlaylistRowProjection> for GuiPlaylistRow {
    fn from(row: &GuiSettingsPlaylistRowProjection) -> Self {
        Self {
            id: row.id.clone(),
            name: row.name.clone(),
            item_count: row.item_count,
        }
    }
}

/// The open playlist and its items (design D9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiOpenPlaylist {
    pub id: String,
    pub name: String,
    pub items: Vec<GuiPlaylistItem>,
}

impl From<&GuiSettingsOpenPlaylistProjection> for GuiOpenPlaylist {
    fn from(playlist: &GuiSettingsOpenPlaylistProjection) -> Self {
        Self {
            id: playlist.id.clone(),
            name: playlist.name.clone(),
            items: playlist.items.iter().map(GuiPlaylistItem::from).collect(),
        }
    }
}

/// One item of the open playlist (design D9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiPlaylistItem {
    pub id: String,
    pub name: String,
}

impl From<&GuiSettingsPlaylistItemProjection> for GuiPlaylistItem {
    fn from(item: &GuiSettingsPlaylistItemProjection) -> Self {
        Self {
            id: item.id.clone(),
            name: item.name.clone(),
        }
    }
}
