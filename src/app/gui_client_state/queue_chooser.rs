//! The Queue chooser's App-side state (gui-named-queues design D8): its entry
//! list, the open/close toggle the GUI-only `t` chord and the header's
//! chooser press share, and the rename a confirmed draft sends.

use mbv_queue::NamedQueueId;

use super::overlay::{GuiOverlay, QueueChooserState};
use super::projection::QueueChooserEntryProjection;
use crate::app::state::playback::ShownEntry;
use crate::gui::GuiCore;

impl GuiCore {
    /// The Queue chooser's entries (spec `gui-queue-chooser` "The chooser lists
    /// local queues and connected peers"): every Named queue of the local Owner
    /// in queue order, then one entry for the connected peer Player owner,
    /// named by its device name. The list is read per call, like every other
    /// projection, and a shown entry that the list does not hold — a peer that
    /// just disconnected, a queue another Client closed — marks nothing.
    ///
    /// Each entry's count comes from that queue's freshest view (design D8 of
    /// `gui-queue-panel-redesign`): the current queue's snapshot, the shown
    /// resting queue's live view, another resting queue's published length —
    /// the Owner keeps that fresh — or the peer's own view.
    #[must_use]
    pub(in crate::app) fn queue_chooser_entries(&self) -> Vec<QueueChooserEntryProjection> {
        let current = self.app.named_queues.current();
        let local_plays = self.app.local_owner_plays();
        let shown_resting = self.app.shown_resting_live();
        let mut entries: Vec<QueueChooserEntryProjection> = self
            .app
            .named_queues
            .entries()
            .iter()
            .map(|entry| {
                let len = if current == Some(entry.id) {
                    self.app.local_view.total_queue_len()
                } else if let Some((shown, view)) = &shown_resting
                    && *shown == entry.id
                {
                    view.total_queue_len()
                } else {
                    entry.len
                };
                QueueChooserEntryProjection {
                    label: entry.name.clone(),
                    queue: Some(entry.id),
                    shown: self.app.shown_entry == Some(ShownEntry::Local(entry.id)),
                    current: current == Some(entry.id),
                    playing: local_plays && current == Some(entry.id),
                    len,
                }
            })
            .collect();
        if self.app.has_direct_remote_queue() {
            let peer_len = self
                .app
                .remote_view
                .as_ref()
                .map_or(0, crate::app::QueueView::total_queue_len);
            entries.push(QueueChooserEntryProjection {
                label: self.app.peer_owner_label(),
                queue: None,
                shown: self.app.shown_entry == Some(ShownEntry::Peer),
                current: false,
                playing: false,
                len: peer_len,
            });
        }
        entries
    }

    /// Open or close the Queue chooser (design D8): the GUI-only `t` chord and
    /// the header's chooser press run this one toggle. It opens with its
    /// cursor on the shown entry, so Enter leaves the shown queue as it is, and
    /// it starts no rename draft.
    pub(in crate::app) fn toggle_gui_queue_chooser(&mut self) {
        if let Some(GuiOverlay::QueueChooser(_)) = &self.gui.overlay {
            self.gui.overlay = None;
            return;
        }
        let cursor = self
            .queue_chooser_entries()
            .iter()
            .position(|entry| entry.shown)
            .unwrap_or(0);
        self.gui.overlay = Some(GuiOverlay::QueueChooser(QueueChooserState {
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
    /// peers"). A queue the Owner no longer holds — another Client closed it
    /// between the paint and the press — is not shown: the panel falls back to
    /// the current queue, as it does after a close, so the shown entry never
    /// names a queue with no label and no edits.
    pub(in crate::app) fn show_gui_queue_chooser_entry(&mut self, queue: Option<NamedQueueId>) {
        if matches!(self.gui.overlay, Some(GuiOverlay::QueueChooser(_))) {
            self.gui.overlay = None;
        }
        match queue {
            Some(queue) if !self.app.named_queues.holds(queue) => {
                self.app.show_local_current_entry();
            }
            Some(queue) => self.app.show_named_queue(queue),
            None => self.app.show_peer_entry(),
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
        self.gui.overlay = None;
        let sent = self
            .app
            .local_queue_player()
            .remote()
            .send_named_queue_rename(queue, name.to_string());
        if let Some(op) = self.app.report_named_queue_send("rename", sent) {
            self.note_named_queue_op(op);
        }
    }
}
