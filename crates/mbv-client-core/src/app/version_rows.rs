//! The text of the Settings version rows (`owner-gate-on-wire-identity`
//! design D4): the application version, build identity, and wire identity of
//! this Client and of the connected Owner. `App` formats both values, so the
//! TUI and the GUI show the same text.

use super::App;
use mbv_ctrl::BuildInfo;
use mbv_ui_model::settings::SettingKey;

impl App {
    /// The value a version row shows. `ClientVersion` formats this binary's
    /// identity, `OwnerVersion` the identity the connected Owner reported in
    /// its hello. Any other key has no version value and yields an empty
    /// string.
    #[must_use]
    pub fn version_row_value(&self, key: SettingKey) -> String {
        match key {
            SettingKey::ClientVersion => format_build_info(&BuildInfo::current()),
            SettingKey::OwnerVersion => format_build_info(self.player.remote().owner_build()),
            _ => String::new(),
        }
    }
}

/// `version · build · wire`, with an empty field shown as `unknown`. An Owner
/// from before the wire identity reports it empty.
fn format_build_info(info: &BuildInfo) -> String {
    [
        info.app_version.as_str(),
        info.build_id.as_str(),
        info.wire_id.as_str(),
    ]
    .map(|field| if field.is_empty() { "unknown" } else { field })
    .join(" \u{b7} ")
}

#[cfg(test)]
mod tests;
