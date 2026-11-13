//! The GUI's dragged panel widths (`replace-tauri-with-blitz` design D14): the
//! Queue panel's column width and the Wide arrangement's list column width
//! that a GUI session dragged leave for the next start. One TOML file in the
//! state directory, beside the GUI launch state. The webview's saved widths
//! are not migrated: a first start after the switch reads no file, and the
//! defaults apply.

use std::path::{Path, PathBuf};

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
    if let Err(error) = write_state_at(path, widths) {
        tracing::warn!(
            target: "gui_panel_widths",
            error = %error,
            path = %path.display(),
            "the panel widths do not save"
        );
    }
}

/// Serialize the widths, write a process-unique temporary file beside `path`,
/// and atomically rename it over `path` — the same replace rule the launch
/// states write by (see `launch_state::write_state_at`): a killed Client
/// leaves the previous completed save intact.
fn write_state_at(path: &Path, widths: &GuiPanelWidths) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = toml::to_string(widths).map_err(|error| std::io::Error::other(error.to_string()))?;
    let file_name = path.file_name().map_or_else(
        || "gui-panel-widths.toml".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let tmp = path.with_file_name(format!(
        "{file_name}.tmp-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::write(&tmp, &text)?;
    if let Err(error) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(error);
    }
    Ok(())
}
