//! The local Owner's Named queues as a Client knows them (gui-named-queues
//! design D3, D7): the queue list, the current queue, and the one resting
//! queue the GUI Client shows.
//!
//! Only a Client that negotiated `named-queues` ever receives any of it, so a
//! TUI Client's state here stays empty and its shown queue keeps the
//! connection rule (ADR 0034, amended by ADR 0041).

use mbv_ctrl::{NamedQueueEntry, NamedQueueEvent};
use mbv_queue::NamedQueueId;

use crate::app::App;
use crate::app::state::playback::ShownEntry;
use crate::app::state::queue_view::{AdoptCause, QueueView};

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

    /// Whether `queue` is a Named queue that rests: the Owner still holds it
    /// and it is not the current queue.
    #[must_use]
    pub(in crate::app) fn rests(&self, queue: NamedQueueId) -> bool {
        self.current != Some(queue) && self.entries.iter().any(|entry| entry.id == queue)
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
    /// current, or it closed) drops its resting view here, so the Queue panel
    /// falls back to the current queue instead of painting a queue the Owner
    /// no longer holds.
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
    }

    /// Adopt one resting queue's snapshot (design D7): it fills the shown
    /// resting view when the GUI Client shows that queue, and it never touches
    /// the current queue's view or the playback status, because a resting
    /// queue holds no playhead.
    pub(in crate::app) fn adopt_named_queue_snapshot(
        &mut self,
        queue: NamedQueueId,
        data: &mbv_ctrl::UnifiedQueueStateData,
    ) {
        if self.gui_shown_entry != Some(ShownEntry::Local(queue)) {
            return;
        }
        match &mut self.shown_resting {
            Some((shown, view)) if *shown == queue => {
                view.adopt(data, AdoptCause::Background { held: false });
            }
            _ => self.shown_resting = Some((queue, QueueView::from_snapshot(data))),
        }
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
        self.gui_shown_entry = Some(ShownEntry::Local(queue));
        if !self.named_queues.rests(queue) {
            self.shown_resting = None;
            return;
        }
        if self.shown_resting.as_ref().map(|(shown, _)| *shown) != Some(queue) {
            self.shown_resting = Some((queue, QueueView::default()));
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
}
