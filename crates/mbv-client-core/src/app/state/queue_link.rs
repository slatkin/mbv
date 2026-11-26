use crate::app::{
    App, PendingQueueAction, QueueLink, QueueLinkResolution, QueueTarget, QueueView, UndoEntry,
};
use mbv_queue::{QueueItem, QueueSlotId};

impl App {
    pub(in crate::app) fn has_remote_queue(&self) -> bool {
        self.remote_view.is_some()
    }

    pub fn has_direct_remote_queue(&self) -> bool {
        self.has_remote_queue()
    }

    /// Return the Player link and event receiver that own the requested
    /// queue. Local resolves to the suspended home link when present.
    pub fn queue_link(
        &mut self,
        link: QueueLink,
    ) -> (
        &mbv_player::PlayerProxy,
        &mut std::sync::mpsc::Receiver<mbv_ctrl::player::PlayerEvent>,
    ) {
        match (link, self.suspended_local.as_mut()) {
            (QueueLink::Local, Some(home)) => (&home.player, &mut home.player_rx),
            _ => (&self.player, &mut self.player_rx),
        }
    }

    /// The `PlayerProxy` that owns the Local queue's state: the suspended home
    /// link when present, else the current player. Read-only counterpart of
    /// [`App::queue_link`] for `QueueLink::Local`.
    pub fn local_queue_player(&self) -> &mbv_player::PlayerProxy {
        self.suspended_local
            .as_ref()
            .map_or(&self.player, |home| &home.player)
    }

    /// The link to the local Owner, the only Player owner a local queue or a
    /// local playback output belongs to: the suspended home link while this
    /// Client is attached to a peer (packaged `mbvd` or another machine's
    /// Owner), the current link otherwise, and `None` while attached to a peer
    /// with no home link left.
    pub(in crate::app) fn local_owner_link(&self) -> Option<&mbv_player::PlayerProxy> {
        // An unset endpoint (before the first connect) is the local Owner.
        let on_peer_without_home = self.suspended_local.is_none()
            && self
                .player_endpoint
                .as_ref()
                .is_some_and(|endpoint| !endpoint.is_local());
        (!on_peer_without_home).then(|| self.local_queue_player())
    }

    /// The `PlayerProxy` that owns `link`'s queue and playback: the suspended
    /// home link for `Local` while a peer attachment holds the active link,
    /// else the active link's own proxy. Every status write and every owner
    /// command names its link through this one accessor, so an answer that
    /// arrives on one link never lands in the other link's mirror (design D7
    /// of `gui-named-queues`).
    pub(crate) fn queue_owner_player(&self, link: QueueLink) -> &mbv_player::PlayerProxy {
        match link {
            QueueLink::Local => self.local_queue_player(),
            QueueLink::Peer => &self.player,
        }
    }

    /// The link the active Player connection carries: the peer while a Direct
    /// remote control connection exists, else the local Owner — whatever the
    /// GUI Client shows. The active link's events arrive on this one, so a
    /// drain that adopts one writes this link's state and view.
    pub(in crate::app) fn active_queue_link(&self) -> QueueLink {
        QueueLinkResolution::new(self.has_direct_remote_queue(), None).playback_target()
    }

    /// The `PlayerProxy` whose queue and playback the Queue panel shows and
    /// controls: the Local queue's link (the suspended home link while a peer
    /// attachment is shown), else the peer attachment. The shown queue and
    /// the player panel resolve to one link (ADR 0034), so the owner's
    /// liveness, its status, and the transport commands are all this link's.
    pub(in crate::app) fn shown_queue_owner_player(&self) -> &mbv_player::PlayerProxy {
        self.queue_owner_player(self.playing_queue_link())
    }

    /// The confirmed status the player panel shows: the shown entry's owner's
    /// playback (design D7 of `gui-named-queues`, ADR 0041). Presentation and
    /// transport gates read this, so the panel, its transport, its track
    /// chips, and the queue edits all name one owner. The active Owner link's
    /// own event handling keeps reading `self.player`, which is that link's
    /// status whatever is shown.
    #[must_use]
    pub fn shown_playback_status(&self) -> mbv_ctrl::player::PlayerStatus {
        self.shown_queue_owner_player().status_snapshot()
    }

    ///
    /// # Panics
    ///
    /// Panics when the requested Peer link has no remote queue.
    pub fn queue_for_link(&self, link: QueueLink) -> &QueueView {
        match link {
            QueueLink::Local => &self.local_view,
            QueueLink::Peer => self
                .remote_view
                .as_ref()
                .expect("remote queue link requires remote queue"),
        }
    }

    /// Position of `slot_id` in `link`'s queue, if present.
    pub fn slot_index(&self, link: QueueLink, slot_id: QueueSlotId) -> Option<usize> {
        self.queue_for_link(link).slot_index(slot_id)
    }

