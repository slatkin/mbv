//! The `library` command's wire form (add-gui-library-panel design D2): the
//! Library panel's window content sends one semantic request, and the neutral
//! [`mbv::GuiLibraryCommand`] this module builds from it is what the Client
//! core applies. Item identities are stable Service ids, never row positions,
//! so a command built from a stale frame addresses the item that frame painted
//! or nothing.

use serde::Deserialize;

/// One request from the Library panel's window content (design D2). The
/// variants and their field names are the wire vocabulary the front end
/// sends; the translation into [`mbv::GuiLibraryCommand`] is the only place
/// the two shapes meet.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(
    tag = "command",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GuiLibraryCommand {
    /// Open the destination at `position`: the tab position the frame's
    /// destination tabs carry, where 0 is Home.
    OpenDestination { position: usize },
    /// Replace the selected collections of the Grouped Music library
    /// `libIdx` with `groupIds`, in pill order.
    SetCollections {
        lib_idx: usize,
        group_ids: Vec<String>,
    },
    /// Show the item's detail in the Hero.
    ShowDetail { item_id: String },
    /// Open the album node: load its tracks when the cache does not hold
    /// them.
    OpenAlbum { album_id: String },
    /// Take Panel focus for the Library panel.
    TakeFocus,
    /// Play the item.
    Play { item_id: String },
    /// Play the item next.
    PlayNext { item_id: String },
    /// Add the item to the queue.
    AddToQueue { item_id: String },
    /// Shuffle the artist whose albums are `albumIds`.
    ShuffleArtist { album_ids: Vec<String> },
    /// Run the item menu's entry at `index`.
    MenuEntry { item_id: String, index: usize },
}

impl From<GuiLibraryCommand> for mbv::GuiLibraryCommand {
    fn from(command: GuiLibraryCommand) -> Self {
        match command {
            GuiLibraryCommand::OpenDestination { position } => Self::OpenDestination { position },
            GuiLibraryCommand::SetCollections { lib_idx, group_ids } => {
                Self::SetCollections { lib_idx, group_ids }
            }
            GuiLibraryCommand::ShowDetail { item_id } => Self::ShowDetail { item_id },
            GuiLibraryCommand::OpenAlbum { album_id } => Self::OpenAlbum { album_id },
            GuiLibraryCommand::TakeFocus => Self::TakeFocus,
            GuiLibraryCommand::Play { item_id } => Self::Play { item_id },
            GuiLibraryCommand::PlayNext { item_id } => Self::PlayNext { item_id },
            GuiLibraryCommand::AddToQueue { item_id } => Self::AddToQueue { item_id },
            GuiLibraryCommand::ShuffleArtist { album_ids } => Self::ShuffleArtist { album_ids },
            GuiLibraryCommand::MenuEntry { item_id, index } => Self::MenuEntry { item_id, index },
        }
    }
}
