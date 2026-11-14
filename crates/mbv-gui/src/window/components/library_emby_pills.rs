//! The plain Emby-library destination's pills
//! (`replace-tauri-with-blitz` task 7.4, the port of
//! `LibraryEmbyPills.svelte`; add-gui-library-panel design D12, row 8.1): in
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

use crate::bridge::frame::library::GuiEmbyList;
use crate::window::components::library_pill_row::{LibraryPill, LibraryPillRow};
use crate::window::library::emby_list::emby_group_key;
use crate::window::library::state::LibraryState;

/// The props of the Emby pills (the `LibraryEmbyPills.svelte` props).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryEmbyPillsProps {
    /// The carried plain Emby-library content: the pills and the library
    /// index the set-feed-group command names (row 8.1).
    pub list: GuiEmbyList,
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
        .list
        .pills
        .iter()
        .map(|pill| LibraryPill {
            key: emby_group_key(&pill.group),
            name: pill.name.clone(),
            selected: pill.selected,
        })
        .collect();

    // The pill the keys' focus holds, as the group's stable key; it paints
    // the accent outline (design D6).
    let focused = library.emby_ui.cloned().focused_pill;

    // Press a pill: select its group (row 8.1).
    let press_library = library.clone();
    let carried_pills = props.list.pills.clone();
    let lib_idx = props.list.lib_idx;
    let on_press = move |key: String| {
        if let Some(pill) = carried_pills
            .iter()
            .find(|candidate| emby_group_key(&candidate.group) == key)
        {
            press_library.select_emby_pill(lib_idx, pill.group.clone());
        }
    };

    // The keys' pill request becomes the command a press sends (row 8.1):
    // the effect reads the intent through the memo, so the consume-write of
    // the state never re-runs an effect that subscribes to it — the memo
    // settles at `None`, and the re-run finds nothing to send.
    let watch_ui = library.emby_ui;
    let intent = use_memo(move || watch_ui.cloned().pill_select);
    let apply_library = library.clone();
    use_effect(move || {
        if intent.cloned().is_some() {
            apply_library.apply_emby_pill_select(lib_idx);
        }
    });

    rsx! {
        LibraryPillRow {
            aria_label: "Emby library folders",
            pills,
            focused,
            pill_attr: "emby",
            onpress: on_press,
        }
    }
}

#[cfg(test)]
mod tests;
