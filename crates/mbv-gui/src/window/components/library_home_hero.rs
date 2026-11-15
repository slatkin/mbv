//! The Home Hero (`replace-tauri-with-blitz` task 7.5, the port of
//! `LibraryHomeHero`; add-gui-library-panel design D12, the
//! USER-APPROVED GUI Home design): the selected row's detail pane in the
//! Music Hero's look — the artwork box the `artShape` arm names (landscape,
//! square, or portrait), the title, the meta rows with their duration and
//! progress roles, the overview, and the hover actions. No Workspace and no
//! tracklist paint here: Home has neither, so the pane is one block.
//!
//! Play, Play next (`n`), and Add to queue (`a`) send the Home commands with
//! the frame's selected row — the core owns the selection, so the actions
//! name the row the frame painted and never a position.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiHomeDetail;
use crate::bridge::library::GuiLibraryCommand;
use crate::window::components::library_hero_summary::LibraryHeroSummary;
use crate::window::library::state::LibraryState;

/// The props of the Home Hero (the `LibraryHomeHero` props, in order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryHomeHeroProps {
    /// The carried Home content (design D12): the selected row and its Hero
    /// facts.
    pub detail: GuiHomeDetail,
}

/// The Home action a button sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HomeAction {
    /// Play the selected row (design D12).
    Play,
    /// Play the selected row next in the queue.
    PlayNext,
    /// Add the selected row to the queue.
    Queue,
}

/// Send a Home command for the frame's selected row (design D12): the frame's
/// selected row, never a position. The command sends only while the frame
/// holds both the selection and a Hero.
fn act(library: &LibraryState, action: HomeAction, detail: &GuiHomeDetail) {
    let (Some(selected), Some(_)) = (&detail.selected, &detail.hero) else {
        return;
    };
    let row = selected.clone();
    let command = match action {
        HomeAction::Play => GuiLibraryCommand::HomePlay { row: row.into() },
        HomeAction::PlayNext => GuiLibraryCommand::HomePlayNext { row: row.into() },
        HomeAction::Queue => GuiLibraryCommand::HomeQueue { row: row.into() },
    };
    library.link.library(command);
}

/// The Home Hero: the summary
/// block the one Hero summary paints, over the flat list.
#[component]
pub fn LibraryHomeHero(props: LibraryHomeHeroProps) -> Element {
    let library = consume_context::<LibraryState>();

    // The actions' sends (design D12): the frame's selected row, never a
    // position.
    let play_detail = props.detail.clone();
    let play_library = library.clone();
    let on_play = Callback::new(move |()| act(&play_library, HomeAction::Play, &play_detail));
    let next_detail = props.detail.clone();
    let next_library = library.clone();
    let on_play_next = Callback::new(move |()| {
        act(&next_library, HomeAction::PlayNext, &next_detail);
    });
    let queue_detail = props.detail.clone();
    let queue_library = library.clone();
    let on_queue = Callback::new(move |()| act(&queue_library, HomeAction::Queue, &queue_detail));

    let Some(hero) = props.detail.hero else {
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
                on_play_next: Some(on_play_next),
                on_queue: Some(on_queue),
            }
        }
    }
}

#[cfg(test)]
mod tests;
