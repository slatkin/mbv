//! The Library Hero (`replace-tauri-with-blitz` task 7.5, the port of
//! `LibraryHero`; add-gui-library-panel design D5, D8): the canvas's
//! detail pane for the picked album — the artist header with Shuffle artist,
//! the album block with its square artwork and its Play, Play next, Add to
//! queue, and More actions controls, and the album's tracklist. The Wide
//! arrangement paints it as the pane beside the list, and the Narrow
//! arrangement paints the very same painters inside its overlay (design D4),
//! so the two cannot drift.
//!
//! Every control sends the command the core applies through its existing App
//! action (design D8): Play, Play next, Add to queue, and Shuffle artist name
//! stable Service ids, and Shuffle artist names the artist's albums in the
//! current union. The Hero's controls paint their keys (`n`, `a`).
//!
//! More actions opens the picked item's menu: the core builds the entries and
//! the frame carries them, so the window paints what the core can run. The
//! menu is offered only while the frame carries entries, because
//! `execute_context_action` resolves a leaf target through the TUI's library
//! cursor — the picked item is the one item the frame's entries are about.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{
    GuiArtShapeFrame, GuiLibraryDetail, GuiMetaKindFrame, GuiMetaSegmentFrame, GuiMusicArtistFrame,
    GuiMusicHeroFrame, GuiMusicMenuEntryFrame,
};
use crate::bridge::library::GuiLibraryCommand;
use crate::window::components::library_hero_actions::LibraryHeroActions;
use crate::window::components::library_hero_frame::{HeroBlock, HeroHeader, LibraryHeroFrame};
use crate::window::components::library_panel::row_menu_entries;
use crate::window::components::library_tracklist::LibraryTracklist;
use crate::window::components::queue_row_menu::QueueRowMenu;
use crate::window::icons::{Ellipsis, Shuffle};
use crate::window::library::hero::{album_facts, artist_facts, artist_union_albums, hero_kicker};
use crate::window::library::parts::{library_music_detail, library_music_list};
use crate::window::sections::SectionRef;

use crate::window::library::state::LibraryState;

/// The labels the Hero's own buttons already send (design D8): the menu's
/// The props of the Music Hero (the `LibraryHero` props, in order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryHeroProps {
    /// The carried Music content: the picked album's Hero facts, the id of
    /// the track playing at the playhead, and the Hero's square artwork
    /// (design D2, D5), as the one handle every reader clones
    /// (`share-gui-sections-by-rc` design D3).
    pub detail: SectionRef<GuiLibraryDetail>,
}

/// The Hero's play (design D8): the shared play-or-activate path, so the
/// album's tracks reach the Owner as one replacement.
fn play_album(library: &LibraryState, album_id: &str) {
    library.link.library(GuiLibraryCommand::Play {
        item_id: album_id.to_owned(),
    });
}

/// Admit the album directly after the active slot (design D8).
fn play_next_album(library: &LibraryState, album_id: &str) {
    library.link.library(GuiLibraryCommand::PlayNext {
        item_id: album_id.to_owned(),
    });
}

/// Append the album to the queue's tail (design D8).
fn add_album_to_queue(library: &LibraryState, album_id: &str) {
    library.link.library(GuiLibraryCommand::AddToQueue {
        item_id: album_id.to_owned(),
    });
}

/// Shuffle the artist (design D8): the albums of the current union, played
/// shuffled.
fn shuffle_artist(
    library: &LibraryState,
    artists: &[GuiMusicArtistFrame],
    hero: &GuiMusicHeroFrame,
) {
    library.link.library(GuiLibraryCommand::ShuffleArtist {
        album_ids: artist_union_albums(artists, hero),
    });
}

/// Run the menu entry the frame carried (design D8): the core rebuilds the
/// action for the picked item at that index. The menu closes behind the
/// press.
fn run_menu_entry(library: &LibraryState, album_id: &str, index: usize) {
    library.link.library(GuiLibraryCommand::MenuEntry {
        row: crate::gui_core::GuiLibraryMenuRow::Item(album_id.to_owned()),
        index,
    });
    library.close_library_item_menu();
}

/// Run the menu entry a press on the painted menu named (design D8): the
/// index the menu reports is the entry's own shared-builder index, and the
/// handler forwards it verbatim — never a position in the filtered list the
/// Hero paints, so an entry the dropped-duplicates rule re-numbered still
/// runs.
fn press_menu_entry(library: &LibraryState, album_id: &str, index: usize) {
    run_menu_entry(library, album_id, index);
}

/// The Music pair the shuffle reads at press time: the union's artists and
/// the picked album's Hero facts, cloned once per press — the closure reads
/// the current frame, so a Hero repaint never clones the union's albums or
/// the tracklist for it (the 7.3-7.4 review's whole-list finding).
fn current_union(library: &LibraryState) -> Option<(Vec<GuiMusicArtistFrame>, GuiMusicHeroFrame)> {
    let list = library.sections.library_list.peek();
    let detail = library.sections.library_detail.peek();
    let list = library_music_list(list.as_deref())?;
    let hero = library_music_detail(detail.as_deref())?.hero.as_ref()?;
    Some((list.artists.clone(), hero.clone()))
}

