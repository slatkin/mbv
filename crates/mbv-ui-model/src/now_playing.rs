//! The shell-owned now-playing projection for workspace rows
//! (hero-workspace-now-playing D1): one value the shell derives per sync
//! pass, and every workspace owner matches its rows against, so the Queue
//! and every Workspace agree on which item is playing.

use mbv_ids::ItemId;

/// What one sync pass derives as now-playing for the workspace owners.
///
/// The shell computes it from the playing queue's displayed playback state
/// and the playing queue's item at the displayed active index (D1); owners
/// never read playback state. Each variant names its provider (D2), so an
/// Emby id can never match an Audiobookshelf row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceNowPlaying {
    /// A playing Emby item whose workspace row lights with the clamped live
    /// progress percentage (`0..=100`). `None` mirrors the Queue's pending
    /// row: a selection awaiting the owner's confirmation carries no
    /// percentage yet (D6).
    Item {
        item_id: ItemId,
        progress: Option<u16>,
    },
    /// A playing Audiobookshelf book resolved to the chapter containing the
    /// playback position (D3): the chapter's stable Service id and the
    /// position's fraction of the chapter's `start..end`, clamped to
    /// `0..=100`. Pending books (D6) and books without chapters never
    /// resolve here, so audio-part rows never light.
    Chapter {
        library_item_id: String,
        chapter_id: usize,
        progress: u16,
    },
}
