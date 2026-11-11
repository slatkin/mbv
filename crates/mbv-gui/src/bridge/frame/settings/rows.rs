//! The F2 rows of a settings page (`add-gui-settings-screen` design D9): each
//! row's stable setting name, its label, how it is edited, its value text, its
//! toggle flag, and its choice list once loaded.

use mbv::GuiSettingsRowProjection;
use mbv_ui_model::settings::SettingKey;
use serde::Serialize;

/// One F2 row (design D9).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiSettingRow {
    /// The setting's stable wire name: the `activate` command's `setting`.
    pub key: String,
    /// The row's label.
    pub label: String,
    /// How the row is edited.
    pub kind: GuiSettingRowKind,
    /// The value's text, as the TUI paints it.
    pub value: String,
    /// The toggle's flag. `None` for a row that is no toggle.
    pub flag: Option<bool>,
    /// A choice list's choices once its load lands (row 5.1), and `None`
    /// otherwise.
    pub choices: Option<Vec<GuiSettingChoice>>,
}

impl From<&GuiSettingsRowProjection> for GuiSettingRow {
    fn from(row: &GuiSettingsRowProjection) -> Self {
        Self {
            key: row.key.name().to_string(),
            label: row.label.clone(),
            kind: GuiSettingRowKind::from(row.kind),
            value: row.value.clone(),
            flag: row.flag,
            choices: row
                .choices
                .as_ref()
                .map(|choices| choices.iter().map(GuiSettingChoice::from).collect()),
        }
    }
}

/// The four ways an F2 row is edited (design D9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiSettingRowKind {
    Toggle,
    Cycle,
    ChoiceList,
    Action,
}

impl From<mbv::GuiSettingsRowKind> for GuiSettingRowKind {
    fn from(kind: mbv::GuiSettingsRowKind) -> Self {
        match kind {
            mbv::GuiSettingsRowKind::Toggle => Self::Toggle,
            mbv::GuiSettingsRowKind::Cycle => Self::Cycle,
            mbv::GuiSettingsRowKind::ChoiceList => Self::ChoiceList,
            mbv::GuiSettingsRowKind::Action => Self::Action,
        }
    }
}

/// One choice of a choice-list row (design D9): `id` is the value the apply
/// command sends back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiSettingChoice {
    pub id: String,
    pub label: String,
    pub selected: bool,
}

impl From<&mbv::GuiSettingsChoiceProjection> for GuiSettingChoice {
    fn from(choice: &mbv::GuiSettingsChoiceProjection) -> Self {
        Self {
            id: choice.id.clone(),
            label: choice.label.clone(),
            selected: choice.selected,
        }
    }
}

/// The setting key a wire name names, for the `activate` command: `None` for a
/// name this build declares no key for, so a stale frame's edit changes
/// nothing.
#[must_use]
pub fn setting_key(name: &str) -> Option<SettingKey> {
    SettingKey::from_name(name)
}
