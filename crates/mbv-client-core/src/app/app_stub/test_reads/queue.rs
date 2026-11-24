//! The queue surfaces' test seeds and reads (`split-tui-gui-clients` design
//! D4b rule 5): the projected queue view, its adoptions, the undo stack, and
//! the artwork fetch counters the queue projection's fetches are observed
//! from.
//!
//! The shell's queue tests drove those shapes by writing the fields directly;
//! each seam here names the shape instead — a re-adopted owner snapshot, an
//! undo entry, a restored row — so those tests keep their claims and reach no
//! field.

use super::super::App;

impl App {
    /// Re-adopt an owner snapshot through the production adoption, with the
    /// cause the adoption arrived under: a replacement, or the owner's own
    /// answer.
    #[cfg(any(test, feature = "test"))]
    pub fn readopt_local_queue_for_test(
        &mut self,
        state: &mbv_ctrl::UnifiedQueueStateData,
        cause: crate::app::state::queue_view::AdoptCause,
    ) {
        self.local_view.adopt(state, cause);
    }

    /// Hold `state` as the local queue's projected view, adopted as a
    /// replacement.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_local_queue_snapshot_for_test(&mut self, state: &mbv_ctrl::UnifiedQueueStateData) {
        self.local_view = crate::app::QueueView::from_snapshot(state);
    }

    /// Hold `items` as the local queue's slots, cursor at `cursor`, as the
    /// Owner's queue adoption leaves them.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_local_queue_items_for_test(
        &mut self,
        items: Vec<mbv_queue::QueueItem>,
        cursor: usize,
    ) {
        let state = seeded_queue_state(items, None);
        self.local_view = crate::app::QueueView::from_snapshot(&state);
        self.local_view.set_cursor(cursor);
    }

    /// Hold `items` as the local queue's slots with `active` as the active
    /// slot, cursor at `cursor`, as the Owner's queue adoption leaves them.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_local_queue_items_with_active_for_test(
        &mut self,
        items: Vec<mbv_queue::QueueItem>,
        cursor: usize,
        active: usize,
    ) {
        let state = seeded_queue_state(items, Some(active));
        self.local_view = crate::app::QueueView::from_snapshot(&state);
        self.local_view.set_cursor(cursor);
    }

    /// Adopt `items` as the playing queue's slots, cursor at `cursor`, through
    /// the replacement adoption an Owner update runs: the queue the shipped
    /// playback link names, whichever view that is.
    #[cfg(any(test, feature = "test"))]
    pub fn adopt_playing_queue_items_for_test(
        &mut self,
        items: Vec<mbv_queue::QueueItem>,
        cursor: usize,
    ) {
        let state = seeded_queue_state(items, None);
        let queue = self.playback_queue_mut();
        queue.adopt(
            &state,
            crate::app::state::queue_view::AdoptCause::Replacement,
        );
        queue.set_cursor(cursor);
    }

    /// Adopt `items` as the playing queue's Emby slots, cursor at `cursor`, as
    /// [`Self::adopt_playing_queue_items_for_test`] does.
    #[cfg(any(test, feature = "test"))]
    pub fn adopt_playing_queue_emby_items_for_test(
        &mut self,
        items: Vec<mbv_emby_model::EmbyItem>,
        cursor: usize,
    ) {
        self.adopt_playing_queue_items_for_test(
            items
                .into_iter()
                .map(|item| mbv_queue::QueueItem::Emby(Box::new(item)))
                .collect(),
            cursor,
        );
    }

    /// Adopt `items` as the playing queue's slots with `active` as the active
    /// slot, cursor at `cursor`, as
    /// [`Self::adopt_playing_queue_items_for_test`] does.
    #[cfg(any(test, feature = "test"))]
    pub fn adopt_playing_queue_items_with_active_for_test(
        &mut self,
        items: Vec<mbv_queue::QueueItem>,
        cursor: usize,
        active: usize,
    ) {
        let state = seeded_queue_state(items, Some(active));
        let queue = self.playback_queue_mut();
        queue.adopt(
            &state,
            crate::app::state::queue_view::AdoptCause::Replacement,
        );
        queue.set_cursor(cursor);
    }

    /// Hold `items` as the local queue view's Emby slots, cursor at `cursor`,
    /// as the Owner's queue adoption leaves them: the shape the queue tests
    /// seed with Emby fixtures.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_local_queue_emby_items_for_test(
        &mut self,
        items: Vec<mbv_emby_model::EmbyItem>,
        cursor: usize,
    ) {
        self.seed_local_queue_items_for_test(
            items
                .into_iter()
                .map(|item| mbv_queue::QueueItem::Emby(Box::new(item)))
                .collect(),
            cursor,
        );
    }

    /// Hold `site` as where the now-playing title paints, as the projection
    /// pass leaves it once the artwork can carry it.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_queue_card_title_site_for_test(
        &mut self,
        site: mbv_ui_model::playback::NowPlayingTitleSite,
    ) {
        self.queue_card_projection.title_site = site;
    }

    /// The site the now-playing title paints on: the header, or the artwork
    /// once it can carry the title.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn queue_card_title_site_for_test(&self) -> mbv_ui_model::playback::NowPlayingTitleSite {
        self.queue_card_projection.title_site
    }

    /// The revision the projected queue view carries: the identity a repaint
    /// updates instead of rebuilding the rows.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn local_queue_revision_for_test(&self) -> mbv_queue::QueueRevision {
        self.local_view.revision()
    }

    /// Whether a direct remote queue is held: the shown queue resolves to the
    /// peer link.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn has_direct_remote_queue_for_test(&self) -> bool {
        self.has_direct_remote_queue()
    }

    /// Hold a removed queue item's undo entry on `link`'s undo stack, as the
    /// edit that removed `item` at `index` leaves it: the inverse re-appends
    /// the item there when the undo is consumed.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_queue_undo_remove_for_test(
        &mut self,
        link: mbv_ui_model::playback::QueueLink,
        index: usize,
        item: Box<mbv_queue::QueueItem>,
    ) {
        self.undo_stack_for_link_mut(link)
            .push(crate::app::state::playback::UndoEntry::Remove {
                target: crate::app::state::playback::QueueTarget::current(link),
                item,
                index,
            });
    }

    /// Hold a moved queue slot's undo entry on `link`'s undo stack, as the
    /// edit that moved `slot_id` away from `from` leaves it: the inverse moves
    /// the slot back when the undo is consumed.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_queue_undo_move_for_test(
        &mut self,
        link: mbv_ui_model::playback::QueueLink,
        slot_id: mbv_queue::QueueSlotId,
        from: usize,
    ) {
        self.undo_stack_for_link_mut(link)
            .push(crate::app::state::playback::UndoEntry::Move {
                target: crate::app::state::playback::QueueTarget::current(link),
                slot_id,
                from,
            });
    }

    /// The playback state the shown queue displays, through the production
    /// resolution: the state a test asserts the queue's rows and playhead
    /// against.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn displayed_queue_playback_state_for_test(&self) -> mbv_ui_model::playback::PlaybackState {
        self.displayed_queue_playback_state()
    }

    /// Whether the shown queue's previous and next transport positions are
    /// usable, through the production read: `(prev_available, next_available)`.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn transport_prev_next_available_for_test(&self) -> (bool, bool) {
        self.transport_prev_next_available()
    }

    /// Hold `source` as the local queue view's source, as a queue adopted from
    /// a saved playlist or an album browse leaves it: the shape the consume
    /// and save paths read.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_local_queue_source_for_test(&mut self, source: mbv_queue::QueueSource) {
        let state = mbv_ctrl::UnifiedQueueStateData {
            queue: None,
            start_slot: None,
            status: mbv_ctrl::player::PlayerStatus::default(),
            active_slot: None,
            slots: self
                .local_view
                .slots()
                .iter()
                .map(|slot| mbv_ctrl::UnifiedQueueSlot {
                    slot_id: mbv_ctrl::slot_id_to_u64(slot.slot_id),
                    item: slot.item.clone(),
                })
                .collect(),
            revision: 1,
            source,
            lineage: mbv_queue::QueueLineage::default(),
            in_flight_transition: None,
            queued_latest_transition: None,
        };
        let cursor = self.local_view.cursor();
        self.local_view.adopt(
            &state,
            crate::app::state::queue_view::AdoptCause::Replacement,
        );
        self.local_view.set_cursor(cursor);
    }

    /// Whether the local queue holds unsaved edits: the flag a save prompt
    /// reads before it replaces or drops the queue.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn queue_dirty_for_test(&self) -> bool {
        self.queue_dirty
    }

    /// The slot the recorded delete waits to stop: the pending-delete marker
    /// the Stopped event clears, or `None` while no delete is recorded.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn pending_delete_slot_for_test(&self) -> Option<mbv_queue::QueueSlotId> {
        self.pending_delete_slot
    }

    /// The slots the recorded multi-delete waits to stop, or `None` while no
    /// multi-delete is recorded.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn pending_delete_selection_for_test(&self) -> Option<Vec<mbv_queue::QueueSlotId>> {
        self.pending_delete_selection.clone()
    }

    /// Whether a queue transfer waits to settle: the pending transfer the
    /// observation tick settles or fails.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn pending_queue_transfer_for_test(&self) -> bool {
        self.pending_queue_transfer.is_some()
    }

    /// Whether a peer's queue is held beside the home queue: the state a
    /// direct remote-control connection installs.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn has_remote_queue_for_test(&self) -> bool {
        self.has_remote_queue()
    }

    /// Remove the slot at `pos` through the production removal path, as the
    /// queue panel's delete does.
    #[cfg(any(test, feature = "test"))]
    pub fn remove_from_queue_for_test(&mut self, pos: usize) {
        self.remove_from_queue(pos);
    }

    /// Remove `slot_ids` from `target`'s queue through the production
    /// multi-delete path, as the queue panel's selection delete does.
    #[cfg(any(test, feature = "test"))]
    pub fn remove_slots_from_queue_for_test(
        &mut self,
        target: crate::app::QueueTarget,
        slot_ids: &[mbv_queue::QueueSlotId],
    ) {
        self.remove_slots_from_queue(target, slot_ids);
    }

    /// Undo `link`'s last queue edit through the production undo path, as the
    /// queue panel's undo does.
    #[cfg(any(test, feature = "test"))]
    pub fn undo_last_queue_edit_for_test(&mut self, link: mbv_ui_model::playback::QueueLink) {
        self.undo_last_queue_edit(link);
    }

    /// Send `op` on `target`'s queue link through the production queue-op
    /// path, as the queue panel's edit does.
    #[cfg(any(test, feature = "test"))]
    pub fn queue_op_for_test(
        &mut self,
        target: crate::app::QueueTarget,
        op: mbv_remote_player::QueueOp,
    ) {
        self.queue_op(target, op);
    }

    /// Wait for `id`'s queue-op answer through the production pump, as the
    /// edit path does before the next input.
    #[cfg(any(test, feature = "test"))]
    pub fn await_queue_op_for_test(
        &mut self,
        link: mbv_ui_model::playback::QueueLink,
        id: mbv_ctrl::QueueOpId,
    ) {
        self.await_queue_op(link, id);
    }

    /// Wait for `id`'s queue-op answer with the pump bound to `bound`, as the
    /// timeout-path tests drive it.
    #[cfg(any(test, feature = "test"))]
    pub fn await_queue_op_with_bound_for_test(
        &mut self,
        link: mbv_ui_model::playback::QueueLink,
        id: mbv_ctrl::QueueOpId,
        bound: std::time::Duration,
    ) {
        self.await_queue_op_with_bound(link, id, bound);
    }

    /// Adopt `snapshot` as `link`'s queue through the production owner-update
    /// path, as a background owner broadcast does.
    #[cfg(any(test, feature = "test"))]
    pub fn handle_unified_queue_updated_for_test(
        &mut self,
        link: mbv_ui_model::playback::QueueLink,
        snapshot: &mbv_ctrl::UnifiedQueueStateData,
    ) {
        self.handle_unified_queue_updated(link, snapshot);
    }

    /// The queue link the shown playback resolves to: the one an edit
    /// addresses.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn active_queue_link_for_test(&self) -> mbv_ui_model::playback::QueueLink {
        self.active_queue_link()
    }

    /// Whether `link`'s undo stack holds no entry: whether the undo consumed
    /// the one an edit made.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn queue_undo_is_empty_for_test(&self, link: mbv_ui_model::playback::QueueLink) -> bool {
        match link {
            mbv_ui_model::playback::QueueLink::Local => self.queue_undo_stack.is_empty(),
            mbv_ui_model::playback::QueueLink::Peer => self.remote_queue_undo_stack.is_empty(),
        }
    }

    /// Whether the queue projection's now-playing fetch holds `key`, and the
    /// projection's fetch-work snapshot beside it. The projection issues the
    /// visual slot's fetch, so a test observes the request it started and
    /// that a repaint tick started none.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn queue_card_fetch_loading_for_test(&self, key: &str) -> bool {
        self.images.is_loading(key)
    }

    /// The projection's fetch work: how many fetches are active and how many
    /// are queued beside them. A repaint tick starts none, so both counts
    /// hold.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn queue_card_fetch_work_for_test(&self) -> (usize, usize) {
        let (_, active, queued) = self.images.fetch_work_snapshot();
        (active, queued)
    }

    /// How many card image fetches the store started: `0` while the visual
    /// slot is hidden, and above it once the slot shows.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn card_image_fetch_count_for_test(&self) -> u32 {
        self.images.card_image_fetch_calls()
    }

    /// Whether the image store paints a terminal image protocol at all.
    #[cfg(any(test, feature = "test"))]
    pub fn enable_image_protocol_for_test(&mut self) {
        self.images.configure_protocol(None, true);
    }

    /// Hold `enabled` as whether the configured picker paints, over the
    /// picker `protocol` names (`None` keeps the configured default).
    #[cfg(any(test, feature = "test"))]
    pub fn configure_image_protocol_for_test(&mut self, protocol: Option<String>, enabled: bool) {
        self.images.configure_protocol(protocol, enabled);
    }

    /// Hold `entry` as what the artwork fetch for `key` landed, as the fetch's
    /// own completion leaves the store.
    #[cfg(any(test, feature = "test"))]
    pub fn complete_card_image_fetch_for_test(
        &mut self,
        key: &str,
        entry: mbv_images::CachedImage,
    ) {
        self.images.complete_fetch(key.to_owned(), entry);
    }

    /// Let every interrupted artwork fetch's retry time pass, as the retry
    /// clock would once its backoff elapses.
    #[cfg(any(test, feature = "test"))]
    pub fn pass_interrupted_fetch_retries_for_test(&mut self) {
        self.images.pass_interrupted_fetch_retries_for_test();
    }

    /// Drop the artwork entry stored under `key`, as a generation change or a
    /// logged-out session does.
    #[cfg(any(test, feature = "test"))]
    pub fn remove_cached_image_for_test(&mut self, key: &str) {
        self.images.remove_image(key);
    }

    /// Store `entry` under `key`, as a landed fetch does.
    #[cfg(any(test, feature = "test"))]
    pub fn insert_cached_image_for_test(&mut self, key: &str, entry: mbv_images::CachedImage) {
        self.images.insert_image(key.to_owned(), entry);
    }

    /// Whether the artwork entry stored under `key` was last encoded for
    /// `box_size`.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn cached_cover_box_is_for_test(&self, key: &str, box_size: (u16, u16)) -> bool {
        self.images.image(key).and_then(|entry| entry.cover_box) == Some(box_size)
    }

    /// Note that the card image fetch for `key` got no answer, as a dropped
    /// request leaves it.
    #[cfg(any(test, feature = "test"))]
    pub fn interrupt_card_image_fetch_for_test(&mut self, key: &str) {
        self.images.fetch_interrupted(key.to_owned());
    }

    /// Drop the decoded pixels the artwork entry stored under `key` keeps,
    /// leaving its source.
    #[cfg(any(test, feature = "test"))]
    pub fn drop_cached_pixels_for_test(&mut self, key: &str) {
        if let Some(entry) = self.images.image_mut(key) {
            entry.img = None;
        }
    }

    /// Hold the queue card projection, as a sync pass computes it for the
    /// shown slot.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_queue_card_projection_for_test(
        &mut self,
        projection: mbv_ui_model::playback::QueueCardProjection,
    ) {
        self.queue_card_projection = projection;
    }

    /// Hold the dimming-backdrop fact, which forces the shell's halfblocks
    /// fallback over a differently configured picker (#451).
    #[cfg(any(test, feature = "test"))]
    pub fn set_dim_backdrop_active_for_test(&mut self, active: bool) {
        self.set_dim_backdrop_active(active);
    }

    /// The artwork entry stored under `key`, for a test that reads its stored
    /// pixels or its absence directly.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn cached_image_for_test(&self, key: &str) -> Option<&mbv_images::CachedImage> {
        self.cached_image(key)
    }

    /// The decoded pixels of the artwork entry stored under `key`.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn cached_image_pixels_for_test(&self, key: &str) -> Option<image::DynamicImage> {
        self.cached_image_pixels(key)
    }

    /// The source the local queue's projected view carries, as the last
    /// adoption left it.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn local_queue_source_for_test(&self) -> &mbv_queue::QueueSource {
        self.local_view.source()
    }

    /// Reserve `req` on the image cache as an in-flight card fetch, as a
    /// painted card's fetch does.
    #[cfg(any(test, feature = "test"))]
    pub fn reserve_image_fetch_for_test(&mut self, req: mbv_images::ImageFetchReq) {
        let _ = self.images.reserve_fetch(req, 0);
    }

    /// Whether an album-track replacement waits behind the confirm gate, as a
    /// play on a populated target queue leaves it.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn has_gated_replacement_for_test(&self) -> bool {
        self.queue_deferrals.has_gated_replacement()
    }

    /// Adopt `items` as the direct remote link's projected queue, starting at
    /// `cursor`, as the peer's snapshot leaves it. The fixture keeps a target
    /// queue: a test that adopts into it holds a routed peer.
    ///
    /// # Panics
    ///
    /// Panics when the `App` holds no direct remote view.
    #[cfg(any(test, feature = "test"))]
    pub fn adopt_direct_remote_queue_items_for_test(
        &mut self,
        items: Vec<mbv_emby_model::EmbyItem>,
        cursor: usize,
    ) {
        let slots: Vec<_> = items
            .into_iter()
            .enumerate()
            .map(|(index, item)| mbv_ctrl::UnifiedQueueSlot {
                slot_id: index as u64 + 1,
                item: mbv_queue::QueueItem::Emby(Box::new(item)),
            })
            .collect();
        let state = mbv_ctrl::UnifiedQueueStateData {
            queue: None,
            start_slot: None,
            status: mbv_ctrl::player::PlayerStatus {
                current_idx: cursor,
                ..Default::default()
            },
            active_slot: None,
            slots,
            revision: 1,
            source: mbv_queue::QueueSource::default(),
            lineage: mbv_queue::QueueLineage::default(),
            in_flight_transition: None,
            queued_latest_transition: None,
        };
        let view = self
            .remote_view
            .as_mut()
            .expect("the direct remote fixture keeps a target queue");
        view.adopt(
            &state,
            crate::app::state::queue_view::AdoptCause::Replacement,
        );
        view.set_cursor(cursor);
    }

    /// Hold `source` and `lineage` as the source update this Client sent and
    /// waits for the owner to confirm.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_pending_owner_source_update_for_test(
        &mut self,
        source: mbv_queue::QueueSource,
        lineage: mbv_queue::QueueLineage,
    ) {
        self.pending_owner_source_update = Some((source, lineage));
    }

    /// The source update this Client sent and waits for the owner to confirm.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn pending_owner_source_update_for_test(
        &self,
    ) -> Option<(mbv_queue::QueueSource, mbv_queue::QueueLineage)> {
        self.pending_owner_source_update.clone()
    }

    /// Hold `dirty` as whether the queue carries edits the owner has not
    /// confirmed.
    #[cfg(any(test, feature = "test"))]
    pub fn set_queue_dirty_for_test(&mut self, dirty: bool) {
        self.queue_dirty = dirty;
    }

    /// The Client-local queue epoch a mutation is made against.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn queue_epoch_for_test(&self) -> crate::app::state::queue_owner::QueueEpoch {
        self.queue_epoch
    }

    /// The origin a mutation is made against: this Client's epoch plus
    /// `lineage` as the owner reported it.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn queue_origin_with_lineage_for_test(
        &self,
        lineage: mbv_queue::QueueLineage,
    ) -> crate::app::state::queue_owner::QueueOrigin {
        crate::app::state::queue_owner::QueueOrigin {
            epoch: self.queue_epoch,
            lineage,
        }
    }

    /// Adopt the home link's `snapshot` as the local queue, as the Client's
    /// drain does when the owner reports one.
    #[cfg(any(test, feature = "test"))]
    pub fn adopt_home_snapshot_for_test(&mut self, snapshot: &mbv_ctrl::UnifiedQueueStateData) {
        self.adopt_home_snapshot(snapshot);
    }

    /// Apply a saved playlist's `source` change at `origin`, answering whether
    /// the update reached the owner link.
    #[cfg(any(test, feature = "test"))]
    pub fn apply_saved_playlist_source_for_test(
        &mut self,
        source: mbv_queue::QueueSource,
        origin: crate::app::state::queue_owner::QueueOrigin,
    ) -> bool {
        self.apply_saved_playlist_source(source, origin)
    }
}

/// The owner snapshot that adopts `items` as slots, `active` naming the active
/// slot and the playhead: the state both the local view's seed and the playing
/// queue's adoption are built from.
#[cfg(any(test, feature = "test"))]
fn seeded_queue_state(
    items: Vec<mbv_queue::QueueItem>,
    active: Option<usize>,
) -> mbv_ctrl::UnifiedQueueStateData {
    let slots: Vec<_> = items
        .into_iter()
        .enumerate()
        .map(|(index, item)| mbv_ctrl::UnifiedQueueSlot {
            slot_id: u64::try_from(index).unwrap_or(u64::MAX) + 1,
            item,
        })
        .collect();
    mbv_ctrl::UnifiedQueueStateData {
        queue: None,
        start_slot: None,
        status: mbv_ctrl::player::PlayerStatus {
            current_idx: active.unwrap_or_default(),
            ..Default::default()
        },
        active_slot: active.and_then(|index| slots.get(index).map(|slot| slot.slot_id)),
        slots,
        revision: 1,
        source: mbv_queue::QueueSource::Unknown,
        lineage: mbv_queue::QueueLineage::default(),
        in_flight_transition: None,
        queued_latest_transition: None,
    }
}
