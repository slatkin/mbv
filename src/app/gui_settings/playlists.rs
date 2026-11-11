//! The Sessions and Playlists section commands (`add-gui-settings-screen`
//! design D5, row 5.5): target selection, then the playlist actions through
//! the shared `App` methods the TUI's sidebar calls — addressed by playlist id
//! and item id, never by a row index.

use mbv_ui_model::panel_targets::ChooserTargetKey;
use mbv_ui_msg::MusicTreeAction;

use crate::app::App;

impl App {
    /// Row 5.5's `selectTarget` arm (design D5): select the playback target
    /// through the chooser's own path, leaving the screen open (the spec's
    /// *Sessions lists and selects the playback targets*).
    pub(super) fn apply_gui_settings_select_target(&mut self, target: &ChooserTargetKey) {
        if !self.gui_settings_is_open() {
            Self::log_ignored_gui_settings_command("selectTarget", "the Settings screen is closed");
            return;
        }
        self.select_chooser_target(target);
    }

    /// Row 5.5's `playlistOpen` arm: open the playlist `id` names to its
    /// items, or return the list view and load it for no id.
    pub(super) fn apply_gui_settings_playlist_open(&mut self, id: Option<&str>) {
        if !self.gui_settings_is_open() {
            Self::log_ignored_gui_settings_command("playlistOpen", "the Settings screen is closed");
            return;
        }
        if let Some(id) = id {
            self.open_gui_settings_playlist(id);
            return;
        }
        self.playlists_open = None;
        self.playlists_open_items.clear();
        self.spawn_load_playlists();
    }

    /// Row 5.5's `playlistBack` arm: return from the open playlist to the
    /// list, as the TUI's Back does.
    pub(super) fn apply_gui_settings_playlist_back(&mut self) {
        if !self.gui_settings_is_open() {
            Self::log_ignored_gui_settings_command("playlistBack", "the Settings screen is closed");
            return;
        }
        self.playlists_open = None;
        self.playlists_open_items.clear();
    }

    /// Row 5.5's `playlistRefresh` arm: reload the playlist `id` names, or the
    /// open one — and with neither open, the list — as the TUI's Refresh does.
    pub(super) fn apply_gui_settings_playlist_refresh(&mut self, id: Option<&str>) {
        if !self.gui_settings_is_open() {
            Self::log_ignored_gui_settings_command(
                "playlistRefresh",
                "the Settings screen is closed",
            );
            return;
        }
        if let Some(id) = id {
            self.open_gui_settings_playlist(id);
            return;
        }
        match self.playlists_open.clone() {
            Some(playlist) => {
                self.playlists_open = None;
                self.playlists_open_items.clear();
                self.spawn_open_playlist(&playlist);
            }
            None => self.spawn_load_playlists(),
        }
    }

    /// Row 5.5's `playlistAction` arm: play, shuffle, or enqueue a whole
    /// playlist through the shared `App::playlist_action`.
    pub(super) fn apply_gui_settings_playlist_action(&mut self, id: &str, action: MusicTreeAction) {
        if !self.gui_settings_is_open() {
            Self::log_ignored_gui_settings_command(
                "playlistAction",
                "the Settings screen is closed",
            );
            return;
        }
        self.playlist_action(id, action);
    }

    /// Row 5.5's `playlistItemAction` arm: play, shuffle, or enqueue one item
    /// of the open playlist. A command naming another playlist than the open
    /// one is stale and changes nothing.
    pub(super) fn apply_gui_settings_playlist_item_action(
        &mut self,
        id: &str,
        item_id: &str,
        action: MusicTreeAction,
    ) {
        if !self.gui_settings_is_open() {
            Self::log_ignored_gui_settings_command(
                "playlistItemAction",
                "the Settings screen is closed",
            );
            return;
        }
        if self
            .playlists_open
            .as_ref()
            .is_none_or(|playlist| playlist.id != id)
        {
            Self::log_ignored_gui_settings_command(
                "playlistItemAction",
                "the open playlist is another one",
            );
            return;
        }
        self.playlist_row_action(item_id, action);
    }

    /// Row 5.5's `playlistRename` arm: rename the playlist on the server
    /// through the shared spawn; the landed list reload repaints the rows.
    pub(super) fn apply_gui_settings_playlist_rename(&mut self, id: &str, name: &str) {
        if !self.gui_settings_is_open() {
            Self::log_ignored_gui_settings_command(
                "playlistRename",
                "the Settings screen is closed",
            );
            return;
        }
        let name = name.trim().to_string();
        if name.is_empty() {
            self.flash(
                "Playlist name can't be empty".into(),
                crate::app::dispatch::notify::ToastSeverity::Error,
            );
            return;
        }
        if !self.playlists.iter().any(|playlist| playlist.id == id) {
            Self::log_ignored_gui_settings_command(
                "playlistRename",
                "the playlist is not in the saved list",
            );
            return;
        }
        self.spawn_rename_playlist(id.to_string(), name);
    }

    /// Row 5.5's `playlistDelete` arm: raise the same delete confirmation the
    /// TUI's sidebar raises.
    pub(super) fn apply_gui_settings_playlist_delete(&mut self, id: &str) {
        if !self.gui_settings_is_open() {
            Self::log_ignored_gui_settings_command(
                "playlistDelete",
                "the Settings screen is closed",
            );
            return;
        }
        let Some(playlist) = self
            .playlists
            .iter()
            .find(|playlist| playlist.id == id)
            .cloned()
        else {
            Self::log_ignored_gui_settings_command(
                "playlistDelete",
                "the playlist is not in the saved list",
            );
            return;
        };
        self.ask_confirm(crate::app::ConfirmModal::two_button(
            format!(
                "Delete playlist '{}'?",
                mbv_ui_model::ui_util::trunc_str(&playlist.name, 40)
            ),
            "Confirm",
            "Cancel",
            crate::app::ConfirmAction::DeletePlaylist {
                id: playlist.id,
                name: playlist.name,
            },
        ));
    }

    /// Open the saved playlist `id` names, or log the stale command when the
    /// list no longer holds it.
    fn open_gui_settings_playlist(&mut self, id: &str) {
        let Some(playlist) = self
            .playlists
            .iter()
            .find(|playlist| playlist.id == id)
            .cloned()
        else {
            Self::log_ignored_gui_settings_command(
                "playlistOpen",
                "the playlist is not in the saved list",
            );
            return;
        };
        self.spawn_open_playlist(&playlist);
    }
}
