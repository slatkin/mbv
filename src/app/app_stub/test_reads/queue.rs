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

    /// Hold a queue edit's undo entry on `link`'s undo stack, as the edit
    /// that made it leaves it: the inverse the undo consumes.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_queue_undo_for_test(
        &mut self,
        link: mbv_ui_model::playback::QueueLink,
        entry: crate::app::state::playback::UndoEntry,
    ) {
        self.undo_stack_for_link_mut(link).push(entry);
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
        self.cached_cover_box(key) == Some(box_size)
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
}
