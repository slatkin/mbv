//! The shell-owned now-playing projection (hero-workspace-now-playing D1):
//! one derivation per sync pass that every workspace owner reads, so the
//! Queue and every Workspace agree on which item is playing. Owners never
//! read playback state; they match the projection's identity against their
//! rows and apply its progress.

use crate::app::App;
use mbv_emby_model::seconds_to_ticks;
use mbv_queue::{AudiobookshelfBookQueueItem, AudiobookshelfItem, QueueItem, QueueItemContentId};
use mbv_ui_model::now_playing::WorkspaceNowPlaying;
use mbv_ui_model::playback::PlaybackState;

/// Derives the workspace now-playing projection for this sync pass (D1):
/// `displayed_playback_state()` — the playing queue's state, with a pending
/// slot already applied as the active index — plus the playing queue's item
/// at that index. Deliberately not `displayed_queue_playback_state()`: that
/// value the Queue column gates by its viewed scope, and the Workspace must
/// not go blank when the user views the other queue scope.
///
/// Feed entries and Audiobookshelf podcast episodes derive nothing: no
/// workspace projects their rows (change non-goals).
pub(in crate::app) fn derive_workspace_now_playing(app: &App) -> Option<WorkspaceNowPlaying> {
    let state = app.displayed_playback_state();
    if !state.active {
        return None;
    }
    let item = app.playback_queue().item_at(state.active_idx?)?;
    let content_id = item.content_id();
    match item {
        QueueItem::Emby(_) => Some(WorkspaceNowPlaying::Item {
            content_id,
            progress: clamped_live_progress(state.position_ticks, state.runtime_ticks),
        }),
        QueueItem::Feed(_) | QueueItem::Audiobookshelf(AudiobookshelfItem::Episode(_)) => None,
        QueueItem::Audiobookshelf(AudiobookshelfItem::Book(book)) => {
            playing_book_chapter(app, &state, book, content_id)
        }
    }
}

/// Resolves a playing book's chapter projection (D3): the chapter containing
/// the playback position against the chapter list the shell already holds in
/// `AudiobookshelfBookBrowseState` (`detail_cache`, absolute seconds on the
/// merged timeline), as the chapter's stable Service id plus the position's
/// fraction of the chapter range, clamped to `0..=100`.
fn playing_book_chapter(
    app: &App,
    state: &PlaybackState,
    book: &AudiobookshelfBookQueueItem,
    content_id: QueueItemContentId,
) -> Option<WorkspaceNowPlaying> {
    // D6: a pending book lights no chapter. The displayed position is 0
    // until the owner confirms, so a chapter resolved from it would light a
    // chapter the owner has not reached.
    if pending_selection_unconfirmed(app) {
        return None;
    }
    let (chapters, _) = app
        .audiobookshelf_book_browse
        .iter()
        .find_map(|browse| browse.detail_cache.get(&book.library_item_id))?;
    let position_ticks = state.position_ticks;
    let chapter = chapters.iter().find(|chapter| {
        position_ticks >= seconds_to_ticks(chapter.start)
            && position_ticks < seconds_to_ticks(chapter.end)
    })?;
    let start = seconds_to_ticks(chapter.start);
    let span = seconds_to_ticks(chapter.end) - start;
    // Degenerate chapter ranges carry no fraction; a zero progress lights
    // the row without a percentage claim about the position.
    let progress = if span > 0 {
        ((position_ticks - start) * 100 / span).clamp(0, 100)
    } else {
        0
    };
    Some(WorkspaceNowPlaying::Chapter {
        content_id,
        chapter_id: chapter.id,
        progress: u16::try_from(progress).unwrap_or(u16::MAX),
    })
}

/// Whether the displayed active index is a pending selection the playback
/// owner has not confirmed yet (D6): `displayed_playback_state` synthesizes
/// that slot with position 0, so a chapter derived from it would be a lie.
/// Reads the same sources `displayed_playback_state` uses, so the pending
/// exception never disagrees with the projection's own state.
fn pending_selection_unconfirmed(app: &App) -> bool {
    app.pending_playback_slot().is_some()
        && app.displayed_playback_state() != app.effective_playback_state()
}

/// The clamped live progress percentage (`0..=100`) a workspace row paints,
/// mirroring the Queue's active-row math (`queue_media_row_at`): no known
/// position or runtime carries no percentage.
fn clamped_live_progress(position_ticks: i64, runtime_ticks: i64) -> Option<u16> {
    (position_ticks > 0 && runtime_ticks > 0).then(|| {
        u16::try_from((position_ticks * 100 / runtime_ticks).clamp(0, 100)).unwrap_or(u16::MAX)
    })
}

#[cfg(test)]
mod tests;
