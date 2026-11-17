//! The local Owner's Named queues as a Client knows them (gui-named-queues
//! design D3, D7): the queue list, the current queue, and the one resting
//! queue the GUI Client shows.
//!
//! Only a Client that negotiated `named-queues` ever receives any of it, so a
//! TUI Client's state here stays empty and its shown queue keeps the
//! connection rule (ADR 0034, amended by ADR 0041).

use mbv_ctrl::{NamedQueueEntry, NamedQueueEvent};
use mbv_queue::NamedQueueId;

use crate::app::state::playback::ShownEntry;
use crate::app::state::queue_view::{AdoptCause, QueueView};
use crate::app::{App, QueueLink};

/// The Named queues one local Owner reported (`CtrlEvent::NamedQueueList`).
#[derive(Debug, Default)]
pub(in crate::app) struct NamedQueueList {
    /// Every Named queue in queue order, as the Owner last published the list.
    ///
    /// An entry's `len` is the number of slots the queue held when the Owner
    /// published the list, and the Owner publishes it only on attach, create,
    /// rename, close, and swap. The current queue's `len` therefore goes stale
    /// after an ordinary edit: its displayed length comes from the
    /// current-queue snapshot (`local_view`). Only a resting queue's `len` is
    /// meaningful, and the Owner keeps that fresh through the queue's
    /// `NamedQueueSnapshot` while a Client shows it.
    entries: Vec<NamedQueueEntry>,
    /// The current queue: the one that holds the playhead. `None` until the
    /// first list arrives.
    current: Option<NamedQueueId>,
}

impl NamedQueueList {
    /// Adopt one published list and its current queue.
    pub(in crate::app) fn adopt(&mut self, entries: Vec<NamedQueueEntry>, current: NamedQueueId) {
        self.entries = entries;
        self.current = Some(current);
    }

    /// The current queue, or `None` until the first list arrives.
    #[must_use]
    pub(in crate::app) fn current(&self) -> Option<NamedQueueId> {
        self.current
    }

    /// Whether the Owner still holds `queue`, whichever of its queues is
    /// current. A list the Owner published before the queue existed answers
    /// false here, as it does for a queue the Owner closed.
    #[must_use]
    pub(in crate::app) fn holds(&self, queue: NamedQueueId) -> bool {
        self.entries.iter().any(|entry| entry.id == queue)
    }

    /// Whether `queue` is a Named queue that rests: the Owner still holds it
    /// and it is not the current queue.
    #[must_use]
    pub(in crate::app) fn rests(&self, queue: NamedQueueId) -> bool {
        self.current != Some(queue) && self.holds(queue)
    }
}

impl App {
    /// Adopt one piece of Named-queue traffic (design D3, D7): the queue list,
    /// or one resting queue's snapshot. Both Owner links carry it — the active
    /// link, or the suspended home link while a peer holds control — so every
    /// drain that reaches the local Owner routes it here.
    pub(in crate::app) fn adopt_named_queue_event(&mut self, event: NamedQueueEvent) {
        match event {
            NamedQueueEvent::List { queues, current } => {
                self.adopt_named_queue_list(queues, current);
            }
            NamedQueueEvent::Snapshot { queue, data } => {
                self.adopt_named_queue_snapshot(queue, &data);
            }
        }
    }

    /// Adopt one `NamedQueueList` (design D3): the queue list and the current
    /// queue. A shown resting queue the Owner no longer rests (it became
    /// current) drops its resting view here, so the Queue panel falls back to
    /// the current queue instead of painting a queue the Owner no longer
    /// holds. A shown entry the Owner no longer holds at all (it closed) shows
    /// the current queue, with the entry-change reset (design D7, spec
    /// `gui-queue-chooser` "The close control closes the shown entry").
    pub(in crate::app) fn adopt_named_queue_list(
        &mut self,
        entries: Vec<NamedQueueEntry>,
        current: NamedQueueId,
    ) {
        self.named_queues.adopt(entries, current);
        if let Some((queue, _)) = &self.shown_resting
            && !self.named_queues.rests(*queue)
        {
            self.shown_resting = None;
        }
        // A shown local queue that closed leaves the shown entry on the queue
        // that holds the playhead.
        if let Some(ShownEntry::Local(queue)) = self.gui_shown_entry
            && !self.named_queues.holds(queue)
        {
            self.show_local_current_entry();
        }
    }

