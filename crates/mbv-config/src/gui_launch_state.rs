//! Versioned GUI launch location: the destination, the selected collections of
//! each destination, and Panel focus a completed GUI Client session leaves for
//! the next start (`add-gui-library-panel` design D9). Additive beside
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

use super::launch_state::{LaunchStateError, write_state_at};
use super::{LaunchPanelFocus, TabIdentity, state_dir};
use std::path::{Path, PathBuf};

/// Current on-disk format version. The loader accepts only this version and
/// returns `None` for anything else, so a newer file never mis-resolves into
/// stale identities — a start without a location falls back to the Client's
/// own first-choice rules.
pub const GUI_LAUNCH_STATE_VERSION: u32 = 1;

const fn default_gui_launch_state_version() -> u32 {
    GUI_LAUNCH_STATE_VERSION
}

/// One GUI launch location: the active destination, the selected collections
/// of each destination, and the Panel that held focus. Written only at orderly
/// GUI exit, and read once at start.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GuiLaunchState {
    #[serde(default = "default_gui_launch_state_version")]
    pub version: u32,
    /// The destination that was active at exit.
    pub destination: TabIdentity,
    #[serde(default)]
    pub panel_focus: LaunchPanelFocus,
    /// One entry per destination whose selection the session changed, in the
    /// catalog's order. A destination with no entry had no selection: it opens
    /// with none.
    #[serde(default)]
    pub collections: Vec<GuiDestinationCollections>,
}

/// The selected collections of one destination: the destination's stable
/// identity, the selected collection group ids in pill order, and the album
/// each group's pill last showed (`add-gui-library-panel` design D13).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GuiDestinationCollections {
    pub destination: TabIdentity,
    pub group_ids: Vec<String>,
    /// The last picked album of each collection group, so each pill reopens
    /// on its own album. Absent in a file written before this field.
    #[serde(default)]
    pub picks: Vec<GuiGroupPick>,
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

/// The location a start without a saved one restores (design D9's
/// first-choice fallback): the restore takes the same open path a saved
/// location takes, and its fallback opens the first presented destination.
/// The identity is Home — no Service library, so the fallback resolution
/// picks the first presented one — with Library Panel focus and no saved
/// collection sets, whose empty set is the show-all selection: the union of
/// every group (add-gui-library-panel, the user's contract).
#[must_use]
pub fn first_launch() -> GuiLaunchState {
    GuiLaunchState {
        version: GUI_LAUNCH_STATE_VERSION,
        destination: TabIdentity::Home,
        panel_focus: LaunchPanelFocus::Library,
        collections: Vec::new(),
    }
}

pub(super) fn save_gui_launch_state_at(
    path: &Path,
    state: &GuiLaunchState,
) -> Result<(), LaunchStateError> {
    write_state_at(path, state)
}

/// Load the saved location. Missing, unreadable, malformed, or
/// version-mismatched files all yield `None` — a start is never prevented by
/// launch-state data.
#[must_use]
pub fn load_gui_launch_state() -> Option<GuiLaunchState> {
    load_gui_launch_state_at(&gui_launch_state_path())
}

pub(super) fn load_gui_launch_state_at(path: &Path) -> Option<GuiLaunchState> {
    let text = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str::<GuiLaunchState>(&text) {
        Ok(state) if state.version == GUI_LAUNCH_STATE_VERSION => Some(state),
        Ok(state) => {
            tracing::warn!(
                name: "launch_state.restore.unsupported_version",
                target: "launch_state",
                version = state.version,
                "GUI launch state not restored"
            );
            None
        }
        Err(e) => {
            tracing::warn!(
                name: "launch_state.restore.failed",
                target: "launch_state",
                error = %e,
                "GUI launch state not restored"
            );
            None
        }
    }
}
