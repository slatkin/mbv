//! The Queue Surface projection every Client consumes
//! (`extract-client-core` design D5): the App-owned facts a Queue Surface
//! state adopts each tick, computed once for both Clients. The fingerprint
//! gates the row rebuild (#675): rebuilding the row vec (slot clone +
//! per-row `format!`) on a tick where nothing the projection depends on
//! changed is pure waste.

use mbv_surfaces::queue::QueueCursorUpdate;
use mbv_ui_model::media_list::{ActiveProgress, MediaListRow};
use mbv_ui_model::playback::PlaybackState;

use crate::app::{App, QueueLink};

/// The row projection inputs that can change queue rows. Chrome and pause
/// state are delivered independently; pause only affects the paint-time
/// progress bucket.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) struct QueueProjectionFingerprint {
    revision: u64,
    lineage: mbv_queue::QueueLineage,
    link: QueueLink,
    /// The shown resting Named queue's identity, so showing another queue of
    /// the same link rebuilds its rows even when the two revisions coincide.
    named_queue: Option<mbv_queue::NamedQueueId>,
    active: bool,
    active_target: Option<mbv_queue::QueueSlotId>,
    /// The adopted pending transition awaiting playback-owner observation.
    /// It moves the now-playing row before the owner reports it, so it remains
    /// part of the fingerprint even while `active_target` is unchanged.
    pending_target: Option<mbv_queue::QueueSlotId>,
    progress_bucket: u16,
}

fn progress_bucket(playback: PlaybackState) -> u16 {
    if !playback.active {
        return u16::MAX;
    }
    ActiveProgress::of_ticks(playback.position_ticks, playback.runtime_ticks)
        .map_or(u16::MAX, |progress| u16::from(progress.percent()))
}

/// The observed active slot, or the owner's pending slot when idle.
fn projected_active_target(
    queue_slots: &[mbv_queue::QueueSlot],
    playback: PlaybackState,
    pending_slot: Option<mbv_queue::QueueSlotId>,
) -> Option<mbv_queue::QueueSlotId> {
    if playback.active {
        playback
            .active_idx
            .and_then(|idx| queue_slots.get(idx))
            .map(|slot| slot.slot_id)
    } else {
        pending_slot
    }
}

/// One Queue Surface projection push: the facts a Client's Queue Surface
/// state adopts, with the row work the fingerprint selected. `slots` and
/// `patch` are mutually exclusive: a `slots` push replaces the row set, a
/// `patch` re-projects one row (the active slot's progress bucket moved),
/// and neither means the rows stand as they are.
#[derive(Debug)]
pub struct QueueSurfaceProjection {
    /// The queue link the shown queue resolves to.
    pub link: QueueLink,
    /// The shown queue's playback state.
    pub playback: PlaybackState,
    /// The adopted pending transition awaiting playback-owner observation.
    pub pending_slot: Option<mbv_queue::QueueSlotId>,
    /// The whole row set, projected, when the row inputs changed since the
    /// last push; `None` keeps the Client's rows.
    pub slots: Option<Vec<mbv_queue::QueueSlot>>,
    /// One re-projected row for the active slot, when only the progress
    /// bucket moved; `None` when no single-row patch is due.
    pub patch: Option<(mbv_queue::QueueSlotId, MediaListRow<mbv_queue::QueueSlotId>)>,
    /// The cursor push: `Preserve` keeps the user's selection pinned to its
    /// slot across a content refresh; `Set` is an authoritative move
    /// (follow-the-playhead, jump-to-now-playing, link switch).
    pub cursor: QueueCursorUpdate,
    /// Whether the Surface drops its marks and viewport before it adopts
    /// `cursor` (design D7 of `gui-named-queues`): the shown entry changed.
    pub presentation_reset: bool,
}

/// The Queue rows' start mark for one read (design D9 of `gui-named-queues`):
/// the slot a play of the shown queue starts at, whether that queue is the
/// one that plays, and whether its owner reported the slot at all. The rows
/// paint the start slot filled while the queue plays and outlined while it is
/// stopped or resting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QueueStartProjection {
    /// The shown queue's start slot (`QueueSlotId::raw`). An owner that
    /// reports no start slot falls back to the observed active slot, so a
    /// peer queue keeps the playing mark it paints today.
    pub start: Option<u64>,
    /// Whether the shown queue is the queue that plays. A resting queue never
    /// plays, so its rows carry the start mark and no playing mark.
    pub playing: bool,
    /// Whether the shown queue's owner reports a start slot, which decides
    /// which mark the rows paint: the outlined mark stands only for a reported
    /// start slot, so an owner without one paints the playing mark on its
    /// observed active slot and no outlined mark (design D9).
    pub start_slot_reported: bool,
}

