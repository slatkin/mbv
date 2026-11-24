//! The Podcasts Hero (`replace-tauri-with-blitz` task 7.6, the port of
//! `LibraryPodcastsHero`; add-gui-library-panel design D12, row 10.1):
//! the selected episode's detail pane in the Emby Hero look — the artwork box
//! the `artShape` arm names, the title, the meta rows with their duration and
//! progress roles, and the episode description as the overview.
//!
//! A podcast Hero is episode information only: it carries no Workspace and no
//! tracklist. There is no play-next either: `n` is consumed by the window
//! and does nothing for podcast episodes (design D12, row 10.1), so only Play
//! and Add to queue paint.
//!
//! Play and Add to queue send the Podcasts destination's commands with the
//! frame's selected episode (design D12) — the core owns the selection, so the
//! actions name the row the frame painted and never a position, and the
//! destination's list part names the library index.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiLibraryDetail;
use crate::window::components::library_hero_actions::LibraryHeroActions;
use crate::window::components::library_hero_frame::{HeroBlock, HeroHeader, LibraryHeroFrame};
use crate::window::library::hero::{HeroLayoutFacts, hero_command, hero_layout};
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

/// The Podcasts Hero: the frame
/// over the selected episode, with no Workspace and no play-next control.
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

    // The frame's header and block for the Hero (invariant 30).
    let facts = HeroLayoutFacts {
        title: &hero.title,
        parent: hero.parent.as_deref(),
        meta_rows: &hero.meta_rows,
        duration_row: hero.duration_row,
        progress_row: hero.progress_row,
        overview: hero.overview.as_deref(),
        // A Podcasts Hero's item is always an episode.
        episode: true,
    };
    let layout = hero_layout(&facts, "podcast");

    // The actions' sends (design D12, row 10.1): the Podcasts destination's
    // commands over the picked episode's own stable id. There is no play-next
    // command for podcast episodes.
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
