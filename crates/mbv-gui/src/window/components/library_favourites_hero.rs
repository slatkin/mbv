//! The Favourites Hero (emby-favourites design D6): the selected favourite's
//! detail pane on the shared frame, for the Music, Shows, and plain Emby
//! destinations while their view is Favourites.
//!
//! The Hero's facts ride the Favourites list part, not the destination's detail
//! part (design D6): the core builds every favourite's Hero as the Home Hero
//! builds an Emby item's — the declared artwork and the cast, with no
//! `hero_item` gate — so a favourite series, boxset, album, or artist paints one
//! though the plain library's gate excludes folders and non-movies. The
//! Favourites view therefore paints this Hero in place of the destination's own,
//! which carries no Hero while that view is active.
//!
//! Play and Enqueue send the shared item commands with the list's own pick
//! (design D6): the core resolves the id against the favourites store while the
//! view is Favourites, so a favourite music artist row plays its tracks and a
//! favourite folder plays through the folder-play rule (design D8).

use dioxus_native::prelude::*;

use super::library_hero::hero_backdrop;
use crate::art::backdrop::BackdropStyle;
use crate::bridge::frame::library::{GuiArtShapeFrame, GuiLibraryList};
use crate::window::components::library_hero_actions::LibraryHeroActions;
use crate::window::components::library_hero_frame::{HeroBlock, HeroHeader, LibraryHeroFrame};
use crate::window::library::favourites::favourites_part;
use crate::window::library::state::LibraryState;
use crate::window::sections::SectionRef;

/// The props of the Favourites Hero (design D6): the carried Library list, whose
/// Favourites part holds the picked row and its Hero facts, as the one handle
/// every reader clones (`share-gui-sections-by-rc` design D3).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryFavouritesHeroProps {
    /// The carried list section the Favourites part rides.
    pub list: SectionRef<GuiLibraryList>,
}

/// The Favourites Hero: the shared Hero frame over the selected favourite.
#[component]
pub fn LibraryFavouritesHero(props: LibraryFavouritesHeroProps) -> Element {
    let library = consume_context::<LibraryState>();

    // The Favourites part of the carried list, and the picked row's Hero facts,
    // present exactly while the pane paints.
    let Some((_site, part)) = favourites_part(Some(&props.list)) else {
        return rsx! {};
    };
    let Some(ref hero) = part.hero else {
        return rsx! {};
    };
    let selected = part.selected.clone();

    // The header band (invariant 30): the item is its own largest unit, so it
    // titles the header.
    let header = HeroHeader {
        title: hero.title.clone(),
        pills: hero.facts.pills.clone(),
    };

    // The detail block's text: the item's own title is the block's label and no
    // name line paints below it, because no parent took the header.
    let block = HeroBlock {
        label: hero.title.clone(),
        eyebrow: None,
        name: None,
        name_episode: false,
        pills: Vec::new(),
        rows: hero.facts.rows.clone(),
    };

    // The actions' sends (design D6): the shared item commands over the list's
    // own pick, so the Hero and the row's own paths resolve one id the same way.
    let play_library = library.clone();
    let play_id = selected.clone();
    let on_play = Callback::new(move |()| {
        if let Some(item_id) = &play_id {
            play_library.play_favourites_row(item_id);
        }
    });
    let queue_library = library.clone();
    let queue_id = selected;
    let on_queue = Callback::new(move |()| {
        if let Some(item_id) = &queue_id {
            queue_library.queue_favourites_row(item_id);
        }
    });
    let actions = rsx! {
        LibraryHeroActions {
            attr: "favourites",
            on_play: Some(on_play),
            on_queue: Some(on_queue),
        }
    };

    // The header band's backdrop (design D4): the shared art-to-backdrop mapping
    // over the Hero's own art, `None` while the art is absent, still loading, or
    // the bundled placeholder. A square item — an album or an artist — takes the
    // music mapping, and every other shape the video one.
    let style = match hero.art_shape {
        GuiArtShapeFrame::Square => BackdropStyle::Music,
        GuiArtShapeFrame::Landscape | GuiArtShapeFrame::Portrait => BackdropStyle::Video,
    };
    let backdrop = hero_backdrop(hero.art.as_ref(), style);

    rsx! {
        LibraryHeroFrame {
            attr: "favourites",
            root: "data-library-favourites-hero",
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
