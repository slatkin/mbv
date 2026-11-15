//! The Settings pick menu (add-gui-settings-screen design D13, the port of
//! `lib/settings/pick-menu.ts`): a pick row's value list, open over its pick
//! button until the user chooses a value, presses outside, or presses Esc.
//! The open menu is window content, not a core overlay, so the window holds
//! it here and the Settings key router ([`super::keys`]) reads it: Up and
//! Down move its cursor, Enter sends `setValue`, and Esc closes it with no
//! send. One menu is open at a time — a second pick button's press replaces
//! the first menu — because the cursor and the settings name live in one
//! fact.

use dioxus_native::prelude::*;

use crate::bridge::settings::GuiSettingsCommand;

use super::state::SettingsState;

/// The open pick menu (`pick-menu.ts`'s `PickMenuState`): the row's setting,
/// its values, and the cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickMenuState {
    /// The row's setting name, which `setValue` carries back.
    pub setting: String,
    /// The row's values, in the frame's order.
    pub values: Vec<String>,
    /// The menu cursor, an index into `values`.
    pub cursor: usize,
}

/// The painted text of one value (`pick-menu.ts`'s `pickValueLabel`): the
/// empty value is the automatic choice.
#[must_use]
pub fn pick_value_label(value: &str) -> &str {
    if value.is_empty() { "Any" } else { value }
}

impl SettingsState {
    /// Open the row's value menu with the cursor on the value it shows now
    /// (`pick-menu.ts`'s `openPickMenu`).
    pub fn open_pick_menu(&self, setting: &str, value: &str, values: &[String]) {
        let cursor = values
            .iter()
            .position(|candidate| candidate == value)
            .unwrap_or(0);
        let mut menu = self.pick_menu;
        menu.set(Some(PickMenuState {
            setting: setting.to_owned(),
            values: values.to_vec(),
            cursor,
        }));
    }

    /// Open the row's menu, or close it when it is this row's already
    /// (`pick-menu.ts`'s `togglePickMenu`): the one activation a pick row
    /// runs, from Enter or from a press on its button.
    pub fn toggle_pick_menu(&self, setting: &str, value: &str, values: &[String]) {
        if self
            .pick_menu
            .peek()
            .as_ref()
            .is_some_and(|menu| menu.setting == setting)
        {
            self.close_pick_menu();
            return;
        }
        self.open_pick_menu(setting, value, values);
    }

    /// Close the open menu with no change (`pick-menu.ts`'s
    /// `closePickMenu`): Esc, a press outside, a second press on the pick
    /// button.
    pub fn close_pick_menu(&self) {
        let mut menu = self.pick_menu;
        menu.set(None);
    }

    /// Move the menu cursor one value, inside the list's ends
    /// (`pick-menu.ts`'s `movePickMenuCursor`).
    pub fn move_pick_menu_cursor(&self, step: isize) {
        let mut menu = self.pick_menu;
        menu.with_mut(|menu| {
            if let Some(menu) = menu {
                let last = menu.values.len().saturating_sub(1);
                menu.cursor = menu.cursor.saturating_add_signed(step).min(last);
            }
        });
    }

    /// Choose the value at `index` (`pick-menu.ts`'s `selectPickValue`): the
    /// `setValue` command carries the row's setting and that value, and the
    /// menu closes. An index outside the list sends nothing.
    pub fn select_pick_value(&self, index: usize) {
        let Some(menu) = self.pick_menu.peek().clone() else {
            return;
        };
        let Some(value) = menu.values.get(index) else {
            return;
        };
        self.link.settings(GuiSettingsCommand::SetValue {
            setting: menu.setting.clone(),
            value: value.clone(),
        });
        let mut menu = self.pick_menu;
        menu.set(None);
    }
}
