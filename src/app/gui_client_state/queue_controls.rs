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
use mbv_queue::NamedQueueId;

use crate::app::App;
use crate::app::gui_client_state::overlay::{GuiOverlay, QueueChooserState, QueueRenameDraft};

impl App {
    /// Present the Queue chooser with `queue`'s name open for edit (design D8):
    /// the GUI Client's half of a new queue's or the rename control's request.
    /// The core computes the entry's cursor and draft text, because both read
    /// core-owned queue state; the overlay itself is GUI presentation, so its
    /// construction stays here rather than in the core's Named-queue state.
    pub(in crate::app) fn present_gui_queue_rename(
        &mut self,
        queue: NamedQueueId,
        cursor: usize,
        text: String,
    ) {
        self.gui_overlay = Some(GuiOverlay::QueueChooser(QueueChooserState {
            cursor,
            renaming: Some(QueueRenameDraft::selecting(queue, text)),
        }));
    }

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