    pub(in crate::app) fn remove_queue_slots_where(
        &mut self,
        predicate: impl Fn(&QueueItem) -> bool,
    ) {
        for link in [QueueLink::Local, QueueLink::Peer] {
            if link == QueueLink::Peer && !self.has_remote_queue() {
                continue;
            }
            let slot_ids = self
                .queue_for_link(link)
                .slots()
                .iter()
                .filter(|slot| predicate(&slot.item))
                .map(|slot| mbv_ctrl::slot_id_to_u64(slot.slot_id))
                .collect::<Vec<_>>();
            if !slot_ids.is_empty() {
                // The slot identities come from `link`'s own view, so the edit
                // names no Named queue: it clears the matches of the queue it
                // read, which is the current queue on the local link.
                self.queue_op(
                    QueueTarget::current(link),
                    mbv_remote_player::QueueOp::RemoveSlots { slot_ids },
                );
            }
        }
    }

    ///
    /// # Panics
    ///
    /// Panics when the requested Peer link has no remote queue.
    pub(in crate::app) fn queue_for_link_mut(&mut self, link: QueueLink) -> &mut QueueView {
        match link {
            QueueLink::Local => &mut self.local_view,
            QueueLink::Peer => self
                .remote_view
                .as_mut()
                .expect("remote queue link requires remote queue"),
        }
    }

    /// The queue view `target` names (design D7): one path decides which view
    /// an id maps to, for an edit of the shown queue, for an undone entry's
    /// queue, and for the cursor an undo places alike. The peer link's queue
    /// is the remote view; on the local link, a resting Named queue's own view
    /// answers while the Client shows it, and the current queue's `local_view`
    /// answers otherwise — the shown current queue's edits and the current
    /// queue's edits alike. `None` for a resting queue the Client no longer
    /// shows: it holds no Client-side view, so a position in it resolves
    /// against nothing (the owner addresses it by identity instead).
    pub(in crate::app) fn view_for(&mut self, target: QueueTarget) -> Option<&mut QueueView> {
        match target.queue {
            Some(queue) if self.named_queues.rests(queue) => {
                let (shown, view) = self.shown_resting.as_mut()?;
                (*shown == queue).then_some(view)
            }
            // The local link's current queue, and — `queue` being `None` —
            // the peer link's whole queue: the link decides when no Named
            // queue id does.
            None => match target.link {
                QueueLink::Peer => Some(self.queue_for_link_mut(QueueLink::Peer)),
                QueueLink::Local => Some(&mut self.local_view),
            },
            // A Named queue that is current (or that the Owner no longer
            // lists) is the current queue's view, whichever link is active:
            // a peer edit never names one.
            Some(_) => Some(&mut self.local_view),
        }
    }

    pub(in crate::app) fn undo_stack_for_link_mut(
        &mut self,
        link: QueueLink,
    ) -> &mut Vec<UndoEntry> {
        match link {
            QueueLink::Local => &mut self.queue_undo_stack,
            QueueLink::Peer => &mut self.remote_queue_undo_stack,
        }
    }

    /// How the shown queue and the playback target resolve to a link (ADR
    /// 0034, amended by ADR 0041). Only a GUI Client's resolution reads a
    /// shown entry: the TUI Client keeps the connection rule, whatever its
    /// shown-entry field holds.
    fn queue_link_resolution(&self) -> QueueLinkResolution {
        let shown_entry = if self.shows_queue_entries() {
            self.shown_entry
        } else {
            None
        };
        QueueLinkResolution::new(self.has_direct_remote_queue(), shown_entry)
    }

    pub(in crate::app) fn local_queue_metadata_applies(&self, link: QueueLink) -> bool {
        self.queue_link_resolution().local_metadata_applies(link)
    }

    pub(in crate::app) fn queue_link_is_playback(&self, link: QueueLink) -> bool {
        link == self.playing_queue_link()
    }

    pub(in crate::app) fn action_touches_local_queue(&self, _action: &PendingQueueAction) -> bool {
        // ADR 0034: each Player owner keeps its own queue and the shown
        // queue follows the connection, so every queue action lands on the
        // playback target's link — the action itself no longer picks one.
        self.local_queue_metadata_applies(self.playing_queue_link())
    }

    /// Adopt the owner's snapshot source and reconcile a pending
    /// playlist-save source update (design D6).
    pub(in crate::app) fn adopt_owner_source(&mut self, unified: &mbv_ctrl::UnifiedQueueStateData) {
        if let Some((source, lineage)) = self.pending_owner_source_update.clone() {
            if unified.lineage != lineage {
                self.pending_owner_source_update = None;
            } else if unified.source == source {
                self.pending_owner_source_update = None;
                self.queue_dirty = false;
            }
        }
    }

    pub(in crate::app) fn clear_local_queue_metadata(&mut self) {
        self.queue_dirty = false;
        self.queue_undo_stack.clear();
    }

