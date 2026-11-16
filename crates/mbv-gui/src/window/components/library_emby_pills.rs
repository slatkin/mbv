//! The plain Emby-library destination's pills
//! (`replace-tauri-with-blitz` task 7.4, the port of
//! `LibraryEmbyPills`; add-gui-library-panel design D12, row 8.1): in
//! the feed-group mode, the latest scope, `All`, and each root folder that
//! holds items; in a plain library, the latest scope and `All`. A press
//! selects its group and the core loads that group's items. The pills paint
//! the core's carried choice, so the row and the list cannot disagree; a
//! press sends the set-feed-group command (design D1). The keys' Space and
//! Enter intent (design D3) becomes the same command.
//!
//! The row itself is the shared `LibraryPillRow` (design D15), so the
//! canvas's pill form has one owner.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiEmbyPillFrame;
use crate::window::components::library_pill_row::{LibraryPill, library_pills_bound};
use crate::window::library::emby_list::emby_group_key;
use crate::window::library::state::LibraryState;

/// The props of the Emby pills (the `LibraryEmbyPills` props,
/// narrowed to what the pills read): the carried pills and the library index
/// the set-feed-group command names (row 8.1). The panel passes these alone,
/// so a panel repaint never clones the list's rows for the pills.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryEmbyPillsProps {
    /// The carried group pills in painted order (row 8.1).
    pub pills: Vec<GuiEmbyPillFrame>,
    /// The plain Emby-library index the set-feed-group command names.
    pub lib_idx: usize,
}

/// The feed-group Emby library's pills: the carried group pills over the
/// shared pill row, with the keys' pill request turned into the
/// set-feed-group command.
#[component]
pub fn LibraryEmbyPills(props: LibraryEmbyPillsProps) -> Element {
    let library = consume_context::<LibraryState>();

    // The pills the row paints, keyed by the group key the command's pill
    // resolves.
    let pills: Vec<LibraryPill> = props
        .pills
        .iter()
        .map(|pill| LibraryPill {
            key: emby_group_key(&pill.group),
            name: pill.name.clone(),
            selected: pill.selected,
        })
        .collect();

    // Press a pill: select its group (row 8.1).
    let press_library = library.clone();
    let carried_pills = props.pills.clone();
    let lib_idx = props.lib_idx;
    let on_press = Callback::new(move |key: String| {
        if let Some(pill) = carried_pills
            .iter()
            .find(|candidate| emby_group_key(&candidate.group) == key)
        {
            press_library.select_emby_pill(lib_idx, pill.group.clone());
        }
    });

    // The keys' pill request applies the same command (row 8.1).
    let watch_ui = library.emby_ui;
    let intent = use_memo(move || watch_ui.cloned().pill_select);
    let apply_library = library.clone();

    library_pills_bound(
        pills,
        "Emby library folders",
        Some("emby"),
        on_press,
        intent,
        move || apply_library.apply_emby_pill_select(lib_idx),
    )
}

#[cfg(test)]
mod tests;
