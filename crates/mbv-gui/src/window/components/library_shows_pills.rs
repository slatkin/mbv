//! The Shows destination's mode pills (`replace-tauri-with-blitz` task 7.4,
//! the port of `LibraryShowsPills`; add-gui-library-panel design D12,
//! row 7.1): the view modes New, Upcoming, and All series, single-select —
//! the core's active mode is always one of them, so a press selects its mode
//! and the list loads it. The pills paint the core's carried choice, so the
//! row and the list cannot disagree; a press sends the set-mode command
//! (design D1). The keys' Space and Enter intent (design D3) becomes the
//! same command.
//!
//! The row itself is the shared `LibraryPillRow` (design D15), so the
//! canvas's pill form has one owner.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiShowsPillFrame;
use crate::window::components::library_pill_row::{LibraryPill, library_pills_bound};
use crate::window::library::shows::shows_mode_key;
use crate::window::library::state::LibraryState;

/// The props of the Shows pills (the `LibraryShowsPills` props,
/// narrowed to what the pills read): the carried pills and the library index
/// the set-mode command names (design D12). The panel passes these alone, so
/// a panel repaint never clones the list's rows for the pills.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryShowsPillsProps {
    /// The carried mode pills in painted order (design D12).
    pub pills: Vec<GuiShowsPillFrame>,
    /// The Shows library index the set-mode command names.
    pub lib_idx: usize,
}

/// The Shows destination's mode pills: the carried mode pills over the
/// shared pill row, with the keys' pill request turned into the set-mode
/// command.
#[component]
pub fn LibraryShowsPills(props: LibraryShowsPillsProps) -> Element {
    let library = consume_context::<LibraryState>();

    // The pills the row paints, keyed by the mode the command names.
    let pills: Vec<LibraryPill> = props
        .pills
        .iter()
        .map(|pill| LibraryPill {
            key: shows_mode_key(pill.mode).to_owned(),
            name: pill.name.clone(),
            selected: pill.selected,
        })
        .collect();

    // Press a pill: select its mode (design D12).
    let press_library = library.clone();
    let carried_pills = props.pills.clone();
    let lib_idx = props.lib_idx;
    let on_press = Callback::new(move |key: String| {
        if let Some(pill) = carried_pills
            .iter()
            .find(|candidate| shows_mode_key(candidate.mode) == key)
        {
            press_library.select_shows_pill(lib_idx, pill.mode);
        }
    });

    // The keys' pill request applies the same command (design D12).
    let watch_ui = library.shows_ui;
    let intent = use_memo(move || watch_ui.cloned().pill_select);
    let apply_library = library.clone();

    library_pills_bound(pills, "Shows modes", None, on_press, intent, move || {
        apply_library.apply_shows_pill_select(lib_idx);
    })
}

#[cfg(test)]
mod tests;
