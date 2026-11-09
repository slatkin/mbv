//! The `library` command's wire form (add-gui-library-panel design D2): the
//! Library panel's window content sends one semantic request, and the neutral
//! [`mbv::GuiLibraryCommand`] this module builds from it is what the Client
//! core applies. Item identities are stable Service ids, never row positions,
//! so a command built from a stale frame addresses the item that frame painted
//! or nothing.

use mbv_queue::QueueItemContentId;
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
    /// Select the one collection group of the Grouped Music library `libIdx`.
    SelectCollection { lib_idx: usize, group_id: String },
    /// Show the item's detail in the Hero.
    ShowDetail { item_id: String },
    /// Open the album node: load its tracks when the cache does not hold
    /// them.
    OpenAlbum { album_id: String },
    /// Take Panel focus for the Library panel.
    TakeFocus,
    /// Take Panel focus for the Queue panel: Tab across the panel boundary's
    /// far end (design D3, D4). The mirror of [`GuiLibraryCommand::TakeFocus`].
    TakeQueueFocus,
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
    /// Carry the item's full menu in the frame (the right-click item menu).
    RowMenu { item_id: String },
    /// Select a Home row (add-gui-library-panel design D12): the row the
    /// Hero shows. The core keeps the selection, so the frame paints it.
    HomeSelect { row: GuiHomeRowId },
    /// Play a Home row (design D12).
    HomePlay { row: GuiHomeRowId },
    /// Enqueue a Home row after the active slot (design D12): the Owner
    /// owns the queue, and the row resolves through the same Home enqueue
    /// path the add carries.
    HomePlayNext { row: GuiHomeRowId },
    /// Enqueue a Home row (design D12).
    HomeQueue { row: GuiHomeRowId },
}

/// One Home row's stable identity (design D12): the identity the Home
/// paths resolve, addressed by Service ids and never by a row position.
/// `Recent` carries the queue item's content id, whose own serde form is
/// itself (`provider` and `value`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum GuiHomeRowId {
    /// A `Continue watching` row's Emby item id.
    Continue(String),
    /// A `Recently added` row's queue-item content id.
    Recent(QueueItemContentId),
}

impl From<GuiLibraryCommand> for mbv::GuiLibraryCommand {
    fn from(command: GuiLibraryCommand) -> Self {
        match command {
            GuiLibraryCommand::OpenDestination { position } => Self::OpenDestination { position },
            GuiLibraryCommand::SelectCollection { lib_idx, group_id } => {
                Self::SelectCollection { lib_idx, group_id }
            }
            GuiLibraryCommand::ShowDetail { item_id } => Self::ShowDetail { item_id },
            GuiLibraryCommand::OpenAlbum { album_id } => Self::OpenAlbum { album_id },
            GuiLibraryCommand::TakeFocus => Self::TakeFocus,
            GuiLibraryCommand::TakeQueueFocus => Self::TakeQueueFocus,
            GuiLibraryCommand::Play { item_id } => Self::Play { item_id },
            GuiLibraryCommand::PlayNext { item_id } => Self::PlayNext { item_id },
            GuiLibraryCommand::AddToQueue { item_id } => Self::AddToQueue { item_id },
            GuiLibraryCommand::ShuffleArtist { album_ids } => Self::ShuffleArtist { album_ids },
            GuiLibraryCommand::MenuEntry { item_id, index } => Self::MenuEntry { item_id, index },
            GuiLibraryCommand::RowMenu { item_id } => Self::RowMenu { item_id },
            GuiLibraryCommand::HomeSelect { row } => Self::HomeSelect {
                row: mbv::GuiHomeRowId::from(&row),
            },
            GuiLibraryCommand::HomePlay { row } => Self::HomePlay {
                row: mbv::GuiHomeRowId::from(&row),
            },
            GuiLibraryCommand::HomePlayNext { row } => Self::HomePlayNext {
                row: mbv::GuiHomeRowId::from(&row),
            },
            GuiLibraryCommand::HomeQueue { row } => Self::HomeQueue {
                row: mbv::GuiHomeRowId::from(&row),
            },
        }
    }
}

/// One row id's core identity, resolving the wire type the front end sends.
/// The frame mirrors the same shape on the way out, so a command built from
/// a carried row addresses exactly that row.
impl From<&GuiHomeRowId> for mbv::GuiHomeRowId {
    fn from(row: &GuiHomeRowId) -> Self {
        match row {
            GuiHomeRowId::Continue(item_id) => Self::Continue(item_id.clone()),
            GuiHomeRowId::Recent(content_id) => Self::Recent(content_id.clone()),
        }
    }
}
