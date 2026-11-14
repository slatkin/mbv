//! The Podcasts destination's episode pills
//! (`replace-tauri-with-blitz` task 7.4, the port of
//! `LibraryPodcastsPills.svelte`; add-gui-library-panel design D12, row
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

use crate::bridge::frame::library::GuiPodcastsList;
use crate::window::components::library_pill_row::{LibraryPill, LibraryPillRow};
use crate::window::library::podcasts::podcast_pill_key;
use crate::window::library::state::LibraryState;

/// The props of the Podcasts pills (the `LibraryPodcastsPills.svelte`
/// props).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryPodcastsPillsProps {
    /// The carried Podcasts content: the pills and the library index the
    /// set-pill command names (design D12, row 10.1).
    pub list: GuiPodcastsList,
}

/// The Podcasts destination's episode pills: the carried pills over the
/// shared pill row, with the keys' pill request turned into the set-pill
/// command.
#[component]
pub fn LibraryPodcastsPills(props: LibraryPodcastsPillsProps) -> Element {
    let library = consume_context::<LibraryState>();

    // The pills the row paints, keyed by the pill's paint key.
    let pills: Vec<LibraryPill> = props
        .list
        .pills
        .iter()
        .map(|pill| LibraryPill {
            key: podcast_pill_key(&pill.pill),
            name: pill.name.clone(),
            selected: pill.selected,
        })
        .collect();

    // The key of the pill the keys' focus holds, or `None` while no pill has
    // it (design D6): the pill's paint key over the state's pill object.
    let focused = library
        .podcasts_ui
        .cloned()
        .focused_pill
        .map(|pill| podcast_pill_key(&pill));

    // Press a pill: select its view (row 10.1).
    let press_library = library.clone();
    let carried_pills = props.list.pills.clone();
    let lib_idx = props.list.lib_idx;
    let on_press = move |key: String| {
        if let Some(pill) = carried_pills
            .iter()
            .find(|candidate| podcast_pill_key(&candidate.pill) == key)
        {
            press_library.select_podcasts_pill(lib_idx, pill.pill.clone());
        }
    };

    // The keys' pill request becomes the command a press sends (row 10.1):
    // the effect reads the intent through the memo, so the consume-write of
    // the state never re-runs an effect that subscribes to it — the memo
    // settles at `None`, and the re-run finds nothing to send.
    let watch_ui = library.podcasts_ui;
    let intent = use_memo(move || watch_ui.cloned().pill_select);
    let apply_library = library.clone();
    use_effect(move || {
        if intent.cloned().is_some() {
            apply_library.apply_podcasts_pill_select(lib_idx);
        }
    });

    rsx! {
        LibraryPillRow {
            aria_label: "Podcasts views",
            pills,
            focused,
            pill_attr: "podcasts",
            onpress: on_press,
        }
    }
}

#[cfg(test)]
mod tests;
