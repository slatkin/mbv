//! The Audiobooks Hero's Workspace (`add-gui-library-panel` design D1, D12,
//! row 9.1, the port of `lib/library/audiobooks-workspace.ts`): the selected
//! book's chapters. The chapters themselves are the core's, carried by the
//! frame, and the Workspace keeps no cursor — the row a key or a control acts on
//! is the row the pointer marks (`gui-pointer-target-interaction` D2, D6). The
//! chapter whose span holds the predicted position (design D5) is the window's
//! derivation, read from the playhead's position while the Hero's book is the
//! playing book.
//!
//! A chapter row's pointer marker is the chapter number, never a row position:
//! the frame carries the stable Service discriminator the TUI's chapter
//! activation addresses, so [`abs_chapter_at`] parses the marker back and a
//! marker that names no painted chapter sends nothing. The functions below send
//! the commands the Hero's keys and controls name.

use crate::bridge::frame::library::{
    GuiAudiobooksChapterRowFrame, GuiAudiobooksDetail, GuiAudiobooksWorkspaceFrame,
};
use crate::bridge::library::GuiLibraryCommand;

use super::state::LibraryState;

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

/// The Workspace chapter the pointer's marker names (design D1, D2): the
/// marker carries the chapter number, so the frame's chapters resolve it and a
/// stale or foreign marker names no chapter. `None` while the Workspace paints
/// no such chapter.
#[must_use]
pub fn abs_chapter_at<'a>(
    detail: &'a GuiAudiobooksDetail,
    row: &str,
) -> Option<&'a GuiAudiobooksChapterRowFrame> {
    let chapter = row.parse::<usize>().ok()?;
    workspace_of(detail)?
        .rows
        .iter()
        .find(|candidate| candidate.chapter == chapter)
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

/// Play or seek from one chapter (design D12): Enter on the hovered chapter,
/// the row's double-click, and the row's hover play control. A frame that
/// selects no book sends nothing. The chapter number is the stable Service
/// discriminator the TUI's chapter activation addresses.
pub fn play_abs_chapter(
    state: &LibraryState,
    lib_idx: usize,
    detail: &GuiAudiobooksDetail,
    chapter: usize,
) {
    let Some(book_id) = detail.selected.as_deref() else {
        return;
    };
    state.link.library(GuiLibraryCommand::AbsPlayChapter {
        lib_idx,
        book_id: book_id.to_owned(),
        chapter,
    });
}

#[cfg(test)]
mod tests;
