//! The Music destination's collection pills (`replace-tauri-with-blitz` task
//! 7.4, the port of `LibraryPills`; add-gui-library-panel design D6;
//! gui-collection-dropdown design D6): the All row and one pill per group over
//! the destination's collections frame. A press sends the core's collection
//! command the frame names — All, or a toggle of one group — and the pills
//! paint the core's carried selection back, so the row and the list cannot
//! disagree.
//!
//! The row is the transitional seam: `LibraryCollectionDropdown` (design D6)
//! replaces it, and this component and the shared `LibraryPillRow` go with it.
//! The row itself is the shared `LibraryPillRow` (design D15), so the canvas's
//! pill form has one owner.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiCollectionsFrame;
use crate::window::components::library_pill_row::{
    LibraryPill, collection_pills, library_pills_bound,
};
use crate::window::library::state::LibraryState;

/// The props of the Music pills (the `LibraryPills` props, narrowed to what
/// the pills read): the destination's collections frame (design D6). The
/// panel passes this alone, so a panel repaint never clones the list's rows
/// for the pills.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryPillsProps {
    /// The carried collections frame: All, the groups in group-level order,
    /// and the checked membership.
    pub collections: GuiCollectionsFrame,
}

/// The Music destination's pills: the carried collections over the shared pill
/// row, with a press turned into the collection command the frame names.
#[component]
pub fn LibraryPills(props: LibraryPillsProps) -> Element {
    let library = consume_context::<LibraryState>();

    // The pills the row paints: All, then each group (design D6).
    let pills: Vec<LibraryPill> = collection_pills(&props.collections);

    // Press a pill: send the collection command the frame names.
    let collections = props.collections.clone();
    let press_library = library.clone();
    let on_press = Callback::new(move |key: String| {
        press_library.press_collection_pill(&collections, &key);
    });

    // The keys' pill request applies the same command (design D6).
    let watch_ui = library.ui;
    let intent = use_memo(move || watch_ui.cloned().pill_select);
    let apply_library = library.clone();
    let apply_collections = props.collections;

    library_pills_bound(pills, "Collections", None, on_press, intent, move || {
        apply_library.apply_library_pill_select(&apply_collections);
    })
}

#[cfg(test)]
mod tests;
