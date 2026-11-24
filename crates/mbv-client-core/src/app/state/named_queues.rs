//! The local Owner's Named queues as a Client knows them (gui-named-queues
//! design D3, D7): the queue list, the current queue, and the one resting
//! queue the GUI Client shows.
//!
//! Only a Client that negotiated `named-queues` ever receives any of it, so a
//! TUI Client's state here stays empty and its shown queue keeps the
//! connection rule (ADR 0034, amended by ADR 0041).

use mbv_ctrl::{NamedQueueEntry, NamedQueueEvent, QueueOpId};
use mbv_queue::NamedQueueId;
use mbv_ui_model::confirm::ConfirmAction;

use crate::app::dispatch::notify::ToastSeverity;
use crate::app::state::client_capabilities::ClientCapability;
use crate::app::state::playback::ShownEntry;
use crate::app::state::queue_view::{AdoptCause, QueueView};
use crate::app::{App, ConfirmModal, QueueLink, QueueTarget};

/// The close prompt's name field width: the header's queue names are short, and
/// the shared `trunc_str` keeps an entry from widening the modal.
const CLOSE_NAME_CHARS: usize = 40;

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

    /// Every Named queue in queue order, as the Owner last published the list:
    /// the Queue chooser's local entries.
    #[must_use]
    pub(in crate::app) fn entries(&self) -> &[NamedQueueEntry] {
        &self.entries
    }

    /// The name the Owner last published for `queue`, or `None` when the list
    /// does not hold it: the name the close prompt shows and the text a
    /// new queue's rename draft opens with.
    #[must_use]
    pub(in crate::app) fn name_of(&self, queue: NamedQueueId) -> Option<String> {
        self.entries
            .iter()
            .find(|entry| entry.id == queue)
            .map(|entry| entry.name.clone())
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
    /// Whether this Client shows one queue entry at a time that the user
    /// picks (design D6a, D7): a Named queue or the peer's queue. Only the
    /// GUI Client does, so every shown-queue gate reads this one fact.
    #[must_use]
    pub(in crate::app) fn shows_queue_entries(&self) -> bool {
        self.client_capabilities
            .has(ClientCapability::ShownQueueEntries)
    }

    /// The new-queue control (design D8 of `gui-named-queues`, spec
    /// `gui-queue-chooser` "The new-queue control makes and shows a queue"):
    /// ask the local Owner for one new Named queue and hold the request's
    /// `op`. The GUI-only `Ctrl+T` chord and the header's new-queue press both
    /// run this one path. One request is in flight at a time, so a press while
    /// the Owner has not answered — a double click on the control — makes no
    /// second queue. Only a Client that paints the transport chips sends it.
    /// It returns the request's `op` when it sent one, so a caller that keeps
    /// its own bookkeeping (the GUI Client) can record it.
    pub(in crate::app) fn request_new_queue(&mut self) -> Option<QueueOpId> {
        if !self
            .client_capabilities
            .has(ClientCapability::TransportChipKeys)
            || self.pending_new_queue.is_some()
        {
            return None;
        }
        let sent = mbv_remote_player::RemotePlayer::send_named_queue_create(
            self.local_queue_player().remote(),
        );
        let op = self.report_named_queue_send("create", sent);
        self.pending_new_queue = op;
        op
    }

    /// The outcome of one Named-queue command send to the local Owner (design
    /// D3, D8): a failed send warns and flashes once here, and returns no
    /// `op`, so the caller's request ends with nothing to correlate.
    pub fn report_named_queue_send(
        &mut self,
        command: &'static str,
        sent: Result<QueueOpId, mbv_remote_player::RemotePlayerError>,
    ) -> Option<QueueOpId> {
        match sent {
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

    /// Record a Named-queue command whose answer the Client does not pump for,
    /// so a rejection can find it (design D8).
    pub fn note_named_queue_op(&mut self, op: QueueOpId) {
        self.pending_named_queue_ops.insert(op);
    }

    /// Adopt one answer to the held new-queue request (design D8, spec
    /// `gui-queue-chooser` "The new-queue control makes and shows a queue"):
    /// the Owner answers `NamedQueueCreate` with the new queue's own snapshot
    /// under the request's `op`, so that answer — never a queue snapshot — is
    /// what names the queue. It shows the queue and opens its name for edit,
    /// and a rejected answer only ends the request. Returns whether `op` was the
    /// held request, so every caller that meets an answer — the drains, and an
    /// edit's answer pump that took it as a foreign op — treats it the same way
    /// and adopts no queue state.
    pub(in crate::app) fn take_new_queue_answer(
        &mut self,
        op: QueueOpId,
        applied: Option<&mbv_ctrl::UnifiedQueueStateData>,
    ) -> bool {
        if !self.ends_new_queue_request(op) {
            return false;
        }
        if let Some(queue) = applied.and_then(|snapshot| snapshot.queue) {
            self.open_new_queue_rename(queue);
        }
        true
    }

    /// End the Named-queue command a rejected answer answers and tell the user
    /// why (design D8): a refused close, rename, or create would otherwise
    /// no-op silently while the list it raced stays what the panel shows.
    /// Returns whether `op` was one of them, so a pump that meets a foreign
    /// rejection reports only what has no other reporter — an edit's own
    /// rejection already flashed from the pump that waited for it.
    pub(in crate::app) fn report_named_queue_rejection(
        &mut self,
        op: QueueOpId,
        reason: String,
    ) -> bool {
        let ends_create = self.ends_new_queue_request(op);
        if !(ends_create || self.pending_named_queue_ops.remove(&op)) {
            return false;
        }
        self.flash(reason, ToastSeverity::Error);
        true
    }

    /// The rename control: open the shown local queue's name for edit, in the
    /// draft a new queue's name opens in. The peer entry has no name to edit,
    /// so it does nothing there, as the disabled control does.
    pub(in crate::app) fn request_rename_shown_queue(&mut self) {
        if !self
            .client_capabilities
            .has(ClientCapability::TransportChipKeys)
        {
            return;
        }
        if let Some(ShownEntry::Local(queue)) = self.shown_entry {
            self.open_new_queue_rename(queue);
        }
    }

    /// The close control (design D8, spec `gui-queue-chooser` "The close
    /// control closes the shown entry"): it acts on the shown entry. A peer
    /// entry disconnects without touching the peer's playback, and a local
    /// queue closes. The GUI-only `Ctrl+W` chord and the header's close press
    /// both run this one path. Only a Client that paints the transport chips
    /// sends a close.
    pub(in crate::app) fn request_close_shown_queue(&mut self) {
        if !self
            .client_capabilities
            .has(ClientCapability::TransportChipKeys)
        {
            return;
        }
        match self.shown_entry {
            Some(ShownEntry::Peer) => self.disconnect_remote(),
            Some(ShownEntry::Local(queue)) => self.request_close_local_queue(queue),
            // Before the Owner's first queue list no queue can be shown, so the
            // control has nothing to close.
            None => {}
        }
    }

    /// Send one confirmed close to the local Owner (design D3, D8): it deletes a
    /// resting queue and stops playback to close the current one, then publishes
    /// the queue list the panel follows to the current queue. It returns the
    /// `op` it recorded, or `None` when the send failed.
    pub(in crate::app) fn send_named_queue_close(
        &mut self,
        queue: NamedQueueId,
    ) -> Option<QueueOpId> {
        let sent = self
            .local_queue_player()
            .remote()
            .send_named_queue_close(queue);
        let op = self.report_named_queue_send("close", sent);
        if let Some(op) = op {
            self.note_named_queue_op(op);
        }
        op
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
            let _ = self.send_named_queue_close(queue);
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

    /// Show the queue a new-queue request made and open its name for edit
    /// (design D8, spec `gui-queue-chooser` "Make a queue": the Queue panel
    /// shows the new empty queue and its name is open for edit). The Owner
    /// names it `Queue N`, so the draft opens on that name, and cancelling the
    /// draft keeps it. The overlay is the GUI Client's Queue chooser, so this
    /// leaves a Client that presents no chooser unchanged.
    fn open_new_queue_rename(&mut self, queue: NamedQueueId) {
        self.show_named_queue(queue);
        // The GUI Client's core opens the chooser on the name: the overlay is
        // that Client's presentation and the core resolves its cursor against
        // the chooser's live list, so the core trips the one-shot hand-off here
        // instead of naming an overlay a Client without a chooser never shows.
        let text = self.named_queues.name_of(queue).unwrap_or_default();
        self.pending_queue_rename = Some((queue, text));
    }

    /// Whether `op` is the new-queue request the control sent, which this
    /// answer ends (design D8): a held request is taken once, so a late or
    /// repeated answer opens nothing.
    fn ends_new_queue_request(&mut self, op: QueueOpId) -> bool {
        let held = self.pending_new_queue == Some(op);
        if held {
            self.pending_new_queue = None;
        }
        held
    }

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
        if let Some(ShownEntry::Local(queue)) = self.shown_entry
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
        if self.shown_entry != Some(ShownEntry::Local(queue)) {
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
    pub fn show_peer_entry(&mut self) {
        if !self.shows_queue_entries() {
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
    pub fn show_local_current_entry(&mut self) {
        if !self.shows_queue_entries() {
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
        if !self.shows_queue_entries() || self.shown_entry == entry {
            return;
        }
        self.shown_entry = entry;
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
    /// arrive as `UnifiedQueueState` on the same link. The Queue chooser
    /// dismisses itself on the entry it showed, so this setter is the one
    /// place the shown queue and its subscription change together.
    pub fn show_named_queue(&mut self, queue: NamedQueueId) {
        if !self.shows_queue_entries() {
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

    /// The shown resting queue and its own view, when the Queue panel shows a
    /// local Named queue the Owner still rests (design D7): one accessor
    /// decides it, and `shown_local_queue` and `shown_resting_queue_id` both
    /// read it. A stale entry — the Owner has since made the queue current —
    /// answers `None`, so the panel falls back to the current queue's view.
    pub fn shown_resting_live(&self) -> Option<(NamedQueueId, &QueueView)> {
        match &self.shown_resting {
            Some((queue, view)) if self.named_queues.rests(*queue) => Some((*queue, view)),
            _ => None,
        }
    }

    /// The local link's displayed view: the shown resting queue's own view, or
    /// the current queue's `local_view` (design D7). A resting view the Owner
    /// has since made current is stale, so the current queue's view answers
    /// then.
    #[must_use]
    pub(in crate::app) fn shown_local_queue(&self) -> &QueueView {
        match self.shown_resting_live() {
            Some((_, view)) => view,
            None => &self.local_view,
        }
    }

    /// The resting Named queue whose own view the Queue panel shows, or `None`
    /// while the panel shows the current queue's `local_view` (design D7).
    #[must_use]
    pub(in crate::app) fn shown_resting_queue_id(&self) -> Option<NamedQueueId> {
        self.shown_resting_live().map(|(queue, _)| queue)
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

    /// Whether the shown local queue can start while nothing plays (design
    /// D7): the player panel's start gate reads this, because
    /// `shown_queue_play_request` is also `Some` for an empty new queue, which
    /// play/pause cannot start.
    #[must_use]
    pub fn shown_queue_can_start(&self) -> bool {
        self.shown_queue_play_request().is_some() && self.shown_local_queue().total_queue_len() > 0
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
        if !self.shows_queue_entries() {
            return None;
        }
        match self.shown_entry {
            Some(ShownEntry::Local(queue)) => Some(queue),
            Some(ShownEntry::Peer) | None => None,
        }
    }

    /// The queue target an edit of `link`'s queue names (design D7): `link`
    /// plus the shown queue's identity, resolved once per edit. The TUI Client
    /// and a peer entry name no Named queue, so the target is the link's
    /// current queue.
    #[must_use]
    pub(in crate::app) fn queue_target(&self, link: QueueLink) -> QueueTarget {
        QueueTarget {
            link,
            queue: self.shown_queue_edit(),
        }
    }

    /// The queue target an edit of the queue the panel shows names (design
    /// D7): the shown queue's link plus its identity, resolved once per edit.
    #[must_use]
    pub(in crate::app) fn queue_edit_target(&self) -> QueueTarget {
        self.queue_target(self.playing_queue_link())
    }

    /// The row a "Play next" move lands directly after in the queue the panel
    /// shows (design D7, D9 of `gui-named-queues`): the run's active row, or —
    /// for a resting queue the GUI shows, which holds no playhead — its start
    /// slot. `None` while the run is idle, or a resting queue has no start
    /// slot.
    #[must_use]
    pub(in crate::app) fn shown_queue_anchor(&self) -> Option<usize> {
        if self.shows_resting_local_queue() {
            return self.displayed_queue().start_index();
        }
        self.displayed_playback_state().active_idx
    }
}
