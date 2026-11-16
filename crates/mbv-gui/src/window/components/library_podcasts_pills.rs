//! The Podcasts destination's episode pills
//! (`replace-tauri-with-blitz` task 7.4, the port of
//! `LibraryPodcastsPills`; add-gui-library-panel design D12, row
//! 10.1): the latest scope, the TUI's three state pills, and one pill per
//! show, single-select — the core's active pill is always one of them, so a
//! press selects it and the list loads that view. The pills paint the core's
//! carried choice, so the row and the list cannot disagree; a press sends
//! the set-pill command with the pill object the frame carries (design D1).
//! The keys' Space and Enter intent (design D3) becomes the same command.
//!
//! The row itself is the shared `LibraryPillRow` (design D15), so the
//! canvas's pill form has one owner.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiPillFrame;
use crate::bridge::library::GuiPodcastPill;
use crate::window::components::library_pill_row::{LibraryPill, library_pills_bound};
use crate::window::library::podcasts::podcast_pill_key;
use crate::window::library::state::LibraryState;

/// The props of the Podcasts pills (the `LibraryPodcastsPills`
/// props, narrowed to what the pills read): the carried pills and the
/// library index the set-pill command names (design D12, row 10.1). The
/// panel passes these alone, so a panel repaint never clones the list's rows
/// for the pills.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryPodcastsPillsProps {
    /// The carried episode pills in painted order (design D12, row 10.1).
    pub pills: Vec<GuiPillFrame<GuiPodcastPill>>,
    /// The Podcasts library index the set-pill command names.
    pub lib_idx: usize,
}

/// The Podcasts destination's episode pills: the carried pills over the
/// shared pill row, with the keys' pill request turned into the set-pill
/// command.
#[component]
pub fn LibraryPodcastsPills(props: LibraryPodcastsPillsProps) -> Element {
    let library = consume_context::<LibraryState>();

    // The pills the row paints, keyed by the pill's paint key.
    let pills: Vec<LibraryPill> = props
        .pills
        .iter()
        .map(|pill| LibraryPill {
            key: podcast_pill_key(&pill.pill),
            name: pill.name.clone(),
            selected: pill.selected,
        })
        .collect();

    // Press a pill: select its view (row 10.1).
    let press_library = library.clone();
    let carried_pills = props.pills.clone();
    let lib_idx = props.lib_idx;
    let on_press = Callback::new(move |key: String| {
        if let Some(pill) = carried_pills
            .iter()
            .find(|candidate| podcast_pill_key(&candidate.pill) == key)
        {
            press_library.select_podcasts_pill(lib_idx, pill.pill.clone());
        }
    });

    // The keys' pill request applies the same command (row 10.1).
    let watch_ui = library.podcasts_ui;
    let intent = use_memo(move || watch_ui.cloned().pill_select);
    let apply_library = library.clone();

    library_pills_bound(
        pills,
        "Podcasts views",
        Some("podcasts"),
        on_press,
        intent,
        move || apply_library.apply_podcasts_pill_select(lib_idx),
    )
}

#[cfg(test)]
mod tests;
