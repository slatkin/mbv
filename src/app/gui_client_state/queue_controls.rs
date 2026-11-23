//! The two Named-queue request bookkeeping helpers that only GUI modules call
//! (gui-named-queues design D8): the commands the Queue chooser's own edits
//! send, and the one shared send both they and the core controls use.
//!
//! The controls themselves are core behaviour and live in
//! `state/named_queues.rs`: the new-queue, rename, and close controls, the
//! answers they take, and the rejections they report. Neither helper moves
//! content; a send reaches the local Owner, which is authoritative for the
//! Named queues.

use mbv_ctrl::QueueOpId;

use crate::app::App;

impl App {
    /// Record a Named-queue command whose answer the Client does not pump for,
    /// so a rejection can find it (design D8).
    pub(in crate::app) fn note_named_queue_op(&mut self, op: QueueOpId) {
        self.pending_named_queue_ops.insert(op);
    }

    /// Send one Named-queue command to the local Owner (design D3, D8): a
    /// failed send warns and flashes once here, and returns no `op`, so the
    /// caller's request ends with nothing to correlate.
    pub(in crate::app) fn send_named_queue_cmd(
        &mut self,
        command: &'static str,
        send: impl FnOnce(
            &mbv_remote_player::RemotePlayer,
        ) -> Result<QueueOpId, mbv_remote_player::RemotePlayerError>,
    ) -> Option<QueueOpId> {
        match send(self.local_queue_player().remote()) {
            Ok(op) => Some(op),
            Err(e) => {
                tracing::warn!(
                    name: "queue.command.send_failed",
                    target: "queue",
                    command,
                    error = %e,
                    "the named queue command did not reach the Player owner"
                );
                self.flash_error(&e);
                None
            }
        }
    }
}
