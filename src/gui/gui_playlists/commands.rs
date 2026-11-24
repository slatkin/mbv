//! The Playlists screen's commands: each runs through the shared `App`
//! methods the TUI's sidebar calls — addressed by playlist id and item id,
//! never by a row index.

use mbv_ui_msg::MusicTreeAction;

use super::GuiPlaylistsCommand;
use crate::gui::GuiCore;

impl GuiCore {
    /// Apply one Playlists command: each arm calls the shared `App` action the
    /// TUI's sidebar calls. A command built from a stale frame logs its receipt
    /// at debug level and changes nothing.
    pub fn apply_gui_playlists_command(&mut self, command: GuiPlaylistsCommand) {
        tracing::debug!(
            name: "gui_playlists.command",
            target: "gui_playlists",
            command = ?command,
            "the GUI Playlists screen sent a command"
        );
        if !matches!(
            command,
            GuiPlaylistsCommand::Toggle | GuiPlaylistsCommand::Close
        ) && !self.gui.playlists_screen_open
        {
            Self::log_ignored_gui_playlists_command("stale", "the Playlists screen is closed");
            return;
        }
        match command {
            GuiPlaylistsCommand::Toggle => self.toggle_gui_playlists(),
            GuiPlaylistsCommand::Close => self.close_gui_playlists(),
            GuiPlaylistsCommand::Open { id } => self.open_gui_playlist(&id),
            GuiPlaylistsCommand::Collapse => self.app.close_open_playlist(),
            GuiPlaylistsCommand::Refresh { id } => self.refresh_gui_playlists(id.as_deref()),
            GuiPlaylistsCommand::Action { id, action } => self.app.playlist_action(&id, action),
            GuiPlaylistsCommand::ItemAction {
                id,
                item_id,
                action,
            } => self.apply_gui_playlist_item_action(&id, &item_id, action),
            GuiPlaylistsCommand::Rename { id, name } => self.apply_gui_playlist_rename(&id, &name),
            GuiPlaylistsCommand::Delete { id } => self.apply_gui_playlist_delete(&id),
        }
    }

    /// Reload the playlist `id` names, or the open one — and with neither
    /// open, the list.
    fn refresh_gui_playlists(&mut self, id: Option<&str>) {
        if let Some(id) = id {
            self.open_gui_playlist(id);
            return;
        }
        match self.app.playlists_open().cloned() {
            Some(playlist) => {
                self.app.close_open_playlist();
                self.app.spawn_open_playlist(&playlist);
            }
            None => self.app.spawn_load_playlists(),
        }
    }

    /// Play, shuffle, or enqueue one item of the open playlist. A command
    /// naming another playlist than the open one is stale and changes nothing.
    fn apply_gui_playlist_item_action(&mut self, id: &str, item_id: &str, action: MusicTreeAction) {
        if self
            .app
            .playlists_open()
            .is_none_or(|playlist| playlist.id != id)
        {
            Self::log_ignored_gui_playlists_command(
                "itemAction",
                "the open playlist is another one",
            );
            return;
        }
        self.app.playlist_row_action(item_id, action);
    }

    /// Rename the playlist on the server through the shared spawn; the landed
    /// list reload repaints the rows.
    fn apply_gui_playlist_rename(&mut self, id: &str, name: &str) {
        let name = name.trim().to_string();
        if name.is_empty() {
            self.app.flash(
                "Playlist name can't be empty".into(),
                mbv_client_core::dispatch::notify::ToastSeverity::Error,
            );
            return;
        }
        if !self
            .app
            .playlists()
            .iter()
            .any(|playlist| playlist.id == id)
        {
            Self::log_ignored_gui_playlists_command(
                "rename",
                "the playlist is not in the saved list",
            );
            return;
        }
        self.app.spawn_rename_playlist(id.to_string(), name);
    }

    /// Raise the same delete confirmation the TUI's sidebar raises.
    fn apply_gui_playlist_delete(&mut self, id: &str) {
        let Some(playlist) = self
            .app
            .playlists()
            .iter()
            .find(|playlist| playlist.id == id)
            .cloned()
        else {
            Self::log_ignored_gui_playlists_command(
                "delete",
                "the playlist is not in the saved list",
            );
            return;
        };
        self.app
            .ask_confirm(mbv_ui_model::confirm::ConfirmModal::two_button(
                format!(
                    "Delete playlist '{}'?",
                    mbv_ui_model::ui_util::trunc_str(&playlist.name, 40)
                ),
                "Confirm",
                "Cancel",
                mbv_ui_model::confirm::ConfirmAction::DeletePlaylist {
                    id: playlist.id,
                    name: playlist.name,
                },
            ));
    }

    /// Expand the saved playlist `id` names, or log the stale command when the
    /// list no longer holds it.
    fn open_gui_playlist(&mut self, id: &str) {
        let Some(playlist) = self
            .app
            .playlists()
            .iter()
            .find(|playlist| playlist.id == id)
            .cloned()
        else {
            Self::log_ignored_gui_playlists_command(
                "open",
                "the playlist is not in the saved list",
            );
            return;
        };
        self.app.spawn_open_playlist(&playlist);
    }
}
