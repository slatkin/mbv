//! The Queue chooser's App-side state (gui-named-queues design D8): its entry
//! list, the open/close toggle the GUI-only `t` chord and the header's
//! chooser press share, and the rename a confirmed draft sends.

use mbv_queue::NamedQueueId;

use super::overlay::{GuiOverlay, QueueChooserState};
use super::projection::QueueChooserEntryProjection;
use crate::app::App;
use crate::app::state::playback::ShownEntry;

impl App {
    /// The Queue chooser's entries (spec `gui-queue-chooser` "The chooser lists
    /// local queues and connected peers"): every Named queue of the local Owner
    /// in queue order, then one entry for the connected peer Player owner,
    /// named by its device name. The list is read per call, like every other
    /// projection, and a shown entry that the list does not hold — a peer that
    /// just disconnected, a queue another Client closed — marks nothing.
    #[must_use]
    pub(in crate::app) fn queue_chooser_entries(&self) -> Vec<QueueChooserEntryProjection> {
        let current = self.named_queues.current();
        let mut entries: Vec<QueueChooserEntryProjection> = self
            .named_queues
            .entries()
            .iter()
            .map(|entry| QueueChooserEntryProjection {
                label: entry.name.clone(),
                queue: Some(entry.id),
                shown: self.gui_shown_entry == Some(ShownEntry::Local(entry.id)),
                current: current == Some(entry.id),
            })
            .collect();
        if self.has_direct_remote_queue() {
            entries.push(QueueChooserEntryProjection {
                label: self.peer_owner_label(),
                queue: None,
                shown: self.gui_shown_entry == Some(ShownEntry::Peer),
                current: false,
            });
        }
        entries
    }

    /// Open or close the Queue chooser (design D8): the GUI-only `t` chord and
    /// the header's chooser press run this one toggle. It opens with its
    /// cursor on the shown entry, so Enter leaves the shown queue as it is, and
    /// it starts no rename draft.
    pub(in crate::app) fn toggle_gui_queue_chooser(&mut self) {
        if let Some(GuiOverlay::QueueChooser(_)) = &self.gui_overlay {
            self.gui_overlay = None;
            return;
        }
        let cursor = self
            .queue_chooser_entries()
            .iter()
            .position(|entry| entry.shown)
            .unwrap_or(0);
        self.gui_overlay = Some(GuiOverlay::QueueChooser(QueueChooserState {
            cursor,
            renaming: None,
        }));
    }

    /// Show one Queue chooser entry from a press (task 8.3 of
    /// `gui-named-queues`): the entry's painted identity, never its painted
    /// row, because the list is live and a queue can close between the paint
    /// and the press. It runs the same show the chooser's Enter applies and
    /// dismisses the chooser on the entry it showed, so a press and Enter leave
    /// one state; the show starts, stops, and moves no playback (spec
    /// `gui-queue-chooser` "The chooser lists local queues and connected
    /// peers").
    pub(in crate::app) fn show_gui_queue_chooser_entry(&mut self, queue: Option<NamedQueueId>) {
        if matches!(self.gui_overlay, Some(GuiOverlay::QueueChooser(_))) {
            self.gui_overlay = None;
        }
        match queue {
            Some(queue) => self.show_named_queue(queue),
            None => self.show_peer_entry(),
        }
    }

    /// Send one confirmed rename to the local Owner (design D3, D8) and close
    /// the chooser: the Owner trims the name, refuses one left empty, and
    /// publishes the queue list that carries the new name. An empty name never
    /// reaches here — the draft keeps such a name open for edit — so a name
    /// left empty by the trim runs nothing.
    pub(in crate::app) fn rename_named_queue(&mut self, queue: NamedQueueId, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        self.gui_overlay = None;
        if let Err(e) = self
            .local_queue_player()
            .remote()
            .send_named_queue_rename(queue, name.to_string())
        {
            tracing::warn!(
                name: "queue.rename.send_failed",
                target: "queue",
                error = %e,
                "the named queue rename did not reach the Player owner"
            );
            self.flash_error(&e);
        }
    }
}
