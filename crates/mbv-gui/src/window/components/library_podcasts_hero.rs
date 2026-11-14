//! The Podcasts Hero (`replace-tauri-with-blitz` task 7.6, the port of
//! `LibraryPodcastsHero.svelte`; add-gui-library-panel design D12, row 10.1):
//! the selected episode's detail pane in the Emby Hero look — the artwork box
//! the `artShape` arm names, the title, the meta rows with their duration and
//! progress roles, and the episode description as the overview.
//!
//! A podcast Hero is episode information only: it carries no Workspace and no
//! tracklist. There is no play-next either: `n` is consumed by the front end
//! and does nothing for podcast episodes (design D12, row 10.1), so only Play
//! and Add to queue paint.
//!
//! Play and Add to queue send the Podcasts commands with the frame's selected
//! episode — the core owns the selection, so the actions name the row the
//! frame painted and never a position.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiPodcastsDetail;
use crate::bridge::library::GuiLibraryCommand;
use crate::window::components::library_hero_actions::LibraryHeroActions;
use crate::window::components::library_hero_frame::{HeroBlock, HeroHeader, LibraryHeroFrame};
use crate::window::library::hero::{HeroLayoutFacts, hero_layout};
use crate::window::library::state::LibraryState;

/// The props of the Podcasts Hero (the `LibraryPodcastsHero.svelte` props,
/// narrowed to what the Hero reads): the library index the Podcasts commands
/// name, and the carried Podcasts detail — the selected episode and its Hero
/// facts.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryPodcastsHeroProps {
    /// The Podcasts library index the Podcasts commands name (design D2,
    /// D12).
    pub lib_idx: usize,
    /// The selected episode's Hero facts and the selection the actions name.
    pub detail: GuiPodcastsDetail,
}

/// The Podcasts action a button sends (the Svelte `command` union).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PodcastsAction {
    /// Play the selected episode.
    Play,
    /// Add the selected episode to the queue.
    Queue,
}

/// Send a Podcasts command for the frame's selected episode (design D12,
/// row 10.1): the row the frame painted, never a position. The command sends
/// only while the frame holds both the selection and a Hero.
fn act(library: &LibraryState, action: PodcastsAction, lib_idx: usize, detail: &GuiPodcastsDetail) {
    let (Some(item_id), Some(_)) = (&detail.selected, &detail.hero) else {
        return;
    };
    let item_id = item_id.clone();
    let command = match action {
        PodcastsAction::Play => GuiLibraryCommand::PodcastsPlay { lib_idx, item_id },
        PodcastsAction::Queue => GuiLibraryCommand::PodcastsQueue { lib_idx, item_id },
    };
    library.link.library(command);
}

/// The Podcasts Hero (the Svelte markup, character for character): the frame
/// over the selected episode, with no Workspace and no play-next control.
#[component]
pub fn LibraryPodcastsHero(props: LibraryPodcastsHeroProps) -> Element {
    let library = consume_context::<LibraryState>();

    // The Hero's facts, present exactly while the pane paints.
    let Some(ref hero) = props.detail.hero else {
        return rsx! {};
    };

    // The frame's header and block for the Hero (invariant 30).
    let facts = HeroLayoutFacts {
        title: &hero.title,
        parent: hero.parent.as_deref(),
        meta_rows: &hero.meta_rows,
        duration_row: hero.duration_row,
        progress_row: hero.progress_row,
        overview: hero.overview.as_deref(),
    };
    let layout = hero_layout(&facts, "podcast");

    // The actions' sends (design D12, row 10.1): the picked episode's own
    // stable id. There is no play-next command for podcast episodes.
    let lib_idx = props.lib_idx;
    let play_library = library.clone();
    let play_detail = props.detail.clone();
    let on_play = Callback::new(move |()| {
        act(&play_library, PodcastsAction::Play, lib_idx, &play_detail);
    });
    let queue_library = library.clone();
    let queue_detail = props.detail.clone();
    let on_queue = Callback::new(move |()| {
        act(
            &queue_library,
            PodcastsAction::Queue,
            lib_idx,
            &queue_detail,
        );
    });
    let actions = rsx! {
        LibraryHeroActions {
            attr: "podcasts",
            on_play: Some(on_play),
            on_queue: Some(on_queue),
        }
    };

    rsx! {
        LibraryHeroFrame {
            attr: "podcasts",
            root: "data-library-podcasts-hero",
            header: HeroHeader::from(layout.header),
            block: HeroBlock::from(layout.block),
            art_shape: hero.art_shape,
            art: hero.art.clone(),
            art_title: hero.title.clone(),
            actions: Some(actions),
        }
    }
}

#[cfg(test)]
mod tests;
