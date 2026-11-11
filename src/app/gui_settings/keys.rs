//! The Keys section's commands (`add-gui-settings-screen` design D5 and D6,
//! row 5.4): capture, reset, and clear. The candidate configuration is built
//! with the pure `Keybinds` builders and accepted only when
//! `mbv_keybinds::load` returns `Ok`, so the editor applies the same rule set
//! the configuration load applies — never a second one. An accepted edit
//! replaces the running bindings and saves `[keys]` at once; a refused one
//! changes nothing and lands in the frame's refusal line.
//!
//! The `target` grammar is the frame's own: an action id names that action's
//! router chord, `prefix:<action id>` its prefix-namespace chord, and
//! `prefix` the prefix chord itself. A target this build cannot resolve is a
//! command from a stale frame and changes nothing.

use mbv_keybinds::{Keybinds, action_by_id};
use mbv_ui_msg::KeyEvent;

use super::projection::GuiSettingsKeyRefusalProjection;
use crate::app::App;
use crate::app::input::resolver::KeyChord;

/// The frame's binding target (`design D5`): what a `keysCapture` or
/// `keysClear` command edits.
#[derive(Clone, Copy)]
enum GuiKeyTarget {
    /// The action's router-scope chord.
    Router(&'static mbv_keybinds::KeybindAction),
    /// The action's prefix-namespace chord.
    PrefixAction(&'static mbv_keybinds::KeybindAction),
    /// The prefix chord itself.
    Prefix,
}

impl GuiKeyTarget {
    /// Resolve one command target string. `None` for a name this build does
    /// not declare, so a stale frame's command changes nothing.
    fn parse(target: &str) -> Option<Self> {
        if target == "prefix" {
            return Some(Self::Prefix);
        }
        if let Some(id) = target.strip_prefix("prefix:") {
            let action = action_by_id(id)?;
            return action
                .prefix_addressable
                .then_some(Self::PrefixAction(action));
        }
        action_by_id(target).map(Self::Router)
    }

    /// The action the target names, for the refusal line; the prefix chord
    /// itself belongs to no action.
    fn action_id(&self) -> Option<&'static str> {
        match self {
            Self::Router(action) | Self::PrefixAction(action) => Some(action.id),
            Self::Prefix => None,
        }
    }
}

impl App {
    /// Row 5.4's `keysCapture` arm: the captured press becomes the candidate
    /// chord for the target, and the edit is accepted only when the resulting
    /// configuration passes the load rules (design D6).
    pub(super) fn apply_gui_settings_keys_capture(&mut self, target: &str, key: KeyEvent) {
        let Some(target) = GuiKeyTarget::parse(target) else {
            Self::log_ignored_gui_settings_command("keysCapture", "the target is not declared");
            return;
        };
        let Some(chord) = KeyChord::from_key(key).to_keybinds_chord() else {
            self.set_gui_settings_key_refusal(
                target.action_id(),
                "the chord grammar cannot name this key",
            );
            return;
        };
        let keybinds = self.config.lock().unwrap().keybinds.clone();
        let candidate = match target {
            GuiKeyTarget::Router(action) => keybinds.with_router(action, chord),
            GuiKeyTarget::PrefixAction(action) => {
                keybinds.with_prefix_assignment(action, Some(chord))
            }
            GuiKeyTarget::Prefix => keybinds.with_prefix(Some(chord)),
        };
        self.commit_gui_settings_keybinds(&candidate, target.action_id());
    }

    /// Row 5.4's `keysReset` arm: remove the action's router-scope override,
    /// so it fires its declared default chords again (design D6).
    pub(super) fn apply_gui_settings_keys_reset(&mut self, action: &str) {
        let Some(action) = action_by_id(action) else {
            Self::log_ignored_gui_settings_command("keysReset", "the action is not declared");
            return;
        };
        let keybinds = self.config.lock().unwrap().keybinds.without_router(action);
        self.commit_gui_settings_keybinds(&keybinds, Some(action.id));
    }

    /// Row 5.4's `keysClear` arm: clear one prefix-namespace assignment, or
    /// the prefix chord (design D6).
    pub(super) fn apply_gui_settings_keys_clear(&mut self, target: &str) {
        let Some(target) = GuiKeyTarget::parse(target) else {
            Self::log_ignored_gui_settings_command("keysClear", "the target is not declared");
            return;
        };
        let keybinds = self.config.lock().unwrap().keybinds.clone();
        let (candidate, action) = match target {
            GuiKeyTarget::PrefixAction(action) => (
                keybinds.with_prefix_assignment(action, None),
                Some(action.id),
            ),
            GuiKeyTarget::Prefix => (keybinds.with_prefix(None), None),
            // A router-scope chord is reset, never cleared (design D6).
            GuiKeyTarget::Router(_) => {
                Self::log_ignored_gui_settings_command(
                    "keysClear",
                    "a router-scope chord has no clear",
                );
                return;
            }
        };
        self.commit_gui_settings_keybinds(&candidate, action);
    }

    /// Accept `candidate` when the configuration load accepts it (design D6):
    /// the loaded value replaces the running bindings, `[keys]` is saved at
    /// once, and the refusal line clears. Otherwise neither the bindings nor
    /// the file change, and the load's message becomes the refusal text.
    fn commit_gui_settings_keybinds(&mut self, candidate: &Keybinds, action: Option<&str>) {
        let loaded = match mbv_keybinds::load(&candidate.to_raw()) {
            Ok(loaded) => loaded,
            Err(error) => {
                self.set_gui_settings_key_refusal(action, &error.to_string());
                return;
            }
        };
        let config = {
            let mut config = self.config.lock().unwrap();
            config.keybinds = loaded;
            config.clone()
        };
        if let Err(error) =
            crate::config::save_config_section(&config, mbv_config::ConfigSection::Keys)
        {
            tracing::warn!(name: "config.save.failed", target: "config", error = %error, "config save failed");
            self.flash_error(format!(
                "Key bindings changed but the save failed ({error})"
            ));
        }
        if let Some(state) = self.gui_settings.as_mut() {
            state.refusal = None;
        }
    }

    /// Record a refused Keys edit in the frame (design D9): the refusal names
    /// the rule and, for a collision, the other action the load message names.
    fn set_gui_settings_key_refusal(&mut self, action: Option<&str>, text: &str) {
        tracing::info!(
            name: "gui_settings.keys.refused",
            target: "gui_settings",
            action = action.unwrap_or("prefix"),
            reason = %text,
            "the Keys edit was refused"
        );
        if let Some(state) = self.gui_settings.as_mut() {
            state.refusal = Some(GuiSettingsKeyRefusalProjection {
                action: action.unwrap_or("prefix").to_string(),
                text: text.to_string(),
            });
        }
    }
}
