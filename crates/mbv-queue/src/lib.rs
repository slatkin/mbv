//! Canonical queue contents: slots, item kinds, progress, and lineage.
//!
//! `PlaybackQueue` holds the one ordered slot sequence addressed by stable
//! `QueueSlotId`; `QueueItem` snapshots media from every Service. The Player owner
//! holds the Bound queue and the playback lifecycle; admission stays on shell and
//! Player paths, never in components.

// FeedEntry and QueueItem — the two item kinds a playback queue slot can
// hold, plus QueueItem's custom (kind-tagged, legacy-fallback) Deserialize.
mod audiobookshelf;
#[doc(inline)]
pub use audiobookshelf::{AudiobookshelfEpisodeCatalog, EpisodeResume};
mod items;
#[doc(inline)]
pub use items::{
    AudiobookshelfBookQueueItem, AudiobookshelfItem, AudiobookshelfQueueItem, FeedEntry,
    MpvUrlSource, PlaybackTitlePart, PlaybackTitlePartRole, PlaybackTitleParts, QueueItem,
    QueueItemContentId, QueueItemKind,
};
mod twitch;
#[doc(inline)]
pub use twitch::{TwitchItem, TwitchUrlError};
mod ids;
#[doc(inline)]
pub use ids::{NamedQueueId, QueueSlotId};
mod kinds;
#[doc(inline)]
pub use kinds::{FeedKind, ServiceKind};
mod playback_queue;
#[doc(inline)]
pub use playback_queue::{
    PlaybackQueue, QueueMutationResult, QueueSlot, RefreshMergeResult, RemoveSlotResult,
    is_slot_permutation, reorder_move_plan,
};
mod revision;
#[doc(inline)]
pub use revision::{QueueRevision, QueueRevisionMint};
mod state;
#[doc(inline)]
pub use state::{
    LibraryPosition, LibraryPositionLevel, LibraryPositionState, QueueLineage, QueueSource,
    QueueState, TvContentMode,
};
mod execution_sequence;
#[doc(inline)]
pub use execution_sequence::{ExecSlot, ExecutionSequence};
mod start_slot;
#[doc(inline)]
pub use start_slot::{StartSlotObservation, start_slot_after};
mod progress;
#[doc(inline)]
pub use progress::{ProgressObservation, SlotProgress};
