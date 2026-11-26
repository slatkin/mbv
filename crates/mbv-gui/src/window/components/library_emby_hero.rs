//! The plain Emby-library Hero (gui-hero-design-polish design D2, D3, D4):
//! the frame (`LibraryHeroFrame`) over the selected row. The header band
//! carries the item's kind word as the kicker, the title, and the fact pills
//! — the `IMDb` link is a link pill, as every Hero's link fact is. The detail
//! block beside the art holds the labelled fact rows and the action row; the
//! Overview and Cast sections follow when the item has them; the glow behind
//! the header band is the shared art-to-glow mapping over the Hero's own art.
//!
//! A plain Emby library carries no Workspace and no tracklist: a folder or a
//! boxset plays through the TUI's folder-play rule, so the pane is the frame
//! and no list band opens.
//!
//! Play and Add to queue (`a`) send the Emby destination's commands with the
//! frame's selected row (add-gui-library-panel design D12) — the core owns
//! the selection, so the actions name the row the frame painted and never a
//! position, and the destination's list part names the library index.

use dioxus_native::prelude::*;

use super::library_hero::hero_glow;
use crate::bridge::frame::library::GuiLibraryDetail;
use crate::window::components::library_hero_actions::LibraryHeroActions;
use crate::window::components::library_hero_frame::{HeroBlock, HeroHeader, LibraryHeroFrame};
use crate::window::library::hero::{hero_command, label_segments};
use crate::window::library::lettered::{EmbyList, LetteredDestination};
use crate::window::library::parts::library_emby_detail;
use crate::window::library::state::LibraryState;
use crate::window::sections::SectionRef;

/// The props of the Emby Hero (the `LibraryEmbyHero` props, narrowed
/// to what the Hero reads): the carried plain Emby-library detail — the
/// selected row and its Hero facts (add-gui-library-panel design D12, row
/// 8.1).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryEmbyHeroProps {
    /// The selected row's Hero facts and the selection the actions name, as
    /// the one handle every reader clones (`share-gui-sections-by-rc` design
    /// D3).
    pub detail: SectionRef<GuiLibraryDetail>,
}

/// The plain Emby-library Hero:
/// the frame over the selected row — the kind kicker and the fact pills in
/// the header band, the labelled rows in the detail block, and the Overview
/// and Cast sections when the item has them.
#[component]
pub fn LibraryEmbyHero(props: LibraryEmbyHeroProps) -> Element {
    let library = consume_context::<LibraryState>();

    // The plain Emby-library part of the carried detail, narrowed from the
    // handle in this body, and the Hero's facts, present exactly while the
    // pane paints.
    let Some(detail) = library_emby_detail(Some(&props.detail)) else {
        return rsx! {};
    };
    let Some(ref hero) = detail.hero else {
        return rsx! {};
    };

    // The header band (invariant 30): the item is its own largest unit, so
    // it titles the header, and its kind word in upper case is the kicker.
    let header = HeroHeader {
        kicker: label_segments(&hero.kind.to_uppercase()),
        title: hero.title.clone(),
        pills: hero.facts.pills.clone(),
    };

    // The detail block's text: the item's own title is the block's label and
    // no name line paints below it, because no parent took the header.
    let block = HeroBlock {
        label: hero.title.clone(),
        eyebrow: None,
        name: None,
        name_episode: false,
        pills: Vec::new(),
        rows: hero.facts.rows.clone(),
    };

    // The actions' sends (add-gui-library-panel design D12, row 8.1): the
    // Emby destination's commands over the picked row's own stable id.
    let play_library = library.clone();
    let play_detail = props.detail.clone();
    let on_play = Callback::new(move |()| {
        hero_command::<EmbyList>(&play_library, &play_detail, EmbyList::play);
    });
    let queue_library = library.clone();
    let queue_detail = props.detail.clone();
    let on_queue = Callback::new(move |()| {
        hero_command::<EmbyList>(&queue_library, &queue_detail, EmbyList::queue);
    });
    let actions = rsx! {
        LibraryHeroActions {
            attr: "emby",
            on_play: Some(on_play),
            on_queue: Some(on_queue),
        }
    };

    // The header band's glow (design D4): the shared art-to-glow mapping over
    // the Hero's own art, `None` while the art is absent, still loading, or
    // the bundled placeholder.
    let glow = hero_glow(hero.art.as_ref());

    rsx! {
        LibraryHeroFrame {
            attr: "emby",
            root: "data-library-emby-hero",
            header,
            block,
            art_shape: hero.art_shape,
            art: hero.art.clone(),
            art_title: hero.title.clone(),
            actions: Some(actions),
            overview: hero.overview.clone(),
            cast: hero.cast.clone(),
            glow,
        }
    }
}

#[cfg(test)]
mod tests;
