//! The GUI's dragged panel widths (`replace-tauri-with-blitz` design D14): the
//! Queue panel's column width and the Wide arrangement's list column width
//! that a GUI session dragged leave for the next start. One TOML file in the
//! state directory, beside the GUI launch state. The window's saved widths
//! are not migrated: a first start after the switch reads no file, and the
//! defaults apply.

use std::path::{Path, PathBuf};

use super::atomic_replace::write_bytes_atomically;
use super::state_dir;

/// The dragged widths one GUI session leaves for the next start (D14): each
/// column's dragged width in CSS pixels, `None` while the column holds its
/// default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GuiPanelWidths {
    /// The Queue panel column's dragged width in CSS pixels, `None` at its
    /// default.
    #[serde(default)]
    pub queue: Option<u32>,
    /// The Wide arrangement's list column's dragged width in CSS pixels,
    /// `None` at its default.
    #[serde(default)]
    pub list: Option<u32>,
}

/// The state-directory file the widths persist under (D14), beside the GUI
/// launch state.
#[must_use]
pub fn gui_panel_widths_path() -> PathBuf {
    state_dir().join("gui-panel-widths.toml")
}

/// The widths a start loads (D14): a missing, unreadable, or malformed file
/// gives `None` for both columns, and the defaults apply — stored data never
/// blocks the window.
#[must_use]
pub fn load_gui_panel_widths() -> GuiPanelWidths {
    load_gui_panel_widths_at(&gui_panel_widths_path())
}

pub(super) fn load_gui_panel_widths_at(path: &Path) -> GuiPanelWidths {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            tracing::debug!(
                target: "gui_panel_widths",
                error = %error,
                path = %path.display(),
                "no saved panel widths; the defaults apply"
            );
            return GuiPanelWidths::default();
        }
        Err(error) => {
            tracing::warn!(
                target: "gui_panel_widths",
                error = %error,
                path = %path.display(),
                "the saved panel widths do not read; the defaults apply"
            );
            return GuiPanelWidths::default();
        }
    };
    match toml::from_str(&text) {
        Ok(widths) => widths,
        Err(error) => {
            tracing::warn!(
                target: "gui_panel_widths",
                error = %error,
                path = %path.display(),
                "the saved panel widths do not parse; the defaults apply"
            );
            GuiPanelWidths::default()
        }
    }
}

/// Write the widths (D14): a drag end and a double-click reset write them,
/// and the GUI's Reset UI State writes the defaults. A refused write only
/// loses the save — the session still holds its widths — so a failure logs
/// and drops.
pub fn save_gui_panel_widths(widths: &GuiPanelWidths) {
    save_gui_panel_widths_at(&gui_panel_widths_path(), widths);
}

pub(super) fn save_gui_panel_widths_at(path: &Path, widths: &GuiPanelWidths) {
    let result = toml::to_string(widths)
        .map_err(|error| error.to_string())
        .and_then(|text| {
            write_bytes_atomically(path, text.as_bytes()).map_err(|error| error.to_string())
        });
    if let Err(error) = result {
        tracing::warn!(
            target: "gui_panel_widths",
            error = %error,
            path = %path.display(),
            "the panel widths do not save"
        );
    }
}
