//! The Named-queue commands a Client sends a local Player owner, beyond the
//! queue edits `queue_op` carries (design D3, D8 of `gui-named-queues`). The
//! owner is authoritative for the queues, so each command is a request the
//! owner applies and answers.

use mbv_ctrl::{CtrlCmd, QueueOpId};
use mbv_queue::NamedQueueId;

use crate::RemotePlayer;
use crate::error::RemotePlayerError;

impl RemotePlayer {
    /// Ask for one new Named queue (design D3, D8 of `gui-named-queues`): the
    /// new-queue control. The owner names it, publishes the queue list, and
    /// answers the request's `op` with the new queue's own snapshot, so the
    /// returned `op` is what identifies the queue the control must show.
    ///
    /// # Errors
    ///
    /// Returns [`RemotePlayerError::queue_operation`] when the command could
    /// not reach the owner, because the connection is gone.
    pub fn send_named_queue_create(&self) -> Result<QueueOpId, RemotePlayerError> {
        self.send_named_queue_cmd("create", |op| CtrlCmd::NamedQueueCreate { op })
    }

    /// Close one Named queue (design D3, D8 of `gui-named-queues`): the close
    /// control. The owner refuses its last queue, deletes a resting queue
    /// without touching playback, and stops playback when it closes the
    /// current queue, so the `op` correlates the answer with this request.
    ///
    /// # Errors
    ///
    /// Returns [`RemotePlayerError::queue_operation`] when the command could
    /// not reach the owner, because the connection is gone.
    pub fn send_named_queue_close(
        &self,
        queue: NamedQueueId,
    ) -> Result<QueueOpId, RemotePlayerError> {
        self.send_named_queue_cmd("close", |op| CtrlCmd::NamedQueueClose { queue, op })
    }

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
        self.send_named_queue_cmd("rename", |op| CtrlCmd::NamedQueueRename { queue, name, op })
    }

    /// Mints one op id from the queue operations' counter and sends the
    /// command built over it, so the owner's answer correlates with it.
    fn send_named_queue_cmd(
        &self,
        what: &str,
        build: impl FnOnce(QueueOpId) -> CtrlCmd,
    ) -> Result<QueueOpId, RemotePlayerError> {
        let op = self.mint_queue_op_id();
        if !self.send_ctrl_cmd(build(op)) {
            return Err(RemotePlayerError::queue_operation(format!(
                "could not send the named queue {what} to the Player owner"
            )));
        }
        Ok(op)
    }
}
