use crate::app::{App, PendingQueueAction, QueueLink, QueueLinkResolution, QueueView, UndoEntry};
use mbv_queue::{QueueItem, QueueSlotId};

impl App {
    pub(in crate::app) fn has_remote_queue(&self) -> bool {
        self.remote_view.is_some()
    }

    pub(in crate::app) fn has_direct_remote_queue(&self) -> bool {
        self.has_remote_queue()
    }

    /// Return the Player link and event receiver that own the requested
    /// queue. Local resolves to the suspended home link when present.
    pub(in crate::app) fn queue_link(
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
    pub(in crate::app) fn local_queue_player(&self) -> &mbv_player::PlayerProxy {
        self.suspended_local
            .as_ref()
            .map_or(&self.player, |home| &home.player)
    }

    pub(in crate::app) fn queue_for_link(&self, link: QueueLink) -> &QueueView {
        match link {
            QueueLink::Local => &self.local_view,
            QueueLink::Peer => self
                .remote_view
                .as_ref()
                .expect("remote queue link requires remote queue"),
        }
    }

    /// Position of `slot_id` in `link`'s queue, if present.
    pub(in crate::app) fn slot_index(
        &self,
        link: QueueLink,
        slot_id: QueueSlotId,
    ) -> Option<usize> {
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
                self.queue_op(link, mbv_remote_player::QueueOp::RemoveSlots { slot_ids });
            }
        }
    }

    pub(in crate::app) fn queue_for_link_mut(&mut self, link: QueueLink) -> &mut QueueView {
        match link {
            QueueLink::Local => &mut self.local_view,
            QueueLink::Peer => self
                .remote_view
                .as_mut()
                .expect("remote queue link requires remote queue"),
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

    fn queue_link_resolution(&self) -> QueueLinkResolution {
        QueueLinkResolution::new(self.has_direct_remote_queue(), self.queue_link)
    }

    pub(in crate::app) fn local_queue_metadata_applies(&self, link: QueueLink) -> bool {
        self.queue_link_resolution().local_metadata_applies(link)
    }

    pub(in crate::app) fn queue_link_is_playback(&self, link: QueueLink) -> bool {
        link == self.playing_queue_link()
    }

    fn action_queue_link(&self, action: &PendingQueueAction) -> QueueLink {
        match action {
            PendingQueueAction::PlayItems { .. } => self.playing_queue_link(),
            PendingQueueAction::ClearQueue => self.viewed_queue_link(),
        }
    }

    pub(in crate::app) fn action_touches_local_queue(&self, action: &PendingQueueAction) -> bool {
        self.local_queue_metadata_applies(self.action_queue_link(action))
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

    pub(in crate::app) fn playing_queue_link(&self) -> QueueLink {
        self.queue_link_resolution().playback_target()
    }

    pub(in crate::app) fn viewed_queue_link(&self) -> QueueLink {
        self.queue_link_resolution().visible_scope()
    }

    pub(in crate::app) fn displayed_queue(&self) -> &QueueView {
        self.queue_for_link(self.viewed_queue_link())
    }

    pub(in crate::app) fn displayed_queue_mut(&mut self) -> &mut QueueView {
        self.queue_for_link_mut(self.viewed_queue_link())
    }

    pub(in crate::app) fn playback_queue(&self) -> &QueueView {
        self.queue_for_link(self.playing_queue_link())
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
    pub(in crate::app) fn transport_prev_next_available(&self) -> (bool, bool) {
        let st = self.player.status_snapshot();
        (st.previous_idx().is_some(), st.next_idx().is_some())
    }

    /// Resolve the slot kind for the consume side effect. The Player owner
    /// has already applied its consume policy and reports the decision in the
    /// event's `consume` flag.
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

    /// Connected to an mbv daemon/client: the peer's queue is the displayed
    /// queue, empty or not. A Local-endpoint attach has no remote tab, so the
    /// link resolution lands back on the adopted unified Local view.
    pub(in crate::app) fn display_peer_queue_on_connect(&mut self) {
        self.set_queue_link(QueueLink::Peer);
    }

    pub(in crate::app) fn set_queue_link(&mut self, link: QueueLink) {
        let resolved = if link == QueueLink::Peer && self.has_direct_remote_queue() {
            QueueLink::Peer
        } else {
            QueueLink::Local
        };
        if resolved != self.queue_link {
            // A real link switch (`[`/`]`, link pill, session
            // attach/detach) must adopt the new link's own follow
            // position rather than reconciling by slot identity: the two
            // queues hand out colliding `QueueSlotId`s (each is a
            // per-`PlaybackQueue` counter starting at 1), so identity
            // reconciliation can park the cursor on an unrelated slot.
            self.pending_queue_cursor_reanchor = Some(resolved);
        }
        self.queue_link = resolved;
    }
}