    /// Adopt one resting queue's snapshot (design D7): it fills the shown
    /// resting view when the GUI Client shows that queue, and it never touches
    /// the current queue's view or the playback status, because a resting
    /// queue holds no playhead. The first snapshot to fill an empty shown view
    /// places the Queue's active row on the queue's start slot: the entry was
    /// shown before the queue's rows were known.
    pub(in crate::app) fn adopt_named_queue_snapshot(
        &mut self,
        queue: NamedQueueId,
        data: &mbv_ctrl::UnifiedQueueStateData,
    ) {
        if self.gui_shown_entry != Some(ShownEntry::Local(queue)) {
            return;
        }
        let first_fill = match &mut self.shown_resting {
            Some((shown, view)) if *shown == queue => {
                let empty = view.total_queue_len() == 0;
                view.adopt(data, AdoptCause::Background { held: false });
                empty
            }
            _ => {
                self.shown_resting = Some((queue, QueueView::from_snapshot(data)));
                true
            }
        };
        if first_fill {
            self.place_active_row_on_start();
        }
    }

    /// Show the connected peer's queue in the GUI Client's Queue panel (design
    /// D7): a connect from the playback-target chooser shows the peer's entry.
    /// The TUI Client keeps the connection rule, so it records nothing and its
    /// resolution ignores a shown entry either way.
    pub(in crate::app) fn show_peer_entry(&mut self) {
        if !self.gui_client {
            return;
        }
        self.shown_resting = None;
        self.show_entry(Some(ShownEntry::Peer));
    }

    /// Show the local Owner's current queue in the GUI Client's Queue panel
    /// (design D7): a disconnect, a lost link, or a close of the shown queue
    /// lands back on the current queue. Before the first queue list arrives
    /// there is no identity to name, so the shown entry is cleared and the
    /// resolution falls back to the same queue.
    pub(in crate::app) fn show_local_current_entry(&mut self) {
        if !self.gui_client {
            return;
        }
        self.shown_resting = None;
        self.show_entry(self.named_queues.current().map(ShownEntry::Local));
    }

    /// Record the shown entry and run the entry-change reset (design D7, spec
    /// `gui-queue-chooser` "Showing an entry resets the Queue active row and
    /// marks"). Only a changed entry resets: a connect, a disconnect, and a
    /// close each name the entry they land on, and the reset follows the
    /// change, not the event.
    fn show_entry(&mut self, entry: Option<ShownEntry>) {
        if !self.gui_client || self.gui_shown_entry == entry {
            return;
        }
        self.gui_shown_entry = entry;
        self.reset_queue_presentation();
    }

    /// The reset every shown-entry change runs: the Queue filter closes and
    /// empties, the Queue Surface drops its marks, and the active row moves to
    /// the shown queue's start slot, or its first row when it has none.
    fn reset_queue_presentation(&mut self) {
        self.queue_filter.clear();
        self.queue_filter_active = false;
        self.pending_queue_surface_reset = true;
        self.place_active_row_on_start();
    }

    /// Put the Queue's active row on the shown queue's start slot, or its first
    /// row when it has none (design D7, D9): the row the owner starts the queue
    /// at. The Surface adopts the move through the one cursor push, so this
    /// stays the only place the entry-change cursor is decided.
    fn place_active_row_on_start(&mut self) {
        let link = self.playing_queue_link();
        let index = self.displayed_queue().start_index().unwrap_or(0);
        self.displayed_queue_mut().set_cursor(index);
        self.pending_queue_cursor_reanchor = Some(link);
    }

