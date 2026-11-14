//! The Audiobooks Hero (`replace-tauri-with-blitz` task 7.6, the port of
//! `LibraryAudiobooksHero.svelte`; add-gui-library-panel design D12, row 9.1):
//! the selected book's detail pane in the Home and Shows Hero look — the
//! portrait cover-box the book artwork policy declares, the title, and the
//! meta rows with their duration and progress roles. The facts carry no
//! overview, no narrator, and no series: the Hero keeps the TUI's facts.
//!
//! The book's chapters are the Hero's Workspace (the `GuiAudiobooksWorkspaceFrame`
//! rows): title and duration, with the chapter the book plays at marked with
//! the tracklist's play glyph. The Workspace's cursor and hover are the
//! window content's own (`window/library/audiobooks_workspace`, design D1): a
//! press moves the cursor, a double-click plays or seeks from the chapter,
//! and the hovered row paints its play control. Books have no play next, and
//! a chapter enqueues nothing: only the play control paints.
//!
//! Play, and Add to queue (`a`), send the Audiobooks commands with the
//! frame's selected book — the core owns the selection, so the actions name
//! the book the frame painted and never a position.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiAudiobooksDetail, GuiAudiobooksWorkspaceFrame};
use crate::bridge::library::GuiLibraryCommand;
use crate::window::components::library_hero_actions::LibraryHeroActions;
use crate::window::components::library_hero_frame::{HeroBlock, HeroHeader, LibraryHeroFrame};
use crate::window::components::library_workspace_table::{
    LibraryWorkspaceTable, WorkspaceLoading, WorkspaceRow,
};
use crate::window::library::audiobooks_workspace::{
    marked_chapter_of, press_abs_chapter, resolve_abs_chapter_cursor, workspace_of,
};
use crate::window::library::hero::{HeroLayoutFacts, hero_layout};
use crate::window::library::state::LibraryState;

/// The props of the Audiobooks Hero (the `LibraryAudiobooksHero.svelte`
/// props, narrowed to what the Hero reads): the library index the
/// Audiobooks commands name, and the carried Audiobooks detail — the selected
/// book, its playing book, and its Hero facts with the chapter Workspace.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryAudiobooksHeroProps {
    /// The Audiobooks library index the Audiobooks commands name (design D2,
    /// D12).
    pub lib_idx: usize,
    /// The selected book's Hero facts, its Workspace, and the playing book.
    pub detail: GuiAudiobooksDetail,
}

/// The Audiobooks action a button sends (the Svelte `command` union).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AbsAction {
    /// Play the selected book from its saved position.
    Play,
    /// Add the selected book to the queue.
    Queue,
}

/// Send the book command for the frame's selected book (design D12, row
/// 9.1): the book the frame painted, never a position. The command sends
/// only while the frame holds both the selection and a Hero.
fn act_book(
    library: &LibraryState,
    action: AbsAction,
    lib_idx: usize,
    detail: &GuiAudiobooksDetail,
) {
    let (Some(item_id), Some(_)) = (&detail.selected, &detail.hero) else {
        return;
    };
    let item_id = item_id.clone();
    let command = match action {
        AbsAction::Play => GuiLibraryCommand::AbsPlay { lib_idx, item_id },
        AbsAction::Queue => GuiLibraryCommand::AbsQueue { lib_idx, item_id },
    };
    library.link.library(command);
}

/// Play or seek from one chapter (design D12): the double-click and the row's
/// hover play control send the chapter the row names — the Service
/// discriminator the TUI's chapter activation addresses.
fn play_chapter(
    library: &LibraryState,
    lib_idx: usize,
    detail: &GuiAudiobooksDetail,
    chapter: usize,
) {
    let Some(book_id) = detail.selected.as_deref() else {
        return;
    };
    library.link.library(GuiLibraryCommand::AbsPlayChapter {
        lib_idx,
        book_id: book_id.to_owned(),
        chapter,
    });
}

/// Play the chapter the Workspace row id names, when the Workspace still
/// holds it (the Svelte `withChapter`).
fn play_chapter_row(
    library: &LibraryState,
    lib_idx: usize,
    detail: &GuiAudiobooksDetail,
    chapter_id: &str,
) {
    let Some(row) = workspace_of(detail).and_then(|workspace| {
        workspace
            .rows
            .iter()
            .find(|row| row.chapter.to_string() == chapter_id)
    }) else {
        return;
    };
    play_chapter(library, lib_idx, detail, row.chapter);
}

