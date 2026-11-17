//! The GUI queue header's two queue controls (gui-named-queues design D8): the
//! new-queue control and the close control, each one core path that the
//! header's press and its GUI-only chord both run.
//!
//! Both controls are requests to the local Owner, which is authoritative for
//! the Named queues. Neither paints or starts playback; the create answer shows
//! the queue it made, and a close is what the Owner's list then reports.

use mbv_ctrl::QueueOpId;
use mbv_queue::NamedQueueId;
use mbv_ui_model::confirm::ConfirmAction;

use super::overlay::{GuiOverlay, QueueChooserState, QueueRenameDraft};
use crate::app::state::playback::ShownEntry;
use crate::app::{App, ConfirmModal};

/// The close prompt's name field width: the header's queue names are short, and
/// the shared `trunc_str` keeps an entry from widening the modal.
const CLOSE_NAME_CHARS: usize = 40;

impl App {
    /// The new-queue control (design D8, spec `gui-queue-chooser` "The
    /// new-queue control makes and shows a queue"): ask the local Owner for one
    /// new Named queue and hold the request's `op`. The GUI-only `Ctrl+T` chord
    /// and the header's new-queue press both run this one path. One request is
    /// in flight at a time, so a press while the Owner has not answered — a
    /// double click on the control — makes no second queue.
    pub(in crate::app) fn request_new_gui_queue(&mut self) {
        if !self.gui_client || self.pending_new_queue.is_some() {
            return;
        }
        match self.local_queue_player().remote().send_named_queue_create() {
            Ok(op) => self.pending_new_queue = Some(op),
            Err(e) => {
                tracing::warn!(
                    name: "queue.create.send_failed",
                    target: "queue",
                    error = %e,
                    "the named queue create did not reach the Player owner"
                );
                self.flash_error(&e);
            }
        }
    }

    /// Whether `op` is the new-queue request the control sent, which this
    /// answer ends (design D8): a held request is taken once, so a late or
    /// repeated answer opens nothing.
    pub(in crate::app) fn ends_new_queue_request(&mut self, op: QueueOpId) -> bool {
        let held = self.pending_new_queue == Some(op);
        if held {
            self.pending_new_queue = None;
        }
        held
    }

    /// Show the queue a new-queue request made and open its name for edit
    /// (design D8, spec `gui-queue-chooser` "Make a queue": the Queue panel
    /// shows the new empty queue and its name is open for edit). The Owner
    /// names it `Queue N`, so the draft opens on that name, and cancelling the
    /// draft keeps it.
    pub(in crate::app) fn open_new_queue_rename(&mut self, queue: NamedQueueId) {
        self.show_named_queue(queue);
        let cursor = self
            .queue_chooser_entries()
            .iter()
            .position(|entry| entry.queue == Some(queue))
            .unwrap_or_default();
        let text = self.named_queues.name_of(queue).unwrap_or_default();
        self.gui_overlay = Some(GuiOverlay::QueueChooser(QueueChooserState {
            cursor,
            renaming: Some(QueueRenameDraft { queue, text }),
        }));
    }

    /// The close control (design D8, spec `gui-queue-chooser` "The close
    /// control closes the shown entry"): it acts on the shown entry. A peer
    /// entry disconnects without touching the peer's playback, and a local
    /// queue closes. The GUI-only `Ctrl+W` chord and the header's close press
    /// both run this one path.
    pub(in crate::app) fn request_close_shown_queue(&mut self) {
        if !self.gui_client {
            return;
        }
        match self.gui_shown_entry {
            Some(ShownEntry::Peer) => self.disconnect_remote(),
            Some(ShownEntry::Local(queue)) => self.request_close_local_queue(queue),
            // Before the Owner's first queue list no queue can be shown, so the
            // control has nothing to close.
            None => {}
        }
    }

    /// Close one local Named queue from the close control (spec "The close
    /// control closes the shown entry"): the only local queue never closes, and
    /// the current queue asks the shared confirmation while it plays, because
    /// closing it stops playback. Every other local queue closes at once.
    fn request_close_local_queue(&mut self, queue: NamedQueueId) {
        if self.named_queues.entries().len() < 2 {
            return;
        }
        let plays =
            self.named_queues.current() == Some(queue) && self.shown_playback_status().active;
        if !plays {
            self.send_named_queue_close(queue);
            return;
        }
        let name = mbv_ui_model::ui_util::trunc_str(
            &self.named_queues.name_of(queue).unwrap_or_default(),
            CLOSE_NAME_CHARS,
        );
        self.ask_confirm(ConfirmModal::two_button(
            format!("Close '{name}'? This stops playback."),
            "Close",
            "Cancel",
            ConfirmAction::CloseNamedQueue(queue),
        ));
    }

    /// Send one confirmed close to the local Owner (design D3, D8): it deletes a
    /// resting queue and stops playback to close the current one, then publishes
    /// the queue list the panel follows to the current queue.
    pub(in crate::app) fn send_named_queue_close(&mut self, queue: NamedQueueId) {
        if let Err(e) = self
            .local_queue_player()
            .remote()
            .send_named_queue_close(queue)
        {
            tracing::warn!(
                name: "queue.close.send_failed",
                target: "queue",
                error = %e,
                "the named queue close did not reach the Player owner"
            );
            self.flash_error(&e);
        }
    }
}
