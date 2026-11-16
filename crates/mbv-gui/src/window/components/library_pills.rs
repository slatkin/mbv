//! The Library pill row (`replace-tauri-with-blitz` task 7.4, the port of
//! `LibraryPills`; add-gui-library-panel design D6): the group pills
//! of the active Grouped Music library. One group is always selected, as the
//! TUI's Grouped Music pill is, so a press selects its pill and the others
//! deselect. The pills paint the core's carried choice, so the row and the
//! list cannot disagree; a press sends the select-collection command (design
//! D1). A pill request (design D3) becomes the same command.
//!
//! The row itself is the shared `LibraryPillRow` the other destinations
//! paint too (design D15), so the canvas's pill form has one owner.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiMusicPillFrame;
use crate::window::components::library_pill_row::{LibraryPill, library_pills_bound};
use crate::window::library::state::LibraryState;

/// The props of the Music pills (the `LibraryPills` props, narrowed
/// to what the pills read): the carried pills and the library index the
/// select-collection command names (design D2, D6). The panel passes these
/// alone, so a panel repaint never clones the list's rows for the pills.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryPillsProps {
    /// The carried group pills in painted order (design D6).
    pub pills: Vec<GuiMusicPillFrame>,
    /// The Grouped Music library index the select-collection command names.
    pub lib_idx: usize,
}

/// The Music destination's pills: the carried group pills over the shared
/// pill row, with the keys' pill request turned into the select-collection
/// command.
#[component]
pub fn LibraryPills(props: LibraryPillsProps) -> Element {
    let library = consume_context::<LibraryState>();

    // The pills the row paints, keyed by the group id the command names.
    let pills: Vec<LibraryPill> = props
        .pills
        .iter()
        .map(|pill| LibraryPill {
            key: pill.group_id.clone(),
            name: pill.name.clone(),
            selected: pill.selected,
        })
        .collect();

    // Press a pill: select it (design D6).
    let press_library = library.clone();
    let lib_idx = props.lib_idx;
    let on_press =
        Callback::new(move |key: String| press_library.select_library_pill(lib_idx, key));

    // The keys' pill request applies the same command (design D6).
    let watch_ui = library.ui;
    let intent = use_memo(move || watch_ui.cloned().pill_select);
    let apply_library = library.clone();

    library_pills_bound(pills, "Collections", None, on_press, intent, move || {
        apply_library.apply_library_pill_select(lib_idx);
    })
}

#[cfg(test)]
mod tests;