/// The Audiobooks Hero (the Svelte markup, character for character): the
/// frame over the selected book, with the chapters as the Workspace.
#[component]
pub fn LibraryAudiobooksHero(props: LibraryAudiobooksHeroProps) -> Element {
    let library = consume_context::<LibraryState>();

    // The Hero's facts, present exactly while the pane paints.
    let Some(ref hero) = props.detail.hero else {
        return rsx! {};
    };

    // The book's chapter Workspace, or `None` while the detail has not
    // landed.
    let workspace: Option<&GuiAudiobooksWorkspaceFrame> = hero.workspace.as_ref();

    // The playhead's predicted position (design D5): the mark reads it.
    let position = library.playhead_position.cloned();

    // The chapter the Hero marks: the row whose span holds the predicted
    // position while the Hero's book is the playing book (design D5); `None`
    // while another book plays or none does. The marked row's identity is
    // stable within its span, so the per-frame prediction re-renders the
    // Workspace only when the mark moves.
    let marked = marked_chapter_of(&props.detail, Some(position));

    // The chapter the cursor rests on (design D12): the state's chapter while
    // the carried Workspace paints it, the marked chapter, or the first
    // chapter otherwise.
    let abs_cursor = library.audiobooks_workspace.cloned();
    let cursor_chapter =
        resolve_abs_chapter_cursor(workspace, abs_cursor.cursor, marked).map(|row| row.chapter);

    // The frame's header and block for the Hero (invariant 30).
    let facts = HeroLayoutFacts {
        title: &hero.title,
        parent: None,
        meta_rows: &hero.meta_rows,
        duration_row: hero.duration_row,
        progress_row: hero.progress_row,
        overview: hero.overview.as_deref(),
    };
    let layout = hero_layout(&facts, "audiobook");

    // The Workspace table's rows: each chapter with its number, its title,
    // its duration, and the playing mark from the predicted position (design
    // D5).
    let rows: Vec<WorkspaceRow> = workspace.map_or_else(Vec::new, |workspace| {
        workspace
            .rows
            .iter()
            .map(|row| WorkspaceRow {
                id: row.chapter.to_string(),
                number: row.chapter.to_string(),
                name: row.title.clone(),
                duration: row.duration.clone(),
                playing: marked.is_some_and(|marked| marked.chapter == row.chapter),
                state: None,
                progress: None,
            })
            .collect()
    });

    // The actions' sends (design D12, row 9.1): the picked book's own stable
    // id. Books have no play next.
    let lib_idx = props.lib_idx;
    let play_library = library.clone();
    let play_detail = props.detail.clone();
    let on_play = Callback::new(move |()| {
        act_book(&play_library, AbsAction::Play, lib_idx, &play_detail);
    });
    let queue_library = library.clone();
    let queue_detail = props.detail.clone();
    let on_queue = Callback::new(move |()| {
        act_book(&queue_library, AbsAction::Queue, lib_idx, &queue_detail);
    });
    let actions = rsx! {
        LibraryHeroActions {
            attr: "abs",
            on_play: Some(on_play),
            on_queue: Some(on_queue),
        }
    };

    // The Workspace's press and play (design D12): a press moves the cursor
    // onto the chapter, and the row's play gestures send that chapter.
    let press_library = library.clone();
    let on_press = Callback::new(move |chapter_id: String| {
        if let Ok(chapter) = chapter_id.parse::<usize>() {
            press_abs_chapter(&press_library, chapter);
        }
    });
    let play_library = library.clone();
    let play_detail = props.detail.clone();
    let on_play = Callback::new(move |chapter_id: String| {
        play_chapter_row(&play_library, lib_idx, &play_detail, &chapter_id);
    });

    // The book Hero's Workspace (design D12): the book's chapter rows, or
    // the loading line while the detail fetch has not landed. A chapter has
    // no row action beyond play, so the actions list stays empty.
    let loading =
        (workspace.is_none() && props.detail.workspace_loading).then(|| WorkspaceLoading {
            text: "Loading chapters…".to_owned(),
            attr: "data-library-abs-chapters-loading",
        });
    let children = (workspace.is_some() || props.detail.workspace_loading).then(move || {
        rsx! {
            LibraryWorkspaceTable {
                root: "data-library-abs-workspace",
                label: "Chapters",
                hook: "abs-chapter",
                noun: "Chapter",
                rows,
                cursor: cursor_chapter.map(|chapter| chapter.to_string()),
                actions: Vec::new(),
                on_press,
                on_play,
                loading,
            }
        }
    });

    rsx! {
        LibraryHeroFrame {
            attr: "abs",
            root: "data-library-abs-hero",
            header: HeroHeader::from(layout.header),
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
