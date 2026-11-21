//! The Feeds Hero (`replace-tauri-with-blitz` task 7.6, the port of
//! `LibraryFeedsHero`; add-gui-library-panel design D12, row 11.1):
//! the selected entry's detail pane in the Emby Hero look — the artwork box
//! the `artShape` arm names, the title, and the meta rows with their duration
//! role.
//!
//! A feed entry has no artwork source and no overview, so the box paints the
//! window's placeholder for the shape and no overview paragraph paints. There
//! is no Workspace and no tracklist. Play next is not a Feeds command
//! (row 11.1), so only Play and Add to queue paint.
//!
//! Play and Add to queue send the Feeds destination's commands with the
//! frame's selected entry (design D12) — the core owns the selection, so the
//! actions name the row the frame painted and never a position. A Client holds
//! one feed tab, so no library index rides the commands.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{
    GuiEntryRowFrame, GuiHomeRowStateFrame, GuiLibraryDetail, GuiLibraryList,
};
use crate::window::components::library_hero_actions::LibraryHeroActions;
use crate::window::components::library_hero_frame::{HeroBlock, HeroHeader, LibraryHeroFrame};
use crate::window::components::library_workspace_table::{
    LibraryWorkspaceTable, WorkspaceAction, WorkspaceActionIcon, WorkspaceRow, WorkspaceRowState,
};
use crate::window::library::episode::{episode_progress, is_played_episode};
use crate::window::library::hero::{HeroLayoutFacts, hero_command, hero_layout};
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

/// The row's played state: played, active with a
/// resume percent, or ordinary.
fn row_state(state: &GuiHomeRowStateFrame) -> WorkspaceRowState {
    if is_played_episode(state) {
        return WorkspaceRowState::Played;
    }
    if episode_progress(state).is_none() {
        return WorkspaceRowState::Ordinary;
    }
    WorkspaceRowState::Active
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
    // name.
    let facts = HeroLayoutFacts {
        title: &hero.title,
        parent: hero.parent.as_deref(),
        meta_rows: &hero.meta_rows,
        duration_row: hero.duration_row,
        progress_row: hero.progress_row,
        overview: None,
    };
    let layout = hero_layout(&facts, "feed");

    // The feed's entries, newest first, as the Workspace table paints them:
    // the checked collections' rows that belong to the Hero's feed. A date
    // takes the
    // time cell. The rows are narrowed from the list handle, so a Hero repaint
    // never clones them (`share-gui-sections-by-rc` design D3).
    let rows: &[GuiEntryRowFrame] =
        library_feeds_list(Some(&props.list)).map_or(&[][..], |list| list.rows.as_slice());
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
                playing: false,
                state: Some(row_state(&row.state)),
                progress: episode_progress(&row.state).map(f32::from),
            })
            .collect(),
    };

    // The actions' sends (design D12, row 11.1): the Feeds destination's
    // commands over the picked entry's own stable id. There is no play-next
    // command for Feeds.
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

    // The row's hover action: add the entry to the queue.
    let action_library = library.clone();
    let row_actions = vec![WorkspaceAction {
        key: "queue".to_owned(),
        label: "Add to queue".to_owned(),
        icon: WorkspaceActionIcon::ListEnd,
        run: Callback::new(move |item_id: String| {
            action_library.queue_feeds_row(&item_id);
        }),
    }];

    rsx! {
        LibraryHeroFrame {
            attr: "feeds",
            root: "data-library-feeds-hero",
            header: HeroHeader::from(layout.header),
            block: HeroBlock::from(layout.block),
            art_shape: hero.art_shape,
            art: None,
            art_title: hero.title.clone(),
            actions: Some(actions),
            children: Some(rsx! {
                LibraryWorkspaceTable {
                    root: "data-library-feeds-entries",
                    label: "Entries",
                    hook: "feeds-entry",
                    noun: "Entry",
                    pointer_list: PointerList::Workspace,
                    rows: entry_rows,
                    actions: row_actions,
                    on_press,
                    on_play,
                }
            }),
        }
    }
}

#[cfg(test)]
mod tests;
