//! The Feeds destination's collection pills (`replace-tauri-with-blitz` task
//! 7.4, the port of `LibraryFeedsPills`; add-gui-library-panel design D12, row
//! 11.1; gui-collection-dropdown design D6): the All row and one pill per
//! subscription over the destination's collections frame, multi-select. A
//! press sends the core's collection command the frame names — All, or a
//! toggle of one subscription — and the pills paint the core's carried
//! selection back, so the row and the list cannot disagree. The keys' Space
//! and Enter intent (design D3) becomes the same command.
//!
//! The row is the transitional seam: `LibraryCollectionDropdown` (design D6)
//! replaces it, and this component and the shared `LibraryPillRow` go with it.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiCollectionsFrame;
use crate::window::components::library_pill_row::{
    LibraryPill, collection_pills, library_pills_bound,
};
use crate::window::library::state::LibraryState;

/// The props of the Feeds pills (the `LibraryFeedsPills` props, narrowed to
/// what the pills read): the destination's collections frame (design D6).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryFeedsPillsProps {
    /// The carried collections frame: All, the configured subscriptions in
    /// the tab's order, and the checked membership.
    pub collections: GuiCollectionsFrame,
}

/// The Feeds destination's pills: the carried collections over the shared pill
/// row, with a press turned into the collection command the frame names.
#[component]
pub fn LibraryFeedsPills(props: LibraryFeedsPillsProps) -> Element {
    let library = consume_context::<LibraryState>();

    // The pills the row paints: All, then each subscription (design D6).
    let pills: Vec<LibraryPill> = collection_pills(&props.collections);

    // Press a pill: send the collection command the frame names.
    let collections = props.collections.clone();
    let press_library = library.clone();
    let on_press = Callback::new(move |key: String| {
        press_library.press_collection_pill(&collections, &key);
    });

    // The keys' pill request applies the same command (row 11.1).
    let watch_ui = library.feeds_ui;
    let intent = use_memo(move || watch_ui.cloned().pill_select);
    let apply_library = library.clone();
    let apply_collections = props.collections;

    library_pills_bound(
        pills,
        "Feeds collections",
        Some("feeds"),
        on_press,
        intent,
        move || apply_library.apply_feeds_pill_select(&apply_collections),
    )
}

#[cfg(test)]
mod tests;
