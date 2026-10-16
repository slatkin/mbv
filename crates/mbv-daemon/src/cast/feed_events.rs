//! The Feed `PlayerEvent`s the Cast output emits to Clients (cast-owner-output
//! design D9, row 6.4's Feed piece). The Client, not the Owner, writes the
//! Feed state file — `persist_feed_slot_lifecycle` runs in the TUI — and it
//! drives from the same ctrl `PlayerEvent`s local playback emits: a receiver
//! pause change emits `PausedChanged`, a settled seek emits `OutputStarted`,
//! a user stop emits `Stopped` (the output change joins task 7.1's hand-off),
//! and a natural end emits `TrackCompleted` beside row 6.3's completion
//! record. Only a loaded Feed slot emits: the Emby and ABS targets report
//! through their provider reporters (the row's Emby and ABS halves). The
//! `Stopped` and `TrackCompleted` payloads mirror the local run's outcomes,
//! so the Client's lifecycle decision is the same one local playback drives.

use mbv_cast::client::{CastPlaybackState, CastStatus};
use mbv_ctrl::player::PlayerEvent;
use mbv_ctrl::{CtrlEvent, PlaybackGeneration};
use mbv_queue::{PlaybackQueue, QueueItem, QueueSlotId};

use super::super::DaemonLoop;
use super::{CastLoadedItem, CastOutput, PlaybackOutput};
use crate::core::broadcast;
use crate::ctrl::ClientRegistry;

/// Whether the loaded item is a Feed slot (design D9): only a Feed target
/// emits the client-driven events; the Emby and ABS targets report through
/// their provider reporters.
fn loaded_is_feed(loaded: &CastLoadedItem) -> bool {
    matches!(
        loaded.progress,
        crate::cast_resolve::CastProgressTarget::Feed { .. }
    )
}

/// The Feed events one fresh status report drives (design D9): the receiver's
/// pause change between the previous and the fresh status, and a submitted
/// seek the fresh status settles. Both require the fresh status to present
/// the loaded item; the pause change also requires the previous status to
/// have presented it, because only a transition between two presenting
/// statuses is a pause change of the loaded item.
#[derive(Clone, Copy)]
pub(super) struct StatusFeedEvents {
    paused_changed: Option<bool>,
    seek_settled: bool,
}

impl StatusFeedEvents {
    /// The events a status that reaches no Feed emission drives.
    pub(super) fn none() -> Self {
        Self {
            paused_changed: None,
            seek_settled: false,
        }
    }
}

/// Reads the fresh status's Feed events (design D9) ahead of the status's
/// installation, while the previous status is still stored. `poll_id` is the
/// id of the poll the fresh status answered: a pending seek settles only on
/// a status whose poll was issued after the seek, so a snapshot captured
/// before the seek — one in flight at arm time — settles nothing.
pub(super) fn status_feed_events(
    loaded: Option<&CastLoadedItem>,
    previous: Option<&CastStatus>,
    fresh: &CastStatus,
    seek_armed_from: Option<u64>,
    poll_id: u64,
) -> StatusFeedEvents {
    let Some(loaded) = loaded.filter(|loaded| loaded_is_feed(loaded)) else {
        return StatusFeedEvents::none();
    };
    if !super::status::presents_loaded_item(loaded, fresh) {
        return StatusFeedEvents::none();
    }
    let paused = |status: &CastStatus| status.state == CastPlaybackState::Paused;
    let paused_changed = previous
        .filter(|previous| super::status::presents_loaded_item(loaded, previous))
        .filter(|previous| paused(previous) != paused(fresh))
        .map(|_| paused(fresh));
    StatusFeedEvents {
        paused_changed,
        seek_settled: seek_armed_from.is_some_and(|from| poll_id >= from),
    }
}

/// Emits the fresh status's Feed events to Clients (design D9): the
/// presentation refresh has already installed the state the events report,
/// and the settled seek's flag clears with its emission.
pub(super) fn emit_status_events(daemon: &mut DaemonLoop, events: StatusFeedEvents) {
    if let Some(paused) = events.paused_changed {
        broadcast_loaded_event(daemon, PlayerEvent::PausedChanged(paused));
    }
    if !events.seek_settled {
        return;
    }
    let PlaybackOutput::Cast(cast) = &mut daemon.owner.output else {
        return;
    };
    cast.pending_seek = None;
    broadcast_loaded_event(daemon, PlayerEvent::OutputStarted);
}

/// Emits the loaded Feed slot's `Stopped` (design D9): a user stop. The
/// payload mirrors the local run's queue stop (`quit_timeout_stop_flags`):
/// the presented position, `played` and `consume` set only by a non-audio
/// item stopped near its end, and the owner's current generation as the
/// observing run identity.
pub(crate) fn emit_stopped(
    clients: &ClientRegistry,
    cast: &CastOutput,
    queue: &PlaybackQueue,
    position_ticks: i64,
    generation: PlaybackGeneration,
) {
    let Some(loaded) = cast.loaded.as_ref().filter(|loaded| loaded_is_feed(loaded)) else {
        return;
    };
    let (played, consume) = match queue.slot(loaded.slot) {
        Some(slot) => {
            let runtime_ticks = slot.item.runtime_ticks();
            let near_end = !slot.item.is_audio()
                && runtime_ticks > 0
                && position_ticks * 20 / runtime_ticks >= 19;
            (near_end, near_end)
        }
        None => (false, false),
    };
    broadcast(
        clients,
        &CtrlEvent::Player(PlayerEvent::Stopped {
            slot_id: Some(loaded.slot),
            run_identity: generation,
            position_ticks,
            played,
            consume,
            error: None,
        }),
    );
}

/// Emits the naturally ended Feed slot's `TrackCompleted` (design D9, beside
/// row 6.3's record): the payload is the recorded outcome, so the Client's
/// lifecycle decision is the same one local playback drives. The caller
/// gates on the ended slot's kind before the record — a consumed slot is
/// gone from the queue by the time this runs.
pub(crate) fn emit_track_completed(
    daemon: &DaemonLoop,
    slot: QueueSlotId,
    position_ticks: i64,
    played: bool,
    consume: bool,
    generation: PlaybackGeneration,
) {
    broadcast(
        &daemon.ctrl_clients,
        &CtrlEvent::Player(PlayerEvent::TrackCompleted {
            slot_id: slot,
            run_identity: generation,
            position_ticks,
            played,
            consume,
        }),
    );
}

/// Broadcasts one Feed `PlayerEvent` to every Client (design D9): the same
/// delivery the event loop's player-event handlers use. The loaded item is
/// re-checked here, because the paths between detection and emission (the
/// advance, the settle) may have dropped or replaced it.
fn broadcast_loaded_event(daemon: &DaemonLoop, event: PlayerEvent) {
    let PlaybackOutput::Cast(cast) = &daemon.owner.output else {
        return;
    };
    if !cast.loaded.as_ref().is_some_and(loaded_is_feed) {
        return;
    }
    broadcast(&daemon.ctrl_clients, &CtrlEvent::Player(event));
}

/// Whether `slot`'s queue item is a Feed entry (design D9): the natural
/// end's `TrackCompleted` is Feed-only.
pub(crate) fn slot_is_feed(queue: &PlaybackQueue, slot: QueueSlotId) -> bool {
    queue
        .slot(slot)
        .is_some_and(|slot| matches!(slot.item, QueueItem::Feed(_)))
}