impl App {
    /// Project the Queue rows' start mark (design D9): the slot a play of the
    /// shown queue starts at, and whether the shown queue is the queue that
    /// plays. Read-only.
    ///
    /// The mark belongs to the queue that plays. A shown resting Named queue
    /// carries no playhead, so it is never `playing`; its `start` is its own
    /// snapshot's start slot (design D7). An owner that reports no start slot,
    /// such as a peer without `named-queues`, keeps its playing mark on the
    /// observed active slot and gives it no outlined mark, which
    /// `start_slot_reported` tells the rows apart.
    #[must_use]
    pub fn gui_queue_start_projection(&self) -> QueueStartProjection {
        let queue = self.displayed_queue();
        let playback = self.displayed_queue_playback_state();
        let reported = queue.start_slot_id();
        QueueStartProjection {
            start: reported
                .or_else(|| queue.active_slot_id())
                .map(mbv_queue::QueueSlotId::raw),
            playing: playback.active && playback.active_idx.is_some(),
            start_slot_reported: reported.is_some(),
        }
    }

    /// Project the shown queue for the Queue Surface state (design D5). Runs
    /// once per Client tick; both Clients consume the same update so the
    /// row, cursor, and pending-slot rules live once.
    ///
    /// Re-anchor only for authoritative content changes; routine updates
    /// preserve the Client-owned cursor. Cursor and content delivery is
    /// intentionally independent of the row fingerprint.
    #[must_use]
    pub fn queue_surface_projection(&mut self) -> QueueSurfaceProjection {
        let link = self.playing_queue_link();
        let playback = self.displayed_queue_playback_state();
        let pending_slot = self
            .pending_playback_slot()
            .filter(|target| self.displayed_queue().slot(*target).is_some());
        let fingerprint = self.queue_projection_fingerprint(link, playback, pending_slot);
        let (rows_changed, bucket_only) = self.queue_projection_changes(&fingerprint);
        let cursor = match self.pending_queue_cursor_reanchor.take() {
            Some(reanchor) if reanchor == link => {
                QueueCursorUpdate::Set(self.displayed_queue().cursor())
            }
            _ => QueueCursorUpdate::Preserve,
        };
        let mut projection = QueueSurfaceProjection {
            link,
            playback,
            pending_slot,
            slots: None,
            patch: None,
            cursor,
            presentation_reset: std::mem::take(&mut self.pending_queue_surface_reset),
        };
        if rows_changed {
            self.last_queue_projection = Some(fingerprint);
        }
        // A progress-only move also changes the fingerprint, so `bucket_only`
        // is tested first: the row set stands and one row re-projects.
        if bucket_only {
            projection.patch = self.queue_projection_patch(&fingerprint, playback, pending_slot);
        } else if rows_changed {
            projection.slots = Some(self.displayed_queue().slots().to_vec());
        }
        projection
    }

    // The adopted pending slot counts only while it is still in the shown
    // queue: a stale target must not move now-playing off the observed row.
    fn queue_projection_fingerprint(
        &self,
        link: QueueLink,
        playback: PlaybackState,
        pending_slot: Option<mbv_queue::QueueSlotId>,
    ) -> QueueProjectionFingerprint {
        let slots = self.displayed_queue().slots();
        QueueProjectionFingerprint {
            revision: self.displayed_queue().revision().raw(),
            lineage: self.displayed_queue().lineage(),
            link,
            named_queue: self.shown_resting_queue_id(),
            active: playback.active,
            active_target: projected_active_target(slots, playback, pending_slot),
            pending_target: pending_slot,
            progress_bucket: progress_bucket(playback),
        }
    }

    fn queue_projection_changes(&self, fingerprint: &QueueProjectionFingerprint) -> (bool, bool) {
        let previous = self.last_queue_projection.as_ref();
        let rows_changed = previous != Some(fingerprint);
        let bucket_only = previous.is_some_and(|old| {
            old.revision == fingerprint.revision
                && old.lineage == fingerprint.lineage
                && old.link == fingerprint.link
                && old.named_queue == fingerprint.named_queue
                && old.active == fingerprint.active
                && old.active_target == fingerprint.active_target
                && old.pending_target == fingerprint.pending_target
                && old.progress_bucket != fingerprint.progress_bucket
        });
        (rows_changed, bucket_only)
    }

    fn queue_projection_patch(
        &self,
        fingerprint: &QueueProjectionFingerprint,
        playback: PlaybackState,
        pending_slot: Option<mbv_queue::QueueSlotId>,
    ) -> Option<(mbv_queue::QueueSlotId, MediaListRow<mbv_queue::QueueSlotId>)> {
        let queue = self.displayed_queue();
        let target = fingerprint.active_target?;
        let index = queue.slot_index(target)?;
        let slot = queue.slots().get(index)?;
        Some((
            target,
            mbv_ui_model::queue_media_row(slot, index, playback, pending_slot),
        ))
    }
}
