//! The Library pill row (`replace-tauri-with-blitz` task 7.4, the port of
//! `LibraryPills.svelte`; add-gui-library-panel design D6): the group pills
//! of the active Grouped Music library. One group is always selected, as the
//! TUI's Grouped Music pill is, so a press selects its pill and the others
//! deselect. The pills paint the core's carried choice, so the row and the
//! list cannot disagree; a press sends the select-collection command (design
//! D1). The keys' Space and Enter intent (design D3) becomes the same
//! command.
//!
//! The row itself is the shared `LibraryPillRow` the other destinations
//! paint too (design D15), so the canvas's pill form has one owner.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiMusicList;
use crate::window::components::library_pill_row::{LibraryPill, LibraryPillRow};
use crate::window::library::state::LibraryState;

/// The props of the Music pills (the `LibraryPills.svelte` props).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryPillsProps {
    /// The carried Music content: the pills and the library index the
    /// select-collection command names (design D2, D6).
    pub list: GuiMusicList,
}

/// The Music destination's pills: the carried group pills over the shared
/// pill row, with the keys' pill request turned into the select-collection
/// command.
#[component]
pub fn LibraryPills(props: LibraryPillsProps) -> Element {
    let library = consume_context::<LibraryState>();

    // The pills the row paints, keyed by the group id the command names.
    let pills: Vec<LibraryPill> = props
        .list
        .pills
        .iter()
        .map(|pill| LibraryPill {
            key: pill.group_id.clone(),
            name: pill.name.clone(),
            selected: pill.selected,
        })
        .collect();

    // The pill the keys' focus holds, or `None` while no pill has it.
    let ui = library.ui.cloned();
    let focused = library
        .focused_library_pill(&props.list.pills, &ui)
        .map(|pill| pill.group_id.clone());

    // Press a pill: select it (design D6).
    let press_library = library.clone();
    let lib_idx = props.list.lib_idx;
    let on_press = move |key: String| press_library.select_library_pill(lib_idx, key);

    // The keys' pill request becomes the command a press sends (design D6):
    // the effect reads the intent through the memo, so the consume-write of
    // the ui state never re-runs an effect that subscribes to it — the memo
    // settles at `None`, and the re-run finds nothing to send.
    let watch_ui = library.ui;
    let intent = use_memo(move || watch_ui.cloned().pill_select);
    let apply_library = library.clone();
    use_effect(move || {
        if intent.cloned().is_some() {
            apply_library.apply_library_pill_select(lib_idx);
        }
    });

    rsx! {
        LibraryPillRow {
            aria_label: "Collections",
            pills,
            focused,
            onpress: on_press,
        }
    }
}

#[cfg(test)]
mod tests;