/// The Library Hero: the artist
/// header with Shuffle artist, the album block with its square artwork and
/// its action controls, and the album's tracklist.
#[component]
pub fn LibraryHero(props: LibraryHeroProps) -> Element {
    // The Hero's More-actions entries (design D8): the frame carries the
    // shared builder's entries in the builder's order — the order the core's
    // execution rebuild indexes. The entries the Hero's own buttons already
    // send (the Play entries and the add-to-queue entry) drop at paint time,
    // so the menu names only what no button offers, and an album whose
    // remainder is empty hides the control entirely.
    const HERO_BUTTON_LABELS: [&str; 3] = ["Play", "Play All", "Add to Queue"];

    let library = consume_context::<LibraryState>();

    // The Music part of the carried detail, narrowed from the handle in this
    // body (`share-gui-sections-by-rc` design D3), and the Hero's facts,
    // present exactly while the pane paints.
    let Some(detail) = library_music_detail(Some(&props.detail)) else {
        return rsx! {};
    };
    let Some(hero) = detail.hero.as_ref() else {
        return rsx! {};
    };

    // The actions' sends (design D8): the picked album's own stable id.
    let album_id = hero.album.album_id.clone();
    let play_library = library.clone();
    let on_play = Callback::new(move |()| play_album(&play_library, &album_id));
    let next_library = library.clone();
    let album_id = hero.album.album_id.clone();
    let on_play_next = Callback::new(move |()| play_next_album(&next_library, &album_id));
    let queue_library = library.clone();
    let album_id = hero.album.album_id.clone();
    let on_queue = Callback::new(move |()| add_album_to_queue(&queue_library, &album_id));

    let menu: Vec<GuiMusicMenuEntryFrame> = hero
        .menu
        .iter()
        .filter(|entry| !HERO_BUTTON_LABELS.contains(&entry.label.as_str()))
        .cloned()
        .collect();

    // Shuffle the artist (design D8): the albums of the current union, played
    // shuffled.
    let shuffle_library = library.clone();
    let header_action = rsx! {
        button {
            "type": "button",
            class: "flex flex-none items-center",
            style: "height: 40px; padding: 0 14px; gap: 8px; background: none; border: 1px solid var(--canvas-library-control); border-radius: 6px; color: var(--canvas-text); font-size: 14px; cursor: pointer;",
            "data-library-hero-shuffle": "",
            onclick: move |_| {
                let Some((artists, hero)) = current_union(&shuffle_library) else {
                    return;
                };
                shuffle_artist(&shuffle_library, &artists, &hero);
            },
            Shuffle { size: 15 }
            "Shuffle artist"
        }
    };

    // The actions row (design D8): the shared actions painter, the picked
    // item's More actions, and its menu.
    let item_menu_open = library.ui.cloned().item_menu_open;
    let menu_open_library = library.clone();
    let menu_library = library.clone();
    // The menu's press names the picked album, cloned once for the callback —
    // the three play closures each moved their own copy.
    let album_id = hero.album.album_id.clone();
    let on_select =
        Callback::new(move |index: usize| press_menu_entry(&menu_library, &album_id, index));
    let actions = rsx! {
        LibraryHeroActions {
            attr: "",
            on_play: Some(on_play),
            on_play_next: Some(on_play_next),
            on_queue: Some(on_queue),
        }
        if !menu.is_empty() {
            button {
                "type": "button",
                class: "flex items-center justify-center",
                style: "width: 44px; height: 44px; background: none; border: 0; color: var(--canvas-muted); cursor: pointer;",
                "aria-label": "More album actions",
                "aria-expanded": "{item_menu_open}",
                "data-library-hero-more": "",
                onclick: move |_| menu_open_library.open_library_item_menu(),
                Ellipsis { size: 16 }
            }
        }
        if item_menu_open && !menu.is_empty() {
            QueueRowMenu {
                cursor: None,
                entries: row_menu_entries(&menu),
                top: "52px".to_owned(),
                right: Some("0".to_owned()),
                on_select: Some(on_select),
            }
        }
    };

    rsx! {
        LibraryHeroFrame {
            attr: "",
            root: "data-library-hero",
            header: HeroHeader {
                kicker: hero_kicker(hero),
                title: hero.artist.clone(),
                facts: artist_facts(hero),
            },
            header_action: Some(header_action),
            block: HeroBlock {
                label: hero.album.name.clone(),
                eyebrow: Some(vec![GuiMetaSegmentFrame {
                    text: hero.album.year.clone(),
                    kind: GuiMetaKindFrame::Date,
                }]),
                name: Some(hero.album.name.clone()),
                facts: Some(album_facts(hero)),
                meta: Vec::new(),
                overview: None,
            },
            art_shape: GuiArtShapeFrame::Square,
            art: detail.art.clone(),
            art_title: hero.album.name.clone(),
            art_placeholder: Some("[Album art · Emby Primary]".to_owned()),
            actions: Some(actions),
            children: Some(rsx! {
                LibraryTracklist {
                    tracks: hero.album.tracks.clone().unwrap_or_default(),
                    playing_track_id: detail.playing_track_id.clone(),
                }
            }),
        }
    }
}

#[cfg(test)]
mod tests;
