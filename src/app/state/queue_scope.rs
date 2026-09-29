use crate::app::{App, PendingQueueAction, PlayerTab, QueueScope, QueueScopeResolution, UndoEntry};
use mbv_queue::{QueueMutationResult, QueueSlotId};

impl App {
    pub(in crate::app) fn has_remote_queue(&self) -> bool {
        self.remote_player_tab.is_some()
    }

    pub(in crate::app) fn has_direct_remote_queue(&self) -> bool {
        self.player.as_remote().is_some() && self.has_remote_queue()
    }

    /// Whether the scope's canonical queue is the playback owner's accepted
    /// submission. The Stay-alive owner is authoritative from its snapshots,
    /// while direct remote scope is already independently projected.
    pub(in crate::app) fn local_queue_is_owner_queue(&self, _scope: QueueScope) -> bool {
        self.player.as_remote().is_some()
    }

    /// Return the Player link and event receiver that own the requested
    /// queue. Local resolves to the suspended home link when present.
    pub(in crate::app) fn queue_link(
        &mut self,
        scope: QueueScope,
    ) -> (
        &mbv_player::PlayerProxy,
        &mut std::sync::mpsc::Receiver<mbv_ctrl::player::PlayerEvent>,
    ) {
        match (scope, self.suspended_local.as_mut()) {
            (QueueScope::Local, Some(home)) => (&home.player, &mut home.player_rx),
            _ => (&self.player, &mut self.player_rx),
        }
    }

    /// The `PlayerProxy` that owns the Local queue's state: the suspended home
    /// link when present, else the current player. Read-only counterpart of
    /// [`App::queue_link`] for `QueueScope::Local`.
    pub(in crate::app) fn local_queue_player(&self) -> &mbv_player::PlayerProxy {
        self.suspended_local
            .as_ref()
            .map_or(&self.player, |home| &home.player)
    }

    pub(in crate::app) fn queue_for_scope(&self, scope: QueueScope) -> &PlayerTab {
        match scope {
            QueueScope::Local => &self.player_tab,
            QueueScope::Remote => self
                .remote_player_tab
                .as_ref()
                .expect("remote queue scope requires remote queue"),
        }
    }

    /// Position of `slot_id` in `scope`'s queue, if present.
    pub(in crate::app) fn slot_index(
        &self,
        scope: QueueScope,
        slot_id: QueueSlotId,
    ) -> Option<usize> {
        self.queue_for_scope(scope)
            .slots()
            .iter()
            .position(|slot| slot.slot_id == slot_id)
    }

    pub(in crate::app) fn queue_for_scope_mut(&mut self, scope: QueueScope) -> &mut PlayerTab {
        match scope {
            QueueScope::Local => &mut self.player_tab,
            QueueScope::Remote => self
                .remote_player_tab
                .as_mut()
                .expect("remote queue scope requires remote queue"),
        }
    }

    /// Adopt the owner's snapshot source and reconcile a pending
    /// playlist-save source update (design D6).

    pub(in crate::app) fn undo_stack_for_scope_mut(
        &mut self,
        scope: QueueScope,
    ) -> &mut Vec<UndoEntry> {
        match scope {
            QueueScope::Local => &mut self.queue_undo_stack,
            QueueScope::Remote => &mut self.remote_queue_undo_stack,
        }
    }

    fn queue_scope_resolution(&self) -> QueueScopeResolution {
        QueueScopeResolution::new(self.has_direct_remote_queue(), self.queue_scope)
    }

    pub(in crate::app) fn local_queue_metadata_applies(&self, scope: QueueScope) -> bool {
        self.queue_scope_resolution().local_metadata_applies(scope)
    }

    pub(in crate::app) fn queue_scope_is_playback(&self, scope: QueueScope) -> bool {
        scope == self.playing_queue_scope()
    }

    fn action_queue_scope(&self, action: &PendingQueueAction) -> QueueScope {
        match action {
            PendingQueueAction::PlayItems { .. } => self.playing_queue_scope(),
            PendingQueueAction::ClearQueue => self.viewed_queue_scope(),
        }
    }

    pub(in crate::app) fn action_touches_local_queue(&self, action: &PendingQueueAction) -> bool {
        self.local_queue_metadata_applies(self.action_queue_scope(action))
    }

    /// Adopt the owner's snapshot source and reconcile a pending
    /// playlist-save source update (design D6).
    pub(in crate::app) fn adopt_owner_source(&mut self, unified: &mbv_ctrl::UnifiedQueueStateData) {
        self.queue_source = unified.source.clone();
        if let Some((source, lineage)) = self.pending_owner_source_update.clone() {
            if unified.lineage != lineage {
                self.pending_owner_source_update = None;
            } else if unified.source == source {
                self.pending_owner_source_update = None;
                self.queue_dirty = false;
                self.clear_local_playlist_entry_ids();
            }
        }
    }

