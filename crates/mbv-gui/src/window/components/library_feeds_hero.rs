//! The Feeds Hero (`replace-tauri-with-blitz` task 7.6, the port of
//! `LibraryFeedsHero`; add-gui-library-panel design D12, row 11.1;
//! gui-hero-design-polish design D2, D4): the selected entry's detail pane on
//! the shared frame — the artwork box the `artShape` arm names, the feed's
//! name as the header title, the entry in the block with its publish date
//! row, and the duration as a fact pill.
//!
//! A feed entry has no artwork source and no overview, so the box paints the
//! window's placeholder for the shape and no Overview section paints. The
//! feed's entries are the list band under the Entries section.
//!
//! Play and Add to queue send the Feeds destination's commands with the
//! frame's selected entry (design D12) — the core owns the selection, so the
//! actions name the row the frame painted and never a position. A Client holds
//! one feed tab, so no library index rides the commands.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiEntryRowFrame, GuiLibraryDetail, GuiLibraryList};
use crate::window::components::library_hero_actions::LibraryHeroActions;
use crate::window::components::library_hero_frame::{HeroBlock, HeroHeader, LibraryHeroFrame};
use crate::window::components::library_workspace_table::{LibraryWorkspaceTable, WorkspaceRow};
use crate::window::library::hero::{hero_command, label_segments};
use crate::window::library::lettered::{FeedsList, LetteredDestination};
use crate::window::library::parts::{library_feeds_detail, library_feeds_list};
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::PointerList;
use crate::window::sections::SectionRef;

/// The props of the Feeds Hero (the `LibraryFeedsHero` props, narrowed
/// to what the Hero reads): the carried Feeds rows — the checked
/// collections' rows,
/// which the list band filters to the Hero's feed — and the selected entry's
/// Hero facts.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryFeedsHeroProps {
    /// The checked collections' entry rows (design D12, row 11.1): the carried
    /// list,
    /// as the one handle every reader clones, which the list band narrows to
    /// the rows that belong to the Hero's feed (`share-gui-sections-by-rc`
    /// design D3).
    pub list: SectionRef<GuiLibraryList>,
    /// The selected entry's Hero facts and the selection the actions name.
    pub detail: SectionRef<GuiLibraryDetail>,
}

/// The Feeds Hero: the frame
/// over the selected entry, with the feed's entries as the list band.
#[component]
pub fn LibraryFeedsHero(props: LibraryFeedsHeroProps) -> Element {
    let library = consume_context::<LibraryState>();

    // The Feeds part of the carried detail, narrowed from the handle in this
    // body, and the Hero's facts, present exactly while the pane paints.
    let Some(detail) = library_feeds_detail(Some(&props.detail)) else {
        return rsx! {};
    };
    let Some(ref hero) = detail.hero else {
        return rsx! {};
    };

    // The frame's header and block for the Hero (invariant 30): the feed is
    // the largest unit and titles the header, and the entry is the block's
    // name line, one step smaller.
    let parent = hero.parent.as_deref().filter(|parent| !parent.is_empty());
    let header = HeroHeader {
        kicker: label_segments("feed"),
        title: parent.map_or_else(|| hero.title.clone(), ToString::to_string),
        pills: hero.facts.pills.clone(),
    };
    let block = HeroBlock {
        label: hero.title.clone(),
        eyebrow: None,
        name: parent.map(|_| hero.title.clone()),
        // A Feeds Hero's item is an entry, which paints as an episode does.
        name_episode: true,
        pills: Vec::new(),
        rows: hero.facts.rows.clone(),
    };

    // The feed's entries, newest first, as the Workspace table paints them:
    // the checked collections' rows that belong to the Hero's feed. A date
    // takes the
    // time cell. The rows are narrowed from the list handle, so a Hero repaint
    // never clones them (`share-gui-sections-by-rc` design D3).
    let rows: &[GuiEntryRowFrame] =
        library_feeds_list(Some(&props.list)).map_or(&[][..], |list| list.rows.as_slice());
    let playing_entry_id = detail.playing_entry_id.as_deref();
    let entry_rows: Vec<WorkspaceRow> = match hero.parent.as_deref() {
        None => Vec::new(),
        Some(parent) => rows
            .iter()
            .filter(|row| row.subtitle.as_deref() == Some(parent))
            .enumerate()
            .map(|(index, row)| WorkspaceRow {
                id: row.id.clone(),
                number: (index + 1).to_string(),
                name: row.title.clone(),
                duration: row.date.clone().unwrap_or_default(),
                playing: playing_entry_id == Some(row.id.as_str()),
                state: Some(row.state.clone()),
            })
            .collect(),
    };

    // The actions' sends (design D12, row 11.1): the Feeds destination's
    // commands over the picked entry's own stable id.
    let play_library = library.clone();
    let play_detail = props.detail.clone();
    let on_play = Callback::new(move |()| {
        hero_command::<FeedsList>(&play_library, &play_detail, FeedsList::play);
    });
    let queue_library = library.clone();
    let queue_detail = props.detail.clone();
    let on_queue = Callback::new(move |()| {
        hero_command::<FeedsList>(&queue_library, &queue_detail, FeedsList::queue);
    });
    let actions = rsx! {
        LibraryHeroActions {
            attr: "feeds",
            on_play: Some(on_play),
            on_queue: Some(on_queue),
        }
    };

    // The list band's press and play (design D12): a press selects the entry
    // the Hero shows, and the row's play gestures send that entry.
    let press_library = library.clone();
    let on_press = Callback::new(move |row_id: String| {
        press_library.select_feeds_row_id(&row_id);
    });
    let play_library = library.clone();
    let on_play = Callback::new(move |item_id: String| {
        play_library.play_feeds_row(&item_id);
    });

    // The row's hover Add (gui-row-hover-actions D5): add the entry to the
    // queue.
    let queue_library = library.clone();
    let on_queue = Callback::new(move |item_id: String| {
        queue_library.queue_feeds_row(&item_id);
    });

    rsx! {
        LibraryHeroFrame {
            attr: "feeds",
            root: "data-library-feeds-hero",
            header,
            block,
            art_shape: hero.art_shape,
            art: None,
            art_title: hero.title.clone(),
            actions: Some(actions),
            // A feed entry has no artwork source, so no glow maps (design D4).
            list_title: Some("Entries".to_owned()),
            children: Some(rsx! {
                LibraryWorkspaceTable {
                    root: "data-library-feeds-entries",
                    label: "Entries",
                    hook: "feeds-entry",
                    noun: "Entry",
                    pointer_list: PointerList::Workspace,
                    rows: entry_rows,
                    on_press,
                    on_play,
                    on_queue: Some(on_queue),
                }
            }),
        }
    }
}

#[cfg(test)]
mod tests;
