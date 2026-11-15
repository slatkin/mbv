//! The Audiobooks Hero's Workspace interaction state (`add-gui-library-panel`
//! design D1, D12, row 9.1, the port of `lib/library/audiobooks-workspace.ts`):
//! the cursor over the selected book's chapters. It is the window content's own
//! — the core never reads it, and the hover is the Workspace table's own paint
//! — and the chapters themselves are the core's, carried by the frame. The
//! chapter whose span holds the predicted position (design D5) is the window's
//! derivation, read from the playhead's position while the Hero's book is the
//! playing book.
//!
//! The cursor is a chapter number, never a row position: the frame carries the
//! stable Service discriminator the TUI's chapter activation addresses, so a
//! cursor whose chapter left the frame (a different book's detail, or the
//! detail landing) resolves to the book's current chapter when one plays, and
//! to the first chapter otherwise. The pure cursor model is
//! [`move_chapter_cursor`]; the store functions below send the commands the
//! Hero's keys and controls name.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{
    GuiAudiobooksChapterRowFrame, GuiAudiobooksDetail, GuiAudiobooksWorkspaceFrame,
};
use crate::bridge::library::GuiLibraryCommand;
use crate::window::library::list::ListMove;

use super::state::LibraryState;

/// The Workspace's interaction state (`audiobooks-workspace.ts`'s
/// `AudiobooksWorkspaceState`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AudiobooksWorkspaceState {
    /// The cursor's chapter number, or `None` while no chapter carries the
    /// cursor (a fresh book, and a book whose chapters have not landed).
    pub cursor: Option<usize>,
}

impl AudiobooksWorkspaceState {
    /// The state a freshly mounted Workspace starts with.
    #[must_use]
    pub fn initial() -> Self {
        Self::default()
    }
}

/// The selected book's Workspace, or `None` while the book's detail has not
/// landed.
#[must_use]
pub fn workspace_of(detail: &GuiAudiobooksDetail) -> Option<&GuiAudiobooksWorkspaceFrame> {
    detail
        .hero
        .as_ref()
        .and_then(|hero| hero.workspace.as_ref())
}

/// The chapter whose span holds the predicted position (design D5):
/// `startTicks <= position < endTicks` — the span's end is exclusive, so a
/// position at a chapter's end marks the next chapter, and an empty span
/// (`startTicks === endTicks`, an audio-part row's `0`/`0`) never holds one.
/// `None` while no row's span holds the position.
#[must_use]
pub fn current_chapter_row(
    rows: &[GuiAudiobooksChapterRowFrame],
    position_ticks: f64,
) -> Option<&GuiAudiobooksChapterRowFrame> {
    // The comparison runs in the spans' integer ticks: the predicted position
    // truncates into the ticks the spans compare in, and a span's boundary is
    // an integer tick, so the chapter a position names is the one the
    // TypeScript model's f64 comparison names.
    let position = mbv_emby_model::saturating_i64_from_f64(position_ticks);
    rows.iter()
        .find(|row| row.start_ticks <= position && position < row.end_ticks)
}

/// The selected book's Hero's marked chapter: the row whose span holds the
/// predicted position while the Hero's book is the playing book (design D5).
/// The Hero's book is the frame's selection; the playing book is the frame's
/// `playing_book_id`, `None` while the active slot is pending. `position_ticks`
/// is the playhead's predicted position, which a reactive caller reads through
/// the playhead's own store.
#[must_use]
pub fn marked_chapter_of(
    detail: &GuiAudiobooksDetail,
    position_ticks: Option<f64>,
) -> Option<&GuiAudiobooksChapterRowFrame> {
    if detail.playing_book_id.is_none() || detail.playing_book_id != detail.selected {
        return None;
    }
    let rows: &[GuiAudiobooksChapterRowFrame] =
        workspace_of(detail).map_or(&[], |workspace| &workspace.rows);
    current_chapter_row(rows, position_ticks.unwrap_or(0.0))
}

/// The chapter the cursor rests on (design D12): the chapter the state's number
/// names while the Workspace still paints it, the marked chapter while the
/// Hero's book plays, and the Workspace's first chapter otherwise — the cursor
/// re-resolves when its chapter leaves the frame, so a book change never leaves
/// it resting on nothing.
#[must_use]
pub fn resolve_abs_chapter_cursor<'a>(
    workspace: Option<&'a GuiAudiobooksWorkspaceFrame>,
    cursor: Option<usize>,
    marked: Option<&'a GuiAudiobooksChapterRowFrame>,
) -> Option<&'a GuiAudiobooksChapterRowFrame> {
    let rows: &[GuiAudiobooksChapterRowFrame] = workspace.map_or(&[], |workspace| &workspace.rows);
    rows.iter()
        .find(|row| Some(row.chapter) == cursor)
        .or(marked)
        .or_else(|| rows.first())
}

