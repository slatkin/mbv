//! The Settings screen's key routing (add-gui-settings-screen design D11,
//! the port of `lib/settings/keys.ts`): [`crate::window::library::keys`]
//! hands every keydown here while the frame carries the Settings screen, and
//! this module is the screen's one routing site — the components press
//! through the store's functions, never a second router. The order is the
//! design's: an active capture takes every key (Esc cancels, any other key
//! is the candidate), a focused text field takes the key (Esc cancels, Enter
//! submits, the rest types), an open pick menu takes its own keys (Up and
//! Down walk it, Enter sets the value, Esc closes it), then the screen keys
//! run — the movements in the focused pane, Tab and Shift+Tab between the
//! panes, Enter opens or activates, Space activates a toggle or a choice
//! mark, and Esc walks the ladder — and every other key goes to the core, so
//! the playback chords keep working behind the screen.

use dioxus_native::prelude::*;

use crate::bridge::frame::settings::GuiSettingsFrame;
use crate::bridge::keys::DomKey;
use crate::window::input::KeyOutcome;
use crate::window::library::keys_shared::{bare, cursor_move};
use crate::window::library::list::ListMove;

use super::state::{
    SettingsPane, SettingsState, current_settings_entries, focused_settings_entry, space_activates,
};

/// Route one keydown while the Settings screen shows (`keys.ts`'s
/// `routeSettingsKey`, design D11): the window's overlay and prefix step ran
/// first, so a key that reaches this point is the screen's or the core
/// router's.
#[must_use]
pub fn route_settings_key(
    key: &DomKey,
    settings: &GuiSettingsFrame,
    state: &SettingsState,
) -> KeyOutcome {
    let ui = state.ui.peek().clone();
    // (2) An active capture takes every key: Esc cancels, and any other key
    // is the candidate chord `keysCapture` carries. A sent capture swallows
    // the keys until the frame's answer ends it.
    if let Some(capture) = &ui.capture {
        if !capture.sent {
            if key.key == "Escape" && bare(key) {
                state.cancel_keys_capture();
            } else {
                state.send_keys_capture(capture.target.clone(), key.clone());
            }
        }
        return KeyOutcome::Handled;
    }
    // (3) A focused text field takes the key: Esc cancels the form, Enter
    // submits it, and every other key types into the field — the `text`
    // answer lets the input's own default run and sends nothing to the core.
    // Tab's default walks the form's fields and buttons.
    if ui.text_field.is_some() {
        if key.key == "Escape" && bare(key) {
            state.cancel_settings_form();
            return KeyOutcome::Handled;
        }
        if key.key == "Enter" && bare(key) {
            state.submit_settings_form(&settings.content);
            return KeyOutcome::Handled;
        }
        return KeyOutcome::Text;
    }
    // A screen key shows the cursor; a pointer press hides it again (the
    // `keyboard` fact in `state.rs`).
    state.show_settings_cursor();
    // (4) An open pick menu takes its own keys (design D13): Up and Down walk
    // its values, Enter sends `setValue` for the cursor value, and Esc closes
    // it with no change. Any other key closes it and falls through, so the
    // screen's own keys keep acting and the menu never outlives them. The
    // read binds before the branch: a scrutinee temporary would hold the
    // signal's read lock through the store calls below.
    let menu = state.pick_menu.peek().clone();
    if let Some(menu) = menu {
        if key.key == "Escape" && bare(key) {
            state.close_pick_menu();
            return KeyOutcome::Handled;
        }
        if key.key == "Enter" && bare(key) {
            state.select_pick_value(menu.cursor);
            return KeyOutcome::Handled;
        }
        let pick_move = cursor_move(&key.key);
        if matches!(pick_move, Some(ListMove::Up | ListMove::Down)) && bare(key) {
            state.move_pick_menu_cursor(if pick_move == Some(ListMove::Up) {
                -1
            } else {
                1
            });
            return KeyOutcome::Handled;
        }
        state.close_pick_menu();
    }
    // (5) The screen keys run.
    if key.key == "Escape" && bare(key) {
        state.settings_escape(settings);
        return KeyOutcome::Handled;
    }
    if key.key == "Tab" && !key.modifiers.ctrl_key && !key.meta_key && !key.modifiers.alt_key {
        state.switch_settings_pane();
        return KeyOutcome::Handled;
    }
    if let Some(mv) = cursor_move(&key.key)
        && bare(key)
    {
        let count = if ui.pane == SettingsPane::Sidebar {
            settings.sections.len()
        } else {
            current_settings_entries(&settings.content, &ui).len()
        };
        state.move_settings_cursor(mv, count);
        return KeyOutcome::Handled;
    }
    if key.key == "Enter" && bare(key) {
        if ui.pane == SettingsPane::Sidebar {
            let section = settings
                .sections
                .get(
                    ui.sidebar_cursor
                        .min(settings.sections.len().saturating_sub(1)),
                )
                .copied();
            if let Some(section) = section {
                state.select_settings_section(section);
            }
            return KeyOutcome::Handled;
        }
        if let Some(entry) = focused_settings_entry(&settings.content, &ui) {
            state.activate_settings_entry(&entry);
        }
        return KeyOutcome::Handled;
    }
    if key.key == " " && bare(key) {
        // Space activates a focused toggle or a choice mark; on the sidebar
        // and on every other entry it is the core's play and pause (the
        // spec's *Global keys keep working behind the screen*).
        if ui.pane == SettingsPane::Content {
            let entry = focused_settings_entry(&settings.content, &ui);
            if space_activates(entry.as_ref()) {
                if let Some(entry) = entry {
                    state.activate_settings_entry(&entry);
                }
                return KeyOutcome::Handled;
            }
        }
        return KeyOutcome::Core;
    }
    // (6) Every other key goes to the core's keyboard router.
    KeyOutcome::Core
}
