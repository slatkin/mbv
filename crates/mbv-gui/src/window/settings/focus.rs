//! The form focus helper (the port of `lib/settings/focus.ts`'s
//! `focusOnMount` action): hand the keyboard to a form's first text field
//! when the form mounts, so a user who picked the action by key types
//! without reaching for the pointer (the spec's *keyboard reachability*). A
//! form mounts exactly when it opens.
//!
//! The Svelte action focuses the first `input` descendant of the node it
//! mounts on, and on an input the input itself. Blitz exposes no descendant
//! query from a mount handle, so the Rust call sites attach this helper to
//! the mounted element that takes the keyboard — the input itself, or the
//! form's first field's input — and the helper focuses that element.

use dioxus_native::prelude::*;

/// Focus the mounted element (`focus.ts`'s `focusOnMount`): the `onmounted`
/// handler of the form field that takes the keyboard calls it.
pub fn focus_on_mount(event: Event<MountedData>) {
    spawn(async move {
        let _ = event.set_focus(true).await;
    });
}