    /// The queue link both the shown queue and the Player-owner playback
    /// resolve to (ADR 0034: each Player owner keeps its own queue, and the
    /// shown queue follows the connection, so the viewed and playing links
    /// are one link — there is no split accessor).
    pub fn playing_queue_link(&self) -> QueueLink {
        self.queue_link_resolution().playback_target()
    }

    /// The queue the Client presents: the peer's queue on the peer link, and
    /// on the local link the shown resting queue's own view when the GUI Client
    /// shows one, else the current queue's `local_view` (design D7).
    pub fn displayed_queue(&self) -> &QueueView {
        match self.playing_queue_link() {
            QueueLink::Local => self.shown_local_queue(),
            QueueLink::Peer => self.queue_for_link(QueueLink::Peer),
        }
    }

    /// The mutable displayed queue. Only presentation state (the cursor) is
    /// written through here: a resting queue's slots change only in the
    /// Owner's snapshots, which `adopt_named_queue_snapshot` adopts.
    pub(in crate::app) fn displayed_queue_mut(&mut self) -> &mut QueueView {
        match self.playing_queue_link() {
            QueueLink::Local => match &mut self.shown_resting {
                Some((queue, view)) if self.named_queues.rests(*queue) => view,
                _ => &mut self.local_view,
            },
            QueueLink::Peer => self.queue_for_link_mut(QueueLink::Peer),
        }
    }

    /// Position of `slot_id` in the Queue panel's shown queue: the slot
    /// addressing every edit, move, and row activation resolves against, so a
    /// row picked from a resting queue is never addressed in another queue
    /// (design D7).
    pub(in crate::app) fn displayed_slot_index(&self, slot_id: QueueSlotId) -> Option<usize> {
        self.displayed_queue().slot_index(slot_id)
    }

    pub fn playback_queue(&self) -> &QueueView {
        self.queue_for_link(self.playing_queue_link())
    }

    /// The queue item the playhead names in the playback queue, from a
    /// displayed playback state the caller already read. `None` while idle
    /// or past the shown queue's end.
    pub fn queue_item_at_playhead(
        &self,
        state: mbv_ui_model::playback::PlaybackState,
    ) -> Option<&mbv_queue::QueueItem> {
        state
            .active_idx
            .and_then(|idx| self.playback_queue().item_at(idx))
    }

    pub(in crate::app) fn playback_queue_mut(&mut self) -> &mut QueueView {
        self.queue_for_link_mut(self.playing_queue_link())
    }

    /// Whether the previous/next transport controls (playback-header mouse
    /// buttons and, implicitly, the `P`/`N` keys) are currently at a usable
    /// queue position: `(prev_available, next_available)`.
    ///
    /// `PlayerStatus::previous_idx`/`next_idx` fold in `active` and
    /// `queue_len`, so they answer both questions from the owner's status.
    pub(crate) fn transport_prev_next_available(&self) -> (bool, bool) {
        let st = self.shown_playback_status();
        (st.previous_idx().is_some(), st.next_idx().is_some())
    }

    /// Resolve the slot kind for the consume side effect. The Player owner
    /// has already applied its consume policy and reports the decision in the
    /// event's `consume` flag.
    ///
    /// # Panics
    ///
    /// Panics while reading the configuration when its lock is poisoned.
    pub(in crate::app) fn should_consume_slot(
        &self,
        slot_id: QueueSlotId,
        consume: bool,
    ) -> (bool, bool) {
        let item = self.playback_queue().slot(slot_id).map(|slot| &slot.item);
        let is_video = item.is_some_and(mbv_queue::QueueItem::is_video);
        let is_audio = item.is_some_and(mbv_queue::QueueItem::is_audio);
        let (consume_videos, consume_audio) = {
            let config = self.config.lock().unwrap();
            (config.consume_videos, config.consume_audio)
        };
        let should_consume =
            consume && ((is_video && consume_videos) || (is_audio && consume_audio));
        tracing::info!(
            name: "consume.slot.checked",
            target: "consume",
            slot = ?slot_id,
            consume,
            is_video,
            consume_videos,
            is_audio,
            consume_audio,
            should_consume,
            "consume check"
        );
        (should_consume, is_audio)
    }

    /// D5: the shown queue follows the connection, so the shown link only
    /// flips when `remote_view` is installed or cleared. When it flips, arm
    /// the cursor re-anchor: the component must adopt the newly shown
    /// queue's own follow position rather than reconcile by slot identity
    /// (the two queues hand out colliding `QueueSlotId`s, each a
    /// per-`PlaybackQueue` counter starting at 1).
    pub(in crate::app) fn reanchor_queue_cursor_after_link_change(
        &mut self,
        previous_shown: QueueLink,
    ) {
        let shown = self.playing_queue_link();
        if shown != previous_shown {
            self.pending_queue_cursor_reanchor = Some(shown);
        }
    }
}
