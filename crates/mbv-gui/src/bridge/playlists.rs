//! The `playlists` command's wire form: the Playlists screen's window content
//! sends one semantic request, and the neutral [`crate::gui_core::GuiPlaylistsCommand`]
//! this module builds from it is what the Client core applies. Identities are
//! stable playlist and item ids, never row positions.

use mbv_ui_msg::MusicTreeAction;

/// The playlist operation a playlist command carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiPlaylistAction {
    Play,
    Shuffle,
    Enqueue,
}

impl From<GuiPlaylistAction> for MusicTreeAction {
    fn from(action: GuiPlaylistAction) -> Self {
        match action {
            GuiPlaylistAction::Play => Self::Play,
            GuiPlaylistAction::Shuffle => Self::Shuffle,
            GuiPlaylistAction::Enqueue => Self::Enqueue,
        }
    }
}

/// One request from the Playlists screen's window content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuiPlaylistsCommand {
    /// Open the screen, and close it while it shows (the top bar's control).
    Toggle,
    /// Close the screen.
    Close,
    /// Expand a playlist to its items.
    Open { id: String },
    /// Collapse the open playlist.
    Collapse,
    /// Refresh the open playlist, or the one `id` names.
    Refresh { id: Option<String> },
    /// Play, shuffle, or enqueue a whole playlist.
    Action {
        id: String,
        action: GuiPlaylistAction,
    },
    /// Play, shuffle, or enqueue one item of the open playlist.
    ItemAction {
        id: String,
        item_id: String,
        action: GuiPlaylistAction,
    },
    /// Rename a playlist.
    Rename { id: String, name: String },
    /// Raise the delete confirmation for a playlist.
    Delete { id: String },
}

/// Translate the wire form into the neutral command.
#[must_use]
pub fn neutral(command: GuiPlaylistsCommand) -> crate::gui_core::GuiPlaylistsCommand {
    match command {
        GuiPlaylistsCommand::Toggle => crate::gui_core::GuiPlaylistsCommand::Toggle,
        GuiPlaylistsCommand::Close => crate::gui_core::GuiPlaylistsCommand::Close,
        GuiPlaylistsCommand::Open { id } => crate::gui_core::GuiPlaylistsCommand::Open { id },
        GuiPlaylistsCommand::Collapse => crate::gui_core::GuiPlaylistsCommand::Collapse,
        GuiPlaylistsCommand::Refresh { id } => crate::gui_core::GuiPlaylistsCommand::Refresh { id },
        GuiPlaylistsCommand::Action { id, action } => {
            crate::gui_core::GuiPlaylistsCommand::Action {
                id,
                action: action.into(),
            }
        }
        GuiPlaylistsCommand::ItemAction {
            id,
            item_id,
            action,
        } => crate::gui_core::GuiPlaylistsCommand::ItemAction {
            id,
            item_id,
            action: action.into(),
        },
        GuiPlaylistsCommand::Rename { id, name } => {
            crate::gui_core::GuiPlaylistsCommand::Rename { id, name }
        }
        GuiPlaylistsCommand::Delete { id } => crate::gui_core::GuiPlaylistsCommand::Delete { id },
    }
}
