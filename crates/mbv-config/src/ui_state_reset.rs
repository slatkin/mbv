// Selective saved-presentation clearing for the F2 Reset UI State action
// (change `separate-data-refresh-from-ui-reset`, design decision 4). This is
// the only place that deliberately forgets saved TUI presentation state: the
// launch snapshot, per-library browse positions, and presentation-layout
// preference entries. It never deletes prefs.json, the state directory or
// any unrelated state file, and it preserves non-presentation preferences
// such as volume, mute and pre-mute volume.

use super::{
    ConfigError, LibraryPositionState, prefs_path, save_library_position_state_result,
    tui_launch_state_path,
};
use std::path::Path;

/// Presentation-layout preference keys removed by a UI-state reset: the
/// current keys plus the legacy aliases that could otherwise reinstate an
/// old tab, focus or split width on the next launch (issue #745). Volume,
/// mute and unknown keys are deliberately absent.
const PRESENTATION_PREF_KEYS: &[&str] = &[
    "panel_focus",
    "queue_column_width",
    "list_pane_width",
    "visual_slot_hidden",
    "power_focus",
    "power_left_width",
    "power_left_tab",
    "library_tab",
];

/// Forget saved TUI presentation state so the next Client starts from
/// defaults. Every independent clear is attempted even when an earlier one
/// fails; the first failure is returned so callers can report a partial
/// reset while the live interface stays reset. A missing file counts as
/// already cleared, and unrelated preference values or state files are left
/// intact.
pub fn clear_saved_ui_presentation_state() -> Result<(), ConfigError> {
    let mut first_error = None;
    for outcome in [
        remove_missing_ok(&tui_launch_state_path()),
        clear_library_positions(),
        clear_presentation_prefs(&prefs_path()),
    ] {
        if let Err(error) = outcome
            && first_error.is_none()
        {
            first_error = Some(error);
        }
    }
    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn remove_missing_ok(path: &Path) -> Result<(), ConfigError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(ConfigError::state(format!(
            "remove {}: {error}",
            path.display()
        ))),
    }
}

fn clear_library_positions() -> Result<(), ConfigError> {
    save_library_position_state_result(&LibraryPositionState::default())
}

fn clear_presentation_prefs(path: &Path) -> Result<(), ConfigError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(ConfigError::state(format!(
                "read {}: {error}",
                path.display()
            )));
        }
    };
    let mut prefs: serde_json::Value = serde_json::from_str(&text)
        .map_err(|error| ConfigError::state(format!("parse {}: {error}", path.display())))?;
    let Some(table) = prefs.as_object_mut() else {
        return Err(ConfigError::state(format!(
            "{}: prefs root is not an object",
            path.display()
        )));
    };
    let mut removed = false;
    for key in PRESENTATION_PREF_KEYS {
        removed |= table.remove(*key).is_some();
    }
    if !removed {
        return Ok(());
    }
    super::state::save_json_atomic(path, &prefs, "prefs")
}
