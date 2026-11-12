//! The Keys section's content (`add-gui-settings-screen` design D9): the
//! prefix chord, one row for each `AllClients` action with its effective
//! chords, and the last refused edit.

use mbv::{GuiSettingsKeyRefusalProjection, GuiSettingsKeyRowProjection};
use serde::Serialize;

/// One Keys row (design D9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiSettingsKeyRow {
    /// The action's stable id: the `keysReset` command's `action`.
    pub id: String,
    pub label: String,
    /// The action's declared section name.
    pub section: String,
    /// The chords that fire the action under the loaded configuration.
    pub chords: Vec<String>,
    /// The action's prefix-namespace chord, where one is assigned.
    pub prefix_chord: Option<String>,
    /// Whether the binding differs from the action's declared default.
    pub changed: bool,
    /// Whether the prefix namespace can address the action.
    pub prefix_addressable: bool,
}

impl From<&GuiSettingsKeyRowProjection> for GuiSettingsKeyRow {
    fn from(row: &GuiSettingsKeyRowProjection) -> Self {
        Self {
            id: row.id.to_string(),
            label: row.label.clone(),
            section: row.section.to_string(),
            chords: row.chords.clone(),
            prefix_chord: row.prefix_chord.clone(),
            changed: row.changed,
            prefix_addressable: row.prefix_addressable,
        }
    }
}

/// A refused Keys edit (design D9): the action's id and the refusal text the
/// validator gave.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiSettingsKeyRefusal {
    pub action: String,
    pub text: String,
}

impl From<&GuiSettingsKeyRefusalProjection> for GuiSettingsKeyRefusal {
    fn from(refusal: &GuiSettingsKeyRefusalProjection) -> Self {
        Self {
            action: refusal.action.clone(),
            text: refusal.text.clone(),
        }
    }
}
