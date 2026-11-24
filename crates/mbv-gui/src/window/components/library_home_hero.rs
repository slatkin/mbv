//! The Home Hero (`replace-tauri-with-blitz` task 7.5, the port of
//! `LibraryHomeHero`; add-gui-library-panel design D12, the
//! USER-APPROVED GUI Home design): the selected row's detail pane in the
//! Music Hero's look — the artwork box the `artShape` arm names (landscape,
//! square, or portrait), the title, the meta rows with their duration and
//! progress roles, the overview, and the hover actions. No Workspace and no
//! tracklist paint here: Home has neither, so the pane is one block.
//!
//! Play and Add to queue (`a`) send the Home destination's row commands with
//! the frame's selected row (design D12) — the core owns the selection, so the
//! actions name the row the frame painted and never a position.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiHomeRowIdFrame, GuiLibraryDetail};
use crate::window::components::library_hero_summary::LibraryHeroSummary;
use crate::window::library::hero::selected_with_hero;
use crate::window::library::parts::library_home_detail;
use crate::window::library::state::LibraryState;
use crate::window::sections::SectionRef;

/// The props of the Home Hero (the `LibraryHomeHero` props, in order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryHomeHeroProps {
    /// The carried Home content (design D12): the selected row and its Hero
    /// facts, as the one handle every reader clones
    /// (`share-gui-sections-by-rc` design D3).
    pub detail: SectionRef<GuiLibraryDetail>,
}

/// Send a Home command for the frame's selected row (design D12): the row the
/// frame painted, never a position. The command sends only while the frame
/// holds both the selection and a Hero. The handle is narrowed here, so the
/// press reads the carried detail without the Hero cloning it per render
/// (`share-gui-sections-by-rc` design D3).
fn press(
    library: &LibraryState,
    detail: &SectionRef<GuiLibraryDetail>,
    command: fn(&LibraryState, GuiHomeRowIdFrame),
) {
    let Some(detail) = library_home_detail(Some(detail)) else {
        return;
    };
    let Some(row) = selected_with_hero(detail.selected.as_ref(), detail.hero.is_some()) else {
        return;
    };
    command(library, row.clone());
}

/// The Home Hero: the summary
/// block the one Hero summary paints, over the flat list.
#[component]
pub fn LibraryHomeHero(props: LibraryHomeHeroProps) -> Element {
    let library = consume_context::<LibraryState>();

    // The actions' sends (design D12): the Home destination's row commands
    // over the frame's selected row, never a position.
    let play_detail = props.detail.clone();
    let play_library = library.clone();
    let on_play = Callback::new(move |()| {
        press(&play_library, &play_detail, LibraryState::play_home_row);
    });
    let queue_detail = props.detail.clone();
    let queue_library = library.clone();
    let on_queue = Callback::new(move |()| {
        press(&queue_library, &queue_detail, LibraryState::queue_home_row);
    });

    // The Home part of the carried detail, narrowed from the handle in this
    // body, and the Hero's facts, present exactly while the pane paints.
    let Some(detail) = library_home_detail(Some(&props.detail)) else {
        return rsx! {};
    };
    let Some(hero) = detail.hero.clone() else {
        return rsx! {};
    };

    rsx! {
        div {
            class: "flex flex-col",
            style: "padding: 28px 32px; gap: 20px;",
            "data-library-hero": "",
            LibraryHeroSummary {
                attr: "home",
                section: "data-library-home-hero",
                hero,
                on_play: Some(on_play),
                on_queue: Some(on_queue),
            }
        }
    }
}

#[cfg(test)]
mod tests;
