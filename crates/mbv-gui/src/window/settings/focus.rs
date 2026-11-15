//! The form focus helper (the port of `lib/settings/focus.ts`'s
//! `focusOnMount` action): hand the keyboard to a form's first text field
//! when the form mounts, so a user who picked the action by key types
//! without reaching for the pointer (the spec's *keyboard reachability*). A
//! form mounts exactly when it opens.
//!
//! the former client action focuses the first `input` descendant of the node it
//! mounts on, and on an input the input itself. Blitz exposes no descendant
//! query from a mount handle, so the Rust call sites attach this helper to
//! the mounted element that takes the keyboard — the input itself, or the
//! form's first field's input — and the helper focuses that element.
//!
//! The mount also arms the router's text-field hold (`state.ts`'s
//! `focusSettingsField`). Blitz's `set_focus` writes the document's focus
//! directly and emits no `focusin`, so the form host's `onfocusin` never runs
//! for the mount's own focus: without the arm the hold stays `None`, and the
//! keyboard hands the letters to the core instead of the field. The host's
//! `onfocusin` then re-arms the hold on every later field-to-field move.

use dioxus_native::prelude::*;

use crate::window::settings::state::{SettingsForm, SettingsState};

/// Focus the mounted field and arm the form's keyboard hold (`focus.ts`'s
/// `focusOnMount`): the `onmounted` handler of the form field that takes the
/// keyboard calls it. The hold is armed here, before the focus lands, so the
/// key router already types into the field when the first key arrives.
pub fn focus_on_mount(event: Event<MountedData>, settings: &SettingsState, form: SettingsForm) {
    settings.focus_settings_field(form);
    spawn(async move {
        let _ = event.set_focus(true).await;
    });
}
