//! The Feeds destination's view pills (`replace-tauri-with-blitz` task 7.4,
//! the port of `LibraryFeedsPills`; add-gui-library-panel design D12,
//! row 11.1): the latest scope and one pill per subscription, single-select
//! — the core's active pill is always one of them, so a press selects it and
//! the list loads that view. The pills paint the core's carried choice, so
//! the row and the list cannot disagree; a press sends the set-pill command
//! with the pill object the frame carries (design D1). The keys' Space and
//! Enter intent (design D3) becomes the same command.
//!
//! The row itself is the shared `LibraryPillRow` (design D15), so the
//! canvas's pill form has one owner.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiPillFrame;
use crate::bridge::library::GuiFeedPill;
use crate::window::components::library_pill_row::{LibraryPill, library_pills_bound};
use crate::window::library::feeds::feed_pill_key;
use crate::window::library::state::LibraryState;

/// The props of the Feeds pills (the `LibraryFeedsPills` props,
/// narrowed to what the pills read): the carried pills of the one feed tab
/// (row 11.1). The panel passes these alone, so a panel repaint never clones
/// the list's rows for the pills.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryFeedsPillsProps {
    /// The carried view pills in painted order (row 11.1).
    pub pills: Vec<GuiPillFrame<GuiFeedPill>>,
}

/// The Feeds destination's view pills: the carried pills over the shared
/// pill row, with the keys' pill request turned into the set-pill command.
#[component]
pub fn LibraryFeedsPills(props: LibraryFeedsPillsProps) -> Element {
    let library = consume_context::<LibraryState>();

    // The pills the row paints, keyed by the pill's paint key.
    let pills: Vec<LibraryPill> = props
        .pills
        .iter()
        .map(|pill| LibraryPill {
            key: feed_pill_key(&pill.pill),
            name: pill.name.clone(),
            selected: pill.selected,
        })
        .collect();

    // Press a pill: select its view (row 11.1).
    let press_library = library.clone();
    let carried_pills = props.pills.clone();
    let on_press = Callback::new(move |key: String| {
        if let Some(pill) = carried_pills
            .iter()
            .find(|candidate| feed_pill_key(&candidate.pill) == key)
        {
            press_library.select_feeds_pill(pill.pill.clone());
        }
    });

    // The keys' pill request applies the same command (row 11.1).
    let watch_ui = library.feeds_ui;
    let intent = use_memo(move || watch_ui.cloned().pill_select);
    let apply_library = library.clone();

    library_pills_bound(
        pills,
        "Feeds views",
        Some("feeds"),
        on_press,
        intent,
        move || apply_library.apply_feeds_pill_select(),
    )
}

#[cfg(test)]
mod tests;