/// The chapter the cursor rests on in the carried frame, or `None` while the
/// Workspace holds no chapter. Enter on the Workspace acts on this chapter.
#[must_use]
pub fn abs_cursor_chapter<'a>(
    state: &LibraryState,
    detail: &'a GuiAudiobooksDetail,
) -> Option<&'a GuiAudiobooksChapterRowFrame> {
    resolve_abs_chapter_cursor(
        workspace_of(detail),
        state.audiobooks_workspace.peek().cursor,
        marked_chapter_of(detail, Some(state.playhead_position.cloned())),
    )
}

/// The chapter number a movement leaves (design D12): the next, the previous,
/// or a page away chapter, clamped to the book's ends. With no cursor on a
/// painted chapter, Up and End start from the last chapter and every other
/// movement from the first, so one key press always lands the cursor. The
/// Workspace carries no measured viewport in this unit, so a page move spans
/// one row.
#[must_use]
pub fn move_chapter_cursor(
    rows: &[GuiAudiobooksChapterRowFrame],
    cursor: Option<usize>,
    mv: ListMove,
) -> Option<usize> {
    let last = rows.len().checked_sub(1)?;
    let at = rows.iter().position(|row| Some(row.chapter) == cursor);
    let Some(at) = at else {
        let row = match mv {
            ListMove::Up | ListMove::End => &rows[last],
            _ => &rows[0],
        };
        return Some(row.chapter);
    };
    let to = match mv {
        ListMove::Up | ListMove::PageUp => at.saturating_sub(1),
        ListMove::Down | ListMove::PageDown => at.saturating_add(1),
        ListMove::Home => 0,
        ListMove::End => last,
    };
    Some(rows[to.min(last)].chapter)
}

/// Move the Workspace's cursor one movement along (design D12): the movement
/// keys while the Hero pane has the pane focus.
pub fn move_abs_chapter_cursor(state: &LibraryState, mv: ListMove, detail: &GuiAudiobooksDetail) {
    let workspace = workspace_of(detail);
    let current = abs_cursor_chapter(state, detail);
    let cursor = move_chapter_cursor(
        workspace.map_or(&[], |workspace| &workspace.rows),
        current.map(|row| row.chapter),
        mv,
    );
    let mut audiobooks_workspace = state.audiobooks_workspace;
    audiobooks_workspace.with_mut(|held| {
        if held.cursor != cursor {
            held.cursor = cursor;
        }
    });
}

/// Play the book the frame selected from its saved position (design D12): the
/// Hero's Play. The queue is replaced through the shared admission rule, so a
/// book that admits nothing changes nothing.
pub fn play_abs_book(state: &LibraryState, lib_idx: usize, detail: &GuiAudiobooksDetail) {
    act_on_book(state, detail, |item_id| GuiLibraryCommand::AbsPlay {
        lib_idx,
        item_id: item_id.to_owned(),
    });
}

/// Add the book the frame selected to the queue (design D12).
pub fn queue_abs_book(state: &LibraryState, lib_idx: usize, detail: &GuiAudiobooksDetail) {
    act_on_book(state, detail, |item_id| GuiLibraryCommand::AbsQueue {
        lib_idx,
        item_id: item_id.to_owned(),
    });
}

/// Send one book command for the frame's selected book, and nothing while the
/// frame selects none: the selection names no book.
fn act_on_book(
    state: &LibraryState,
    detail: &GuiAudiobooksDetail,
    command_of: impl FnOnce(&str) -> GuiLibraryCommand,
) {
    let Some(item_id) = detail.selected.as_deref() else {
        return;
    };
    state.link.library(command_of(item_id));
}

/// Play or seek from the cursor's chapter (design D12): Enter on the
/// Workspace, the row's double-click, and the row's hover play control. A
/// Workspace with no chapter, or a frame that selects no book, sends nothing.
pub fn play_abs_chapter(state: &LibraryState, lib_idx: usize, detail: &GuiAudiobooksDetail) {
    let Some(book_id) = detail.selected.as_deref() else {
        return;
    };
    let Some(chapter) = abs_cursor_chapter(state, detail) else {
        return;
    };
    state.link.library(GuiLibraryCommand::AbsPlayChapter {
        lib_idx,
        book_id: book_id.to_owned(),
        chapter: chapter.chapter,
    });
}

/// Start the cursor over when the selected book changes (design D12): the
/// grooves are the new book's chapters, so the previous book's cursor names
/// nothing.
pub fn reset_abs_chapter_cursor(state: &LibraryState) {
    let mut audiobooks_workspace = state.audiobooks_workspace;
    audiobooks_workspace.set(AudiobooksWorkspaceState::initial());
}

/// Settle the cursor on the row a press names (design D12): a press moves the
/// cursor onto the row it hit, as the Music list's track press does.
pub fn press_abs_chapter(state: &LibraryState, chapter: usize) {
    let mut audiobooks_workspace = state.audiobooks_workspace;
    audiobooks_workspace.with_mut(|held| {
        if held.cursor != Some(chapter) {
            held.cursor = Some(chapter);
        }
    });
}

#[cfg(test)]
mod tests;
