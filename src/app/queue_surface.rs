//! The Queue Surface projection every Client consumes
//! (`add-gui-front-end` design D5): the App-owned facts a Queue Surface
//! state adopts each tick, computed once for both Clients. The fingerprint
//! gates the row rebuild (#675): rebuilding the row vec (slot clone +
//! per-row `format!`) on a tick where nothing the projection depends on
//! changed is pure waste.

use mbv_surfaces::queue::QueueCursorUpdate;
use mbv_ui_model::media_list::MediaListRow;
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
    active: bool,
    active_target: Option<mbv_queue::QueueSlotId>,
    /// The adopted pending transition awaiting playback-owner observation.
    /// It moves the now-playing row before the owner reports it, so it remains
    /// part of the fingerprint even while `active_target` is unchanged.
    pending_target: Option<mbv_queue::QueueSlotId>,
    progress_bucket: u16,
}

fn progress_bucket(playback: PlaybackState) -> u16 {
    if playback.active && playback.position_ticks > 0 && playback.runtime_ticks > 0 {
        u16::try_from((playback.position_ticks * 100 / playback.runtime_ticks).clamp(0, 100))
            .expect("clamped progress bucket fits in u16")
    } else {
        u16::MAX
    }
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
}

impl App {
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
            .filter(|target| self.queue_for_link(link).slot(*target).is_some());
        let fingerprint = self.queue_projection_fingerprint(link, playback, pending_slot);
        let (rows_changed, bucket_only) = self.queue_projection_changes(&fingerprint);
        let cursor = match self.pending_queue_cursor_reanchor.take() {
            Some(reanchor) if reanchor == link => {
                QueueCursorUpdate::Set(self.queue_for_link(link).cursor())
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
        };
        if rows_changed {
            self.last_queue_projection = Some(fingerprint);
            projection.slots = Some(self.queue_for_link(link).slots().to_vec());
        } else if bucket_only {
            projection.patch = self.queue_projection_patch(&fingerprint, playback, pending_slot);
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
        let slots = self.queue_for_link(link).slots();
        QueueProjectionFingerprint {
            revision: self.queue_for_link(link).revision().raw(),
            lineage: self.queue_for_link(link).lineage(),
            link,
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
        let queue = self.queue_for_link(fingerprint.link);
        let target = fingerprint.active_target?;
        let index = queue.slot_index(target)?;
        let slot = queue.slots().get(index)?;
        Some((
            target,
            mbv_ui_model::queue_media_row(slot, index, playback, pending_slot),
        ))
    }
}