    pub(in crate::app) fn clear_local_queue_metadata(&mut self) {
        self.queue_dirty = false;
        self.queue_undo_stack.clear();
    }

    pub(in crate::app) fn playing_queue_scope(&self) -> QueueScope {
        self.queue_scope_resolution().playback_target()
    }

    pub(in crate::app) fn viewed_queue_scope(&self) -> QueueScope {
        self.queue_scope_resolution().visible_scope()
    }

    pub(in crate::app) fn displayed_queue(&self) -> &PlayerTab {
        self.queue_for_scope(self.viewed_queue_scope())
    }

    pub(in crate::app) fn displayed_queue_mut(&mut self) -> &mut PlayerTab {
        self.queue_for_scope_mut(self.viewed_queue_scope())
    }

    pub(in crate::app) fn playback_queue(&self) -> &PlayerTab {
        self.queue_for_scope(self.playing_queue_scope())
    }

    pub(in crate::app) fn playback_queue_mut(&mut self) -> &mut PlayerTab {
        self.queue_for_scope_mut(self.playing_queue_scope())
    }

    /// Whether the previous/next transport controls (playback-header mouse
    /// buttons and, implicitly, the `P`/`N` keys) are currently at a usable
    /// queue position: `(prev_available, next_available)`.
    ///
    /// A connected remote session exposes no queue-position/length fields in
    /// `SessionInfo` (see `mbv_emby::SessionInfo`), so there is no way to
    /// tell whether it's at a queue boundary; both remain available there,
    /// mirroring `Command::PreviousTrack`/`Command::NextTrack`'s dispatch, which
    /// calls `session_jump_track` unconditionally for a connected session with
    /// no boundary check. Local playback uses `PlayerStatus::previous_idx`/
    /// `next_idx`, which already fold in `active` and `queue_len`.
    pub(in crate::app) fn transport_prev_next_available(&self) -> (bool, bool) {
        if self.connected_session_id.is_some() {
            return (true, true);
        }
        let st = self.player.status.lock().unwrap();
        (st.previous_idx().is_some(), st.next_idx().is_some())
    }

    /// Slot-keyed check of whether a completed/stopped queue slot should be
    /// consumed, given a player-reported completion (`consume`) and the
    /// type-specific consume flags. Returns `(should_consume, is_audio)` --
    /// callers that act on the removal need `is_audio` afterward to route to
    /// `on_video_consumed`/`on_audio_consumed`. Resolves the audio/video
    /// flag from the queue model by slot identity instead of raw index.
    pub(in crate::app) fn should_consume_slot(
        &self,
        slot_id: QueueSlotId,
        consume: bool,
    ) -> (bool, bool) {
        let item = self.playback_queue().queue.slot(slot_id).map(|s| &s.item);
        let is_video = item.is_some_and(mbv_queue::QueueItem::is_video);
        let is_audio = item.is_some_and(mbv_queue::QueueItem::is_audio);
        let (consume_videos, consume_audio) = {
            let config = self.config.lock().unwrap();
            let cfg = &*config;
            (cfg.consume_videos, cfg.consume_audio)
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

    /// Removes a completed slot from the local playback queue by identity.
    /// Uses `consume_slot` rather than `remove_slot` so a slot that is
    /// currently marked active can still be consumed. Returns the removed
    /// item's id, or `None` if the slot no longer exists.
    pub(in crate::app) fn consume_slot_from_active_playback_queue(
        &mut self,
        slot_id: QueueSlotId,
    ) -> Option<String> {
        let removed = match self.playback_queue_mut().queue.consume_slot(slot_id) {
            QueueMutationResult::Applied(slot) => slot,
            QueueMutationResult::NotFound => return None,
        };
        self.playback_queue_mut().clamp_cursor();
        Some(removed.item.id().to_string())
    }

    /// Connected to an mbv daemon/client: the peer's queue is the displayed
    /// queue, empty or not. A Local-endpoint attach has no remote tab, so the
    /// scope resolution lands back on the adopted unified Local view.
    pub(in crate::app) fn display_peer_queue_on_connect(&mut self) {
        self.set_queue_scope(QueueScope::Remote);
    }

    pub(in crate::app) fn set_queue_scope(&mut self, scope: QueueScope) {
        let resolved = if scope == QueueScope::Remote && self.has_direct_remote_queue() {
            QueueScope::Remote
        } else {
            QueueScope::Local
        };
        if resolved != self.queue_scope {
            // A real scope switch (`[`/`]`, scope pill, session
            // attach/detach) must adopt the new scope's own follow
            // position rather than reconciling by slot identity: the two
            // queues hand out colliding `QueueSlotId`s (each is a
            // per-`PlaybackQueue` counter starting at 1), so identity
            // reconciliation can park the cursor on an unrelated slot.
            self.pending_queue_cursor_reanchor = Some(resolved);
        }
        self.queue_scope = resolved;
    }
}
