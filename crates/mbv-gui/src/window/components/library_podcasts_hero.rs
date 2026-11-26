//! The Podcasts Hero (`replace-tauri-with-blitz` task 7.6, the port of
//! `LibraryPodcastsHero`; add-gui-library-panel design D12, row 10.1;
//! gui-hero-design-polish design D2, D4): the selected episode's detail pane
//! on the shared frame — the artwork box the `artShape` arm names, the
//! podcast's name as the header title, the episode in the block with its
//! publish date row, the duration as a fact pill, and the episode
//! description as the Overview section. No Episodes list band paints: a
//! podcast Hero is episode information only, and the episodes stay in the
//! Library list (design D2). Only Play and Add to queue paint.
//!
//! Play and Add to queue send the Podcasts destination's commands with the
//! frame's selected episode (design D12) — the core owns the selection, so the
//! actions name the row the frame painted and never a position, and the
//! destination's list part names the library index.

use dioxus_native::prelude::*;

use super::library_hero::hero_backdrop;
use crate::art::backdrop::BackdropStyle;
use crate::bridge::frame::library::GuiLibraryDetail;
use crate::window::components::library_hero_actions::LibraryHeroActions;
use crate::window::components::library_hero_frame::{HeroBlock, HeroHeader, LibraryHeroFrame};
use crate::window::library::hero::hero_command;
use crate::window::library::lettered::{LetteredDestination, PodcastsList};
use crate::window::library::parts::library_podcasts_detail;
use crate::window::library::state::LibraryState;
use crate::window::sections::SectionRef;

/// The props of the Podcasts Hero (the `LibraryPodcastsHero` props,
/// narrowed to what the Hero reads): the carried Podcasts detail — the
/// selected episode and its Hero facts.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryPodcastsHeroProps {
    /// The selected episode's Hero facts and the selection the actions name,
    /// as the one handle every reader clones (`share-gui-sections-by-rc`
    /// design D3).
    pub detail: SectionRef<GuiLibraryDetail>,
}

/// The Podcasts Hero: the frame over the selected episode, with no
/// Workspace.
#[component]
pub fn LibraryPodcastsHero(props: LibraryPodcastsHeroProps) -> Element {
    let library = consume_context::<LibraryState>();

    // The Podcasts part of the carried detail, narrowed from the handle in
    // this body, and the Hero's facts, present exactly while the pane paints.
    let Some(detail) = library_podcasts_detail(Some(&props.detail)) else {
        return rsx! {};
    };
    let Some(ref hero) = detail.hero else {
        return rsx! {};
    };

    // The frame's header and block for the Hero (invariant 30): the podcast
    // is the largest unit and titles the header, and the episode is the
    // block's name line, one step smaller. No list title and no children
    // pass — the Hero paints no list band (design D2).
    let parent = hero.parent.as_deref().filter(|parent| !parent.is_empty());
    let header = HeroHeader {
        title: parent.map_or_else(|| hero.title.clone(), ToString::to_string),
        pills: hero.facts.pills.clone(),
    };
    let block = HeroBlock {
        label: hero.title.clone(),
        eyebrow: None,
        name: parent.map(|_| hero.title.clone()),
        // A Podcasts Hero's item is always an episode.
        name_episode: true,
        pills: Vec::new(),
        rows: hero.facts.rows.clone(),
    };

    // The actions' sends (design D12, row 10.1): the Podcasts destination's
    // commands over the picked episode's own stable id.
    let play_library = library.clone();
    let play_detail = props.detail.clone();
    let on_play = Callback::new(move |()| {
        hero_command::<PodcastsList>(&play_library, &play_detail, PodcastsList::play);
    });
    let queue_library = library.clone();
    let queue_detail = props.detail.clone();
    let on_queue = Callback::new(move |()| {
        hero_command::<PodcastsList>(&queue_library, &queue_detail, PodcastsList::queue);
    });
    let actions = rsx! {
        LibraryHeroActions {
            attr: "podcasts",
            on_play: Some(on_play),
            on_queue: Some(on_queue),
        }
    };

    // The header band's backdrop (design D4): the shared art-to-backdrop mapping over
    // the Hero's own art, `None` while the art is absent, still loading, or
    // the bundled placeholder.
    let backdrop = hero_backdrop(hero.art.as_ref(), BackdropStyle::Video);

    rsx! {
        LibraryHeroFrame {
            attr: "podcasts",
            root: "data-library-podcasts-hero",
            header,
            block,
            art_shape: hero.art_shape,
            art: hero.art.clone(),
            art_title: hero.title.clone(),
            actions: Some(actions),
            overview: hero.overview.clone(),
            backdrop,
        }
    }
}

#[cfg(test)]
mod tests;
