//! The Shows destination's mode pills (`replace-tauri-with-blitz` task 7.4,
//! the port of `LibraryShowsPills`; add-gui-library-panel design D12,
//! row 7.1; gui-collection-dropdown design D8): the row stays mounted until
//! the collection-dropdown units remove it, and the core no longer carries any
//! Shows pill — the destination paints the series list alone — so the row
//! paints nothing and a press changes nothing. The row itself is the shared
//! `LibraryPillRow` (design D15), so the canvas's pill form has one owner.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiShowsPillFrame;
use crate::window::components::library_pill_row::{LibraryPill, library_pills_bound};

/// The props of the Shows pills (the `LibraryShowsPills` props,
/// narrowed to what the pills read): the carried pills, which the core holds
/// empty, and the library index. The panel passes these alone, so a panel
/// repaint never clones the list's rows for the pills.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryShowsPillsProps {
    /// The carried mode pills in painted order (design D12).
    pub pills: Vec<GuiShowsPillFrame>,
    /// The Shows library index the set-mode command names.
    pub lib_idx: usize,
}

/// The Shows destination's mode pills: the carried mode pills over the shared
/// pill row. The core carries none, so the row paints nothing; a press and the
/// keys' pill request change nothing.
#[component]
pub fn LibraryShowsPills(props: LibraryShowsPillsProps) -> Element {
    // The pills the row paints, keyed by their painted order: the core holds
    // none, so this list stays empty.
    let pills: Vec<LibraryPill> = props
        .pills
        .iter()
        .enumerate()
        .map(|(position, pill)| LibraryPill {
            key: position.to_string(),
            name: pill.name.clone(),
            selected: pill.selected,
        })
        .collect();

    // The row paints the carried pills; the destination's press path is the
    // series select, so the pill press has no command to send.
    let on_press = Callback::new(|_key: String| {});

    // The keys' pill request has no command either: the memo never settles on
    // a request.
    let intent = use_memo(|| None::<()>);

    library_pills_bound(pills, "Shows modes", None, on_press, intent, move || {})
}

#[cfg(test)]
mod tests;