    /// Show one local Named queue in the GUI Client's Queue panel (design D7):
    /// record the shown entry and ask the local Owner for that queue's
    /// snapshots when it rests. A resting queue's view starts empty and fills
    /// from its snapshot, so the panel never paints the current queue's rows
    /// under another queue's name. Showing a queue starts, stops, and moves no
    /// playback; the current queue needs no subscription because its snapshots
    /// arrive as `UnifiedQueueState` on the same link. The Queue chooser (row
    /// 7.2) is this setter's live caller, so it is test-only until that row
    /// lands.
    #[cfg(test)]
    pub(in crate::app) fn show_named_queue(&mut self, queue: NamedQueueId) {
        if !self.gui_client {
            return;
        }
        let rests = self.named_queues.rests(queue);
        if !rests {
            self.shown_resting = None;
        } else if self.shown_resting.as_ref().map(|(shown, _)| *shown) != Some(queue) {
            self.shown_resting = Some((queue, QueueView::default()));
        }
        self.show_entry(Some(ShownEntry::Local(queue)));
        if !rests {
            return;
        }
        let _ = self
            .local_queue_player()
            .send_ctrl_cmd(mbv_ctrl::CtrlCmd::NamedQueueShow { queue });
    }

    /// The local link's displayed view: the shown resting queue's own view, or
    /// the current queue's `local_view` (design D7). A resting view the Owner
    /// has since made current is stale, so the current queue's view answers
    /// then.
    #[must_use]
    pub(in crate::app) fn shown_local_queue(&self) -> &QueueView {
        match &self.shown_resting {
            Some((queue, view)) if self.named_queues.rests(*queue) => view,
            _ => &self.local_view,
        }
    }

    /// The resting Named queue whose own view the Queue panel shows, or `None`
    /// while the panel shows the current queue's `local_view` (design D7).
    #[must_use]
    pub(in crate::app) fn shown_resting_queue_id(&self) -> Option<NamedQueueId> {
        match &self.shown_resting {
            Some((queue, _)) if self.named_queues.rests(*queue) => Some(*queue),
            _ => None,
        }
    }

    /// The Named queue the Queue panel shows on the local link: the shown
    /// resting queue, or the current queue when the panel shows `local_view`
    /// (design D7).
    #[must_use]
    pub(in crate::app) fn displayed_queue_id(&self) -> Option<NamedQueueId> {
        self.shown_resting_queue_id()
            .or_else(|| self.named_queues.current())
    }

    /// The Named queue a Play asks the local Owner to start (design D7): the
    /// GUI Client's shown local queue while the local Owner plays nothing, so
    /// Play starts the shown queue at its own start slot (spec
    /// `gui-queue-chooser` "Play with nothing playing starts the shown
    /// queue"). `None` while something plays — the transport then pauses or
    /// resumes the run — and for the TUI Client, whose transport keeps its
    /// behaviour.
    #[must_use]
    pub(in crate::app) fn shown_queue_play_request(&self) -> Option<NamedQueueId> {
        let queue = self.shown_queue_edit()?;
        (!self.shown_playback_status().active).then_some(queue)
    }

    /// Whether the Queue panel shows a resting local Named queue, so the
    /// display is not the run's queue and none of its rows carries a playhead
    /// (design D7).
    #[must_use]
    pub(in crate::app) fn shows_resting_local_queue(&self) -> bool {
        self.playing_queue_link() == QueueLink::Local && self.shown_resting_queue_id().is_some()
    }

    /// The Named queue a GUI edit of the shown queue names on the wire (design
    /// D3, D7): the shown local queue's identity, or `None` for the current
    /// queue. The TUI Client names none — it shows the current queue by the
    /// connection rule — and a peer has no Named queues.
    #[must_use]
    pub(in crate::app) fn shown_queue_edit(&self) -> Option<NamedQueueId> {
        if !self.gui_client {
            return None;
        }
        match self.gui_shown_entry {
            Some(ShownEntry::Local(queue)) => Some(queue),
            Some(ShownEntry::Peer) | None => None,
        }
    }
}
