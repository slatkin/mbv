//! The Shows destination's pill row (`replace-tauri-with-blitz` task 7.4, the
//! port of `LibraryShowsPills`; add-gui-library-panel design D12, row 7.1;
//! gui-collection-dropdown design D8, D6): the row stays mounted until the
//! collection-dropdown units remove it, and the destination paints no
//! collection selector at all — Shows has no collections, so its frame carries
//! none and the row paints nothing. The row itself is the shared
//! `LibraryPillRow` (design D15), so the canvas's pill form has one owner.

use dioxus_native::prelude::*;

use crate::window::components::library_pill_row::{LibraryPill, library_pills_bound};

/// The Shows destination's pill row: no carried pills, because Shows paints
/// the series list alone (design D8).
#[component]
pub fn LibraryShowsPills() -> Element {
    // Shows carries no collections frame, so the row paints nothing and a
    // press changes nothing.
    let pills: Vec<LibraryPill> = Vec::new();

    // The row paints no pill, so the pill press has no command to send.
    let on_press = Callback::new(|_key: String| {});

    // The keys' pill request has no command either: the memo never settles on
    // a request.
    let intent = use_memo(|| None::<()>);

    library_pills_bound(
        pills,
        "Shows collections",
        None,
        on_press,
        intent,
        move || {},
    )
}

#[cfg(test)]
mod tests;
