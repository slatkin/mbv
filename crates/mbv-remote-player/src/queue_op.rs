//! The queue operations a Client sends a Player owner, and their translation
//! into the ctrl wire vocabulary (design D3 of `unified-playback-queue`, D3 and
//! D7 of `gui-named-queues`). The owner is authoritative for the queue, so
//! every operation is a request that the owner applies and answers; this module
//! only speaks the request.

use std::sync::atomic::Ordering;

use mbv_ctrl::{CtrlCmd, QueueOpId};
use mbv_queue::{NamedQueueId, QueueItem, QueueLineage, QueueSource};

use crate::RemotePlayer;
use crate::error::RemotePlayerError;

/// An owner-authoritative queue operation sent over ctrl.
#[derive(Clone, Debug)]
pub enum QueueOp {
    Replace {
        items: Vec<mbv_ctrl::UnifiedQueueSlot>,
        start_idx: Option<usize>,
        source: QueueSource,
        /// One-time start position for the start slot, in seconds. Sent only
        /// to an owner that advertises `queue-replace-start-position`; the
        /// owner otherwise ignores the field and would start at the wrong
        /// position (queue-handoff-and-take design D4).
        start_position_secs: Option<f64>,
    },
    Append {
        items: Vec<QueueItem>,
        before: Option<u64>,
    },
    RemoveSlot {
        slot_id: u64,
    },
    RemoveSlots {
        slot_ids: Vec<u64>,
    },
    MoveSlot {
        slot_id: u64,
        to_index: usize,
    },
    PlaySlot {
        slot_id: u64,
    },
    /// Make a Named queue current and play it, from `slot` or from the queue's
    /// own start slot (design D2, D4 of `gui-named-queues`). It addresses the
    /// `queue` that [`RemotePlayer::send_queue_op`] carries.
    PlayQueue {
        slot: Option<u64>,
    },
    Clear,
    SourceUpdate {
        source: QueueSource,
        lineage: QueueLineage,
    },
    Refresh,
    ApplyProgress {
        updates: Vec<mbv_ctrl::ProgressUpdate>,
    },
}

impl RemotePlayer {
    /// Send an owner queue operation, using answered correlation when negotiated.
    ///
    /// `queue` names the Named queue the operation reaches (design D3, D7 of
    /// `gui-named-queues`): the GUI Client's shown queue, or `None` for the
    /// current queue, which is also the only queue a Client without
    /// `named-queues` can name. `Refresh`, `ApplyProgress`, and `PlayQueue`
    /// carry no queue of their own: the first two address the current queue
    /// only, and `PlayQueue` takes the queue from this argument.
    pub fn send_queue_op(
        &self,
        operation: QueueOp,
        queue: Option<NamedQueueId>,
    ) -> Result<Option<QueueOpId>, RemotePlayerError> {
        let op = self
            .supports_answered_queue_ops()
            .then(|| self.mint_queue_op_id());
        let command = self.queue_op_command(operation, queue, op)?;
        if !self.send_ctrl_cmd(command) {
            return Err(RemotePlayerError::queue_operation(
                "could not send queue operation to Player owner",
            ));
        }
        Ok(op)
    }

    /// The next correlation id of the shared queue operation counter.
    pub(crate) fn mint_queue_op_id(&self) -> QueueOpId {
        QueueOpId(self.next_queue_op_id.fetch_add(1, Ordering::Relaxed))
    }

    /// The ctrl command one [`QueueOp`] sends: the wire vocabulary this crate
    /// translates to, with the Named queue an edit reaches (design D3, D7 of
    /// `gui-named-queues`) and the correlation id an answered owner replies
    /// with.
    fn queue_op_command(
        &self,
        operation: QueueOp,
        queue: Option<NamedQueueId>,
        op: Option<QueueOpId>,
    ) -> Result<CtrlCmd, RemotePlayerError> {
        let command = match operation {
            QueueOp::Replace {
                items,
                start_idx,
                source,
                start_position_secs,
            } => {
                // An owner without the capability ignores the field and would
                // start the slot at its resume source instead — never send it
                // there (queue-handoff-and-take spec, owner without the
                // capability).
                let start_position_secs =
                    start_position_secs.filter(|_| self.supports_queue_replace_start_position());
                CtrlCmd::UnifiedQueueReplace {
                    queue,
                    items: items.iter().map(|slot| slot.item.clone()).collect(),
                    slots: items,
                    start_idx,
                    source,
                    start_position_secs,
                    op,
                }
            }
            QueueOp::Append { items, before } => CtrlCmd::UnifiedQueueAppend {
                queue,
                items,
                before,
                op,
            },
            QueueOp::RemoveSlot { slot_id } => {
                CtrlCmd::UnifiedQueueRemoveSlot { queue, slot_id, op }
            }
            QueueOp::RemoveSlots { slot_ids } => CtrlCmd::UnifiedQueueRemoveSlots {
                queue,
                slot_ids,
                op,
            },
            QueueOp::MoveSlot { slot_id, to_index } => CtrlCmd::UnifiedQueueMoveSlot {
                queue,
                slot_id,
                to_index,
                op,
            },
            QueueOp::PlaySlot { slot_id } => CtrlCmd::UnifiedQueuePlaySlot { queue, slot_id, op },
            // A Named-queue play names its queue, so an unanswered-capability
            // owner (which has no `NamedQueuePlay` form at all) and a caller
            // that named no queue are both refused rather than sent elsewhere.
            QueueOp::PlayQueue { slot } => {
                let Some(queue) = queue else {
                    return Err(RemotePlayerError::queue_operation(
                        "a named queue play requires a named queue",
                    ));
                };
                let Some(op) = op else {
                    return Err(RemotePlayerError::queue_operation(
                        "daemon does not support answered named queue plays",
                    ));
                };
                CtrlCmd::NamedQueuePlay { queue, slot, op }
            }
            QueueOp::Clear => match (op, queue) {
                (Some(op), queue) => CtrlCmd::UnifiedQueueClearOp { queue, op },
                (None, None) => CtrlCmd::UnifiedQueueClear,
                // The legacy clear names no queue: an unanswered-capability
                // owner would clear its current queue instead of the named
                // one.
                (None, Some(_)) => {
                    return Err(RemotePlayerError::queue_operation(
                        "a named queue clear requires an answered owner",
                    ));
                }
            },
            QueueOp::SourceUpdate { source, lineage } => {
                if !self.supports_owner_queue_source_update() {
                    return Err(RemotePlayerError::queue_operation(
                        "daemon does not support owner queue source updates",
                    ));
                }
                CtrlCmd::UnifiedQueueSourceUpdate {
                    queue,
                    source,
                    lineage,
                    op,
                }
            }
            QueueOp::Refresh => {
                let Some(op) = op else {
                    return Err(RemotePlayerError::queue_operation(
                        "daemon does not support answered queue refresh",
                    ));
                };
                CtrlCmd::UnifiedQueueRefresh { op }
            }
            QueueOp::ApplyProgress { updates } => {
                let Some(op) = op else {
                    return Err(RemotePlayerError::queue_operation(
                        "daemon does not support answered queue progress",
                    ));
                };
                CtrlCmd::UnifiedQueueApplyProgress { op, updates }
            }
        };
        Ok(command)
    }
}
