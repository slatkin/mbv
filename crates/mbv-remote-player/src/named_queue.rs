//! The Named-queue commands a Client sends a local Player owner, beyond the
//! queue edits `queue_op` carries (design D3, D8 of `gui-named-queues`). The
//! owner is authoritative for the queues, so each command is a request the
//! owner applies and answers.

use std::sync::atomic::Ordering;

use mbv_ctrl::{CtrlCmd, QueueOpId};
use mbv_queue::NamedQueueId;

use crate::RemotePlayer;
use crate::error::RemotePlayerError;

impl RemotePlayer {
    /// Rename one Named queue (design D3, D8 of `gui-named-queues`): the Queue
    /// chooser's confirmed rename draft. The `op` is minted from the same
    /// counter the queue operations use, so the owner's answer correlates with
    /// this request; the owner trims the name and refuses one left empty.
    ///
    /// # Errors
    ///
    /// Returns [`RemotePlayerError::queue_operation`] when the command could
    /// not reach the owner, because the connection is gone.
    pub fn send_named_queue_rename(
        &self,
        queue: NamedQueueId,
        name: String,
    ) -> Result<QueueOpId, RemotePlayerError> {
        let op = QueueOpId(self.next_queue_op_id.fetch_add(1, Ordering::Relaxed));
        if !self.send_ctrl_cmd(CtrlCmd::NamedQueueRename { queue, name, op }) {
            return Err(RemotePlayerError::queue_operation(
                "could not send the named queue rename to the Player owner",
            ));
        }
        Ok(op)
    }
}
