//! The plain Emby-library Hero (`replace-tauri-with-blitz` task 7.6, the port
//! of `LibraryEmbyHero`; add-gui-library-panel design D12, row 8.1,
//! and D17): the Music Hero's frame (`LibraryHeroFrame`) over the selected
//! row, as the TUI paints every Emby item through the one
//! `hero_content_emby` producer. A movie is its own largest unit, so the
//! header band carries the item's kind word as the kicker, the title, and the
//! plain facts on one line; the block beside the art holds the duration and
//! progress rows, the action row, and the overview; the movie's credits table
//! is the list band.
//!
//! The `IMDb` meta row is a link the header's facts string cannot hold, so
//! the Hero maps it out of the facts it hands `hero_layout` (invariant 30:
//! the piece is never bent) and paints it as the header's trailing action —
//! the `IMDb` button sends the core's open-url command with the url the frame
//! carries, as the TUI's link opens it (the core scheme-gates the url).
//!
//! A plain Emby library carries no Workspace and no tracklist: a folder or a
//! boxset plays through the TUI's folder-play rule, so the pane is the frame
//! and no nested list opens.
//!
//! Play, Play next (`n`), and Add to queue (`a`) send the Emby destination's
//! commands with the frame's selected row (design D12) — the core owns the
//! selection, so the actions name the row the frame painted and never a
//! position, and the destination's list part names the library index.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{
    GuiEmbyHeroCreditFrame, GuiEmbyHeroFrame, GuiLibraryDetail, GuiMetaSegmentFrame,
};
use crate::bridge::library::GuiLibraryCommand;
use crate::window::components::library_hero_actions::LibraryHeroActions;
use crate::window::components::library_hero_frame::{HeroBlock, HeroHeader, LibraryHeroFrame, own};
use crate::window::library::hero::{HeroLayoutFacts, MetaRow, hero_command, hero_layout};
use crate::window::library::lettered::{EmbyList, LetteredDestination};
use crate::window::library::parts::library_emby_detail;
use crate::window::library::state::LibraryState;
use crate::window::sections::SectionRef;

/// The props of the Emby Hero (the `LibraryEmbyHero` props, narrowed
/// to what the Hero reads): the carried plain Emby-library detail — the
/// selected row and its Hero facts (design D12, row 8.1).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryEmbyHeroProps {
    /// The selected row's Hero facts and the selection the actions name, as
    /// the one handle every reader clones (`share-gui-sections-by-rc` design
    /// D3).
    pub detail: SectionRef<GuiLibraryDetail>,
}

/// The text of one meta row, as the `IMDb` row's filter reads it.
fn meta_row_text(row: &[GuiMetaSegmentFrame]) -> String {
    row.iter().map(|segment| segment.text.as_str()).collect()
}

/// The facts `hero_layout` maps: the `IMDb` meta
/// row out of the plain rows (the header action paints it as a link), with
/// the duration and progress indices remapped to the rows that remain.
fn layout_facts(facts: &GuiEmbyHeroFrame) -> (Vec<MetaRow>, Option<usize>, Option<usize>) {
    let mut meta_rows: Vec<MetaRow> = Vec::new();
    let mut duration_row = None;
    let mut progress_row = None;
    for (index, row) in facts.meta_rows.iter().enumerate() {
        if facts.imdb_url.is_some() && meta_row_text(row) == "IMDb" {
            continue;
        }
        if Some(index) == facts.duration_row {
            duration_row = Some(meta_rows.len());
        }
        if Some(index) == facts.progress_row {
            progress_row = Some(meta_rows.len());
        }
        meta_rows.push(row.clone());
    }
    (meta_rows, duration_row, progress_row)
}

/// Open the Hero's `IMDb` link in the user's browser (design D17, Q7): the
/// open-url command with the url the frame carries; the core opens only an
/// http or https URL.
fn open_imdb(library: &LibraryState, url: &str) {
    library.link.library(GuiLibraryCommand::OpenUrl {
        url: url.to_owned(),
    });
}

