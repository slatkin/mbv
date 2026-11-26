//! The Audiobooks Hero (`replace-tauri-with-blitz` task 7.6, the port of
//! `LibraryAudiobooksHero`; add-gui-library-panel design D12, row 9.1;
//! gui-hero-design-polish design D2, D4): the selected book's detail pane on
//! the shared frame — the portrait cover-box the book artwork policy
//! declares, the book's duration as the header band's fact pill, and the
//! book's author row. No playback progress paints anywhere in the Hero: the
//! queue handles playback, and Play still resumes from the stored position.
//! The book's chapters are the Hero's Workspace under the Chapters section
//! (the `GuiAudiobooksWorkspaceFrame` rows): title and duration, with the
//! chapter the book plays at marked with the tracklist's play glyph. The
//! Workspace keeps no cursor (`gui-pointer-target-interaction` D6): the keys
//! act on the chapter the pointer marks, a press on a row does nothing, a
//! double-click plays or seeks from the chapter, and the row's hover controls
//! (gui-row-hover-actions D5) paint Play only. A chapter enqueues nothing, so
//! the table takes no `on_queue`.
//!
//! Play, and Add to queue (`a`), send the Audiobooks destination's commands
//! with the frame's selected book (design D12) — the core owns the selection,
//! so the actions name the book the frame painted and never a position, and
//! the destination's list part names the library index.

use dioxus_native::prelude::*;

use crate::art::{GuiArtRef, glow_from_art};
use crate::bridge::frame::library::{
    GuiAudiobooksDetail, GuiAudiobooksWorkspaceFrame, GuiLibraryDetail,
};
use crate::bridge::library::GuiLibraryCommand;
use crate::window::components::library_hero_actions::LibraryHeroActions;
use crate::window::components::library_hero_frame::{HeroBlock, HeroHeader, LibraryHeroFrame};
use crate::window::components::library_workspace_table::{
    LibraryWorkspaceTable, WorkspaceLoading, WorkspaceRow,
};
use crate::window::library::audiobooks_workspace::{marked_chapter_of, workspace_of};
use crate::window::library::hero::{hero_command, label_segments};
use crate::window::library::lettered::{AudiobooksList, LetteredDestination};
use crate::window::library::parts::library_audiobooks_detail;
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::PointerList;
use crate::window::sections::SectionRef;

/// The props of the Audiobooks Hero (the `LibraryAudiobooksHero`
/// props, narrowed to what the Hero reads): the library index the
/// Audiobooks commands name, and the carried Audiobooks detail — the selected
/// book, its playing book, and its Hero facts with the chapter Workspace.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryAudiobooksHeroProps {
    /// The Audiobooks library index the Audiobooks commands name (design D2,
    /// D12).
    pub lib_idx: usize,
    /// The selected book's Hero facts, its Workspace, and the playing book,
    /// as the one handle every reader clones (`share-gui-sections-by-rc`
    /// design D3).
    pub detail: SectionRef<GuiLibraryDetail>,
}

/// Play or seek from one chapter (design D12): the double-click and the row's
/// hover Play control send the chapter the row names — the Service
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
/// holds it.
fn play_chapter_row(
    library: &LibraryState,
    lib_idx: usize,
    detail: &SectionRef<GuiLibraryDetail>,
    chapter_id: &str,
) {
    let Some(detail) = library_audiobooks_detail(Some(detail)) else {
        return;
    };
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

/// The Audiobooks Hero: the
/// frame over the selected book, with the chapters as the Workspace.
#[component]
pub fn LibraryAudiobooksHero(props: LibraryAudiobooksHeroProps) -> Element {
    let library = consume_context::<LibraryState>();

    // The Audiobooks part of the carried detail, narrowed from the handle in
    // this body, and the Hero's facts, present exactly while the pane paints.
    let Some(detail) = library_audiobooks_detail(Some(&props.detail)) else {
        return rsx! {};
    };
    let Some(hero) = detail.hero.as_ref() else {
        return rsx! {};
    };
    // Whether the detail fetch is still landing, as the Workspace line reads
    // it.
    let workspace_loading = detail.workspace_loading;
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
    let marked = marked_chapter_of(detail, Some(position));

    // The frame's header and block for the Hero (invariant 30): a book is
    // its own largest unit, so it titles the header, and the block holds its
    // author row. The facts carry no overview, no narrator, and no series:
    // the Hero keeps the TUI's facts. No progress paints (design D2).
    let header = HeroHeader {
        kicker: label_segments("audiobook"),
        title: hero.title.clone(),
        pills: hero.facts.pills.clone(),
    };
    let block = HeroBlock {
        label: hero.title.clone(),
        eyebrow: None,
        name: None,
        name_episode: false,
        pills: Vec::new(),
        rows: hero.facts.rows.clone(),
    };

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
            })
            .collect()
    });

    // The actions' sends (design D12, row 9.1): the picked book's own stable
    // id.
    let lib_idx = props.lib_idx;
    let play_library = library.clone();
    let play_detail = props.detail.clone();
    let on_play = Callback::new(move |()| {
        hero_command::<AudiobooksList>(&play_library, &play_detail, AudiobooksList::play);
    });
    let queue_library = library.clone();
    let queue_detail = props.detail.clone();
    let on_queue = Callback::new(move |()| {
        hero_command::<AudiobooksList>(&queue_library, &queue_detail, AudiobooksList::queue);
    });
    let actions = rsx! {
        LibraryHeroActions {
            attr: "abs",
            on_play: Some(on_play),
            on_queue: Some(on_queue),
        }
    };

    // The Workspace's press and play (design D12, D6): a press on a row does
    // nothing — the Workspace keeps no cursor — and the row's play gestures send
    // that chapter.
    let on_press = Callback::new(|_: String| {});
    let play_library = library.clone();
    let play_detail = props.detail.clone();
    let on_play = Callback::new(move |chapter_id: String| {
        play_chapter_row(&play_library, lib_idx, &play_detail, &chapter_id);
    });

    // The book Hero's Workspace (design D12): the book's chapter rows, or
    // the loading line while the detail fetch has not landed. A chapter has
    // no Add path, so the table takes no `on_queue` (gui-row-hover-actions
    // D5) and its rows paint Play only.
    let loading = (workspace.is_none() && workspace_loading).then(|| WorkspaceLoading {
        text: "Loading chapters…".to_owned(),
        attr: "data-library-abs-chapters-loading",
    });
    let children = (workspace.is_some() || workspace_loading).then(move || {
        rsx! {
            LibraryWorkspaceTable {
                root: "data-library-abs-workspace",
                label: "Chapters",
                hook: "abs-chapter",
                noun: "Chapter",
                pointer_list: PointerList::Workspace,
                rows,
                on_press,
                on_play,
                loading,
            }
        }
    });

    // The header band's glow (design D4): the shared art-to-glow mapping over
    // the Hero's own art, `None` while the art is absent, still loading, or
    // the bundled placeholder.
    let glow = hero
        .art
        .as_ref()
        .and_then(glow_from_art)
        .map(|src| GuiArtRef::Image { src });

    rsx! {
        LibraryHeroFrame {
            attr: "abs",
            root: "data-library-abs-hero",
            header,
            block,
            art_shape: hero.art_shape,
            art: hero.art.clone(),
            art_title: hero.title.clone(),
            actions: Some(actions),
            overview: hero.overview.clone(),
            glow,
            list_title: children.is_some().then(|| "Chapters".to_owned()),
            children,
        }
    }
}

#[cfg(test)]
mod tests;
