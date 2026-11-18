//! Versioned GUI launch location: the destination and the selected collections
//! of each destination that a completed GUI Client session leaves for the next
//! start (`add-gui-library-panel` design D9). Additive beside
//! [`super::TuiLaunchState`]: the TUI never reads this file, and the GUI never
//! writes the TUI's.
//
// The snapshot holds stable Service identities only — a `TabIdentity` for the
// destination and each collection set's destination, and the collection
// groups' Service ids — never a tab position, a pill position, or a display
// name, because the catalog's order and the groups' labels can change between
// exit and the next launch. Resolution against the launched catalog (a
// destination the Client does not present, a group the library no longer
// holds) belongs to the Client, not to this file.

use super::launch_state::{LaunchStateError, load_state_at, write_state_at};
use super::{TabIdentity, state_dir};
use std::path::{Path, PathBuf};

/// Current on-disk format version. The loader accepts only this version and
/// returns `None` for anything else, so a newer file never mis-resolves into
/// stale identities — a start without a location falls back to the Client's
/// own first-choice rules.
pub const GUI_LAUNCH_STATE_VERSION: u32 = 1;

const fn default_gui_launch_state_version() -> u32 {
    GUI_LAUNCH_STATE_VERSION
}

/// One GUI launch location: the active destination and the selected
/// collections of each destination. Written only at orderly GUI exit, and read
/// once at start.
///
/// The struct does not deny unknown fields: a file that an earlier build wrote
/// with a Panel focus still loads, and that field is ignored.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GuiLaunchState {
    #[serde(default = "default_gui_launch_state_version")]
    pub version: u32,
    /// The destination that was active at exit.
    pub destination: TabIdentity,
    /// One entry per destination whose selection the session changed, in the
    /// catalog's order. A destination with no entry had no selection: it opens
    /// with none.
    #[serde(default)]
    pub collections: Vec<GuiDestinationCollections>,
    /// The Shows destination's one active view. An older file without the
    /// field loads as All: the destination's default series list.
    #[serde(default)]
    pub shows_view: GuiLaunchShowsView,
}

/// The three views a GUI Shows destination can show (gui-collection-dropdown
/// design D8): a single choice, held in its own launch field rather than in a
/// collection entry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuiLaunchShowsView {
    /// The series list, the default and the value an old file loads as.
    #[default]
    All,
    /// The merged New (latest episodes) list.
    New,
    /// The merged Upcoming premieres list.
    Upcoming,
}

/// The selected collections of one destination: the destination's stable
/// identity, the collection dropdown kind the ids and picks belong to, the
/// checked ids in press order, the list's date order, and the album each
/// collection's pill last showed (`add-gui-library-panel` design D13).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GuiDestinationCollections {
    pub destination: TabIdentity,
    /// The collection dropdown whose selection this entry holds. An entry
    /// written before this field held a Grouped Music library's groups.
    #[serde(default)]
    pub target: GuiLaunchCollectionTarget,
    pub group_ids: Vec<String>,
    /// The list's date order where the destination stores one. Absent for a
    /// Grouped Music library, which sorts by name.
    #[serde(default)]
    pub order: Option<GuiLaunchDateOrder>,
    /// The last picked album of each collection group, so each pill reopens
    /// on its own album. Absent in a file written before this field.
    #[serde(default)]
    pub picks: Vec<GuiGroupPick>,
}

/// The collection dropdown a saved entry belongs to. The destination's
/// identity names the library or tab; this kind distinguishes the dropdowns
/// one identity can hold, so a Music group list and an Emby feed's folder
/// list never read each other's saved ids.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuiLaunchCollectionTarget {
    /// A Grouped Music library's groups. This is the default an old entry
    /// without a `target` field held.
    #[default]
    Music,
    /// An Audiobookshelf podcast library's shows.
    Podcasts,
    /// The Feeds tab's subscriptions.
    Feeds,
    /// An Emby feed-view library's folders.
    EmbyFeed,
}

/// The date order a saved feed-based list sorts by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuiLaunchDateOrder {
    /// The most recent row first.
    #[default]
    Newest,
    /// The earliest row first.
    Oldest,
}

/// The album a collection group's pill last showed: the group's Service id
/// and the album's Service id, never a position or a display name.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GuiGroupPick {
    pub group_id: String,
    pub album_id: String,
}

#[must_use]
pub fn gui_launch_state_path() -> PathBuf {
    state_dir().join("gui_launch_state.json")
}

/// Serialize the location and write it atomically over the GUI launch-state
/// file, through the same write path the TUI snapshot uses (see
/// [`super::save_tui_launch_state`]).
pub fn save_gui_launch_state(state: &GuiLaunchState) -> Result<(), LaunchStateError> {
    save_gui_launch_state_at(&gui_launch_state_path(), state)
}

/// The location a start without a saved one restores (design D9): the
/// restore takes the same open path a saved location takes, and its Home
/// destination opens the first presented destination, so a fresh launch
/// lands on one. The identity is Home — no Service library, which resolves
/// to the first presented destination — with no saved collection sets, so the
/// restore selects nothing for any destination and each opens on its default
/// first group (design D6's single-select pills; see
/// `restore_gui_launch_collections`).
#[must_use]
pub fn first_launch() -> GuiLaunchState {
    GuiLaunchState {
        version: GUI_LAUNCH_STATE_VERSION,
        destination: TabIdentity::Home,
        collections: Vec::new(),
        shows_view: GuiLaunchShowsView::All,
    }
}

pub(super) fn save_gui_launch_state_at(
    path: &Path,
    state: &GuiLaunchState,
) -> Result<(), LaunchStateError> {
    write_state_at(path, state)
}

/// Delete the saved location, so the next start falls back to the Client's
/// own first-choice rules (the GUI's Reset UI State,
/// `add-gui-settings-screen` D10). A file that is already absent is a
/// cleared state, not a failure.
pub fn clear_gui_launch_state() -> Result<(), LaunchStateError> {
    clear_gui_launch_state_at(&gui_launch_state_path())
}

pub(super) fn clear_gui_launch_state_at(path: &Path) -> Result<(), LaunchStateError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(LaunchStateError::remove(format!(
            "remove {}: {error}",
            path.display()
        ))),
    }
}

/// Load the saved location. Missing, unreadable, malformed, or
/// version-mismatched files all yield `None` — a start is never prevented by
/// launch-state data.
#[must_use]
pub fn load_gui_launch_state() -> Option<GuiLaunchState> {
    load_gui_launch_state_at(&gui_launch_state_path())
}

pub(super) fn load_gui_launch_state_at(path: &Path) -> Option<GuiLaunchState> {
    load_state_at(path, GUI_LAUNCH_STATE_VERSION)
}
