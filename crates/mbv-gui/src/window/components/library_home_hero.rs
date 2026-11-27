//! The Home Hero (`LibraryHomeHero`; add-gui-library-panel design D12,
//! gui-hero-design-polish design D2): the shared frame (`LibraryHeroFrame`)
//! over the selected row, with `attr: "home"` so the Hero keeps its
//! `data-library-home-hero-*` hooks. The header band titles the largest unit
//! the item belongs to: the parent (a series, a show, or a feed) when the
//! payload names one, else the item itself. When a parent took the title, the
//! detail block names the item beneath it. The facts map onto the header's
//! pills and the block's labelled rows, the cast and the overview paint
//! under their sections, and the backdrop comes from the shared art-to-backdrop
//! mapping. Home has no list band.
//!
//! Play and Enqueue (`a`) send the Home destination's row commands with
//! the frame's selected row (design D12) — the core owns the selection, so the
//! actions name the row the frame painted and never a position.

use dioxus_native::prelude::*;

use super::library_hero::hero_backdrop;
use crate::art::backdrop::BackdropStyle;
use crate::bridge::frame::library::{GuiHomeHeroFrame, GuiHomeRowIdFrame, GuiLibraryDetail};
use crate::window::components::library_hero_actions::LibraryHeroActions;
use crate::window::components::library_hero_frame::{HeroBlock, HeroHeader, LibraryHeroFrame};
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

/// The frame's header and block for the Home Hero (invariant 30): a parent
/// titles the header, and the item's own name paints in the block's name line,
/// one step smaller. With no parent the item titles the header and the block
/// names nothing.
fn header_and_block(hero: &GuiHomeHeroFrame) -> (HeroHeader, HeroBlock) {
    let parent = hero.parent.as_deref().filter(|parent| !parent.is_empty());
    (
        HeroHeader {
            title: parent.map_or_else(|| hero.title.clone(), ToString::to_string),
            pills: hero.facts.pills.clone(),
        },
        HeroBlock {
            label: hero.title.clone(),
            eyebrow: None,
            name: parent.map(|_| hero.title.clone()),
            name_episode: parent.is_some(),
            pills: Vec::new(),
            rows: hero.facts.rows.clone(),
        },
    )
}

/// The Home Hero: the frame over the selected row, with the facts, the cast,
/// and the overview, and no list band.
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

    let (header, block) = header_and_block(&hero);
    let actions = rsx! {
        LibraryHeroActions {
            attr: "home",
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
            attr: "home",
            root: "data-library-home-hero",
            header,
            block,
            art_shape: hero.art_shape,
            art: hero.art.clone(),
            art_title: hero.title.clone(),
            actions: Some(actions),
            overview: hero.overview.clone(),
            cast: hero.cast.clone(),
            backdrop,
        }
    }
}

#[cfg(test)]
mod tests;
