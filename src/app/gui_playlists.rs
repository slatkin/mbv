//! The GUI Playlists screen's Client-core state: whether the screen is open,
//! the semantic requests its window content sends, and the content the frame
//! carries. The screen is its own main-UI screen beside the Queue, not a
//! Settings section. The window content owns the screen's interaction state
//! (selection, right-click menu); the saved playlists and the open playlist's
//! items stay the App's own (`playlists`, `playlists_open`). The TUI Client
//! never sets any of it — its Playlists sidebar keeps its own shell state.

mod commands;

use mbv_emby_model::EmbyItem;

use crate::gui::GuiCore;

/// The Playlists screen's requests (window content → core). Identities are
/// stable playlist and item ids, never row positions, so a request built from a
/// stale window addresses what that window painted or nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuiPlaylistsCommand {
    /// Open the screen, and close it while it shows. The top bar's Playlists
    /// control sends it.
    Toggle,
    /// Close the screen.
    Close,
    /// Expand the playlist `id` names to its items (the tree's one open node).
    Open { id: String },
    /// Collapse the open playlist.
    Collapse,
    /// Reload the playlist `id` names, or the open one — and with neither open,
    /// the list.
    Refresh { id: Option<String> },
    /// Play, shuffle, or enqueue a whole playlist.
    Action {
        id: String,
        action: mbv_ui_msg::MusicTreeAction,
    },
    /// Play, shuffle, or enqueue one item of the open playlist.
    ItemAction {
        id: String,
        item_id: String,
        action: mbv_ui_msg::MusicTreeAction,
    },
    /// Rename a playlist on the server.
    Rename { id: String, name: String },
    /// Raise the delete confirmation for a playlist.
    Delete { id: String },
}

/// The Playlists screen's content: the saved list, the open playlist's items,
/// and the two loading flags.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiPlaylistsProjection {
    pub playlists: Vec<GuiPlaylistRowProjection>,
    pub open: Option<GuiOpenPlaylistProjection>,
    pub loading: bool,
    pub open_loading: bool,
}

/// One saved playlist: its stable id, name, and item count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiPlaylistRowProjection {
    pub id: String,
    pub name: String,
    pub item_count: u32,
}

/// The open playlist and its items.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiOpenPlaylistProjection {
    pub id: String,
    pub name: String,
    pub items: Vec<GuiPlaylistItemProjection>,
}

/// One item of the open playlist: its stable id and name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiPlaylistItemProjection {
    pub id: String,
    pub name: String,
}

impl GuiCore {
    /// Whether the Playlists screen is open: the fact the window's frame and
    /// the dismissal read.
    #[must_use]
    pub fn gui_playlists_is_open(&self) -> bool {
        self.app.gui.playlists_screen_open
    }

    /// Open the screen and load the saved playlists. The Settings screen and
    /// the Playlists screen never show together, so the open closes Settings.
    pub fn open_gui_playlists(&mut self) {
        // The Settings and Playlists screens never show together, so the open
        // closes Settings through the Settings screen's own entry.
        self.close_gui_settings();
        self.app.gui.playlists_screen_open = true;
        self.app.spawn_load_playlists();
    }

    /// Close the screen and drop the open playlist.
    pub fn close_gui_playlists(&mut self) {
        self.app.gui.playlists_screen_open = false;
        self.app.playlists_open = None;
        self.app.playlists_open_items.clear();
    }

    /// Whether the Playlists screen is open, for a Client's test that asserts a
    /// key or a drained request opened or closed it.
    #[doc(hidden)]
    #[must_use]
    pub fn gui_playlists_open_for_test(&self) -> bool {
        self.gui_playlists_is_open()
    }

    /// Open the screen when closed, close it when open.
    pub fn toggle_gui_playlists(&mut self) {
        if self.app.gui.playlists_screen_open {
            self.close_gui_playlists();
        } else {
            self.open_gui_playlists();
        }
    }

    /// The screen's content, `None` while the screen is closed.
    #[must_use]
    pub fn gui_playlists(&self) -> Option<GuiPlaylistsProjection> {
        self.app
            .gui
            .playlists_screen_open
            .then(|| GuiPlaylistsProjection {
                playlists: self.app.playlists.iter().map(playlist_row).collect(),
                open: self
                    .app
                    .playlists_open
                    .as_ref()
                    .map(|playlist| GuiOpenPlaylistProjection {
                        id: playlist.id.clone(),
                        name: playlist.name.clone(),
                        items: self
                            .app
                            .playlists_open_items
                            .iter()
                            .map(playlist_item)
                            .collect(),
                    }),
                loading: self.app.playlists_loading,
                open_loading: self.app.playlists_open_loading,
            })
    }

    /// Log the Playlists request whose frame is stale: it changes nothing.
    fn log_ignored_gui_playlists_command(command: &str, reason: impl std::fmt::Display) {
        tracing::debug!(
            name: "gui_playlists.command.ignored",
            target: "gui_playlists",
            command,
            reason = %reason,
            "the Playlists command changed nothing"
        );
    }
}

fn playlist_row(item: &EmbyItem) -> GuiPlaylistRowProjection {
    GuiPlaylistRowProjection {
        id: item.id.clone(),
        name: item.name.clone(),
        item_count: item.child_count.unwrap_or(0),
    }
}

fn playlist_item(item: &EmbyItem) -> GuiPlaylistItemProjection {
    GuiPlaylistItemProjection {
        id: item.id.clone(),
        name: item.name.clone(),
    }
}