/// One row of the credits table: the name at the
/// left, the role at the right, the TUI's every-other-row stripe.
fn paint_credit(index: usize, credit: &GuiEmbyHeroCreditFrame) -> Element {
    let stripe = if index % 2 == 1 {
        "var(--canvas-library-raise)"
    } else {
        "none"
    };
    rsx! {
        div {
            key: "{index}",
            class: "flex items-baseline justify-between",
            style: "gap: 24px; padding: 4px 8px; background: {stripe};",
            ..vec![Attribute::new(own("emby", "credit"), "", None, false)],
            span {
                class: "min-w-0",
                style: "font-size: 14px; color: var(--canvas-text);",
                ..vec![Attribute::new(own("emby", "credit-name"), "", None, false)],
                "{credit.name}"
            }
            span {
                class: "font-mono",
                style: "font-size: 13px; color: var(--canvas-muted);",
                ..vec![Attribute::new(own("emby", "credit-role"), "", None, false)],
                "{credit.role}"
            }
        }
    }
}

/// The plain Emby-library Hero:
/// the frame over the selected row, with the `IMDb` link as the header's
/// trailing action and the credits table as the list band.
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

    // The frame's header and block for the Hero (invariant 30): the item is
    // its own largest unit, so it titles the header, and its kind word in
    // upper case is the kicker.
    let (meta_rows, duration_row, progress_row) = layout_facts(hero);
    let facts = HeroLayoutFacts {
        title: &hero.title,
        parent: None,
        meta_rows: &meta_rows,
        duration_row,
        progress_row,
        overview: hero.overview.as_deref(),
    };
    let layout = hero_layout(&facts, &hero.kind.to_uppercase());

    // The IMDb link (design D17, Q7): the TUI underlines the `IMDb` meta row
    // and a press opens the url in the user's browser.
    let imdb_library = library.clone();
    let header_action = hero.imdb_url.clone().map(|url| {
        let imdb_library = imdb_library.clone();
        rsx! {
            button {
                "type": "button",
                class: "font-mono",
                style: "font-size: 13px; padding: 0; background: none; border: 0; text-decoration: underline; color: var(--canvas-muted); cursor: pointer;",
                "data-library-emby-imdb": "{url}",
                onclick: move |_| open_imdb(&imdb_library, &url),
                "IMDb"
            }
        }
    });

    // The actions' sends (design D12, row 8.1): the Emby destination's
    // commands over the picked row's own stable id.
    let play_library = library.clone();
    let play_detail = props.detail.clone();
    let on_play = Callback::new(move |()| {
        hero_command::<EmbyList>(&play_library, &play_detail, EmbyList::play);
    });
    let next_library = library.clone();
    let next_detail = props.detail.clone();
    let on_play_next = Callback::new(move |()| {
        hero_command::<EmbyList>(&next_library, &next_detail, EmbyList::play_next);
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
            on_play_next: Some(on_play_next),
            on_queue: Some(on_queue),
        }
    };

    // The movie's credits table (design D17, Q7): the list band under the
    // overview, the director first as the TUI's table paints it.
    let children = (!hero.credits.is_empty()).then(|| {
        rsx! {
            section {
                class: "flex flex-col",
                "aria-label": "Credits",
                ..vec![Attribute::new(own("emby", "credits"), "", None, false)],
                for (index, credit) in hero.credits.iter().enumerate() {
                    { paint_credit(index, credit) }
                }
            }
        }
    });

    rsx! {
        LibraryHeroFrame {
            attr: "emby",
            root: "data-library-emby-hero",
            header: HeroHeader::from(layout.header),
            header_action,
            block: HeroBlock::from(layout.block),
            art_shape: hero.art_shape,
            art: hero.art.clone(),
            art_title: hero.title.clone(),
            actions: Some(actions),
            children,
        }
    }
}

#[cfg(test)]
mod tests;
