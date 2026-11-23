//! The Named-queue helpers that only GUI modules call (gui-named-queues design
//! D8): the rename draft's overlay, and the op bookkeeping for the commands
//! the Queue chooser's own edits send.
//!
//! The controls themselves are core behaviour and live in
//! `state/named_queues.rs`: the new-queue, rename, and close controls, the
//! answers they take, and the rejections they report. A send reaches the local
//! Owner, which is authoritative for the Named queues.

use mbv_ctrl::QueueOpId;
use mbv_queue::NamedQueueId;

use crate::app::gui_client_state::overlay::{GuiOverlay, QueueChooserState, QueueRenameDraft};
use crate::gui::GuiCore;

impl GuiCore {
    /// Present the Queue chooser with `queue`'s name open for edit (design D8):
    /// the GUI Client's half of a new queue's or the rename control's request.
    /// The core computes the draft text, because it reads core-owned queue
    /// state, and this resolves the entry's cursor against the chooser's live
    /// list — the same list the chooser paints. The overlay itself is GUI
    /// presentation, so its construction stays here rather than in the core's
    /// Named-queue state.
    pub(in crate::app) fn present_gui_queue_rename(&mut self, queue: NamedQueueId, text: String) {
        let cursor = self
            .queue_chooser_entries()
            .iter()
            .position(|entry| entry.queue == Some(queue))
            .unwrap_or_default();
        self.app.gui.overlay = Some(GuiOverlay::QueueChooser(QueueChooserState {
            cursor,
            renaming: Some(QueueRenameDraft::selecting(queue, text)),
        }));
    }

    /// Present the overlays the core handed off for this Client's input pass
    /// (design D8): a new queue the local Owner made opens the Queue chooser
    /// on its name. A Client with no chooser sets no hand-off, so this costs it
    /// nothing. The GUI core thread takes the hand-off after each routed input,
    /// so the chooser's draft opens in the pass that raised it, whether a key
    /// chord or a pointer press raised it; `route_gui_input` takes it before a
    /// key resolves too. Returns whether it presented one, so the tick that
    /// took the hand-off marks its input.
    pub fn present_pending_gui_overlay(&mut self) -> bool {
        let Some((queue, text)) = self.app.pending_queue_rename.take() else {
            return false;
        };
        self.present_gui_queue_rename(queue, text);
        true
    }

    /// Record a Named-queue command whose answer the Client does not pump for,
    /// so a rejection can find it (design D8). The core sends and reports the
    /// command; holding the op it returns is the GUI Client's bookkeeping, so
    /// it stays GUI-side.
    pub(in crate::app) fn note_named_queue_op(&mut self, op: QueueOpId) {
        self.app.pending_named_queue_ops.insert(op);
    }
}
