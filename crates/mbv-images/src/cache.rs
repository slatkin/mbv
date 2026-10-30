use super::{CachedImage, ImageFetchOutcome, ImageFetchReq};
use crate::resize::{ResizeRegisterTx, ResizeResponseRx};
use crate::title_overlay::DERIVED_SEP;
use ratatui_image::picker::Picker;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How long an interrupted key waits before a surface may fetch it again.
/// Retries are demand-driven: only a key a surface still asks for runs again.
const INTERRUPTED_FETCH_RETRY_DELAY: Duration = Duration::from_secs(10);

pub struct ImageCache {
    card_image_states: foldhash::HashMap<String, CachedImage>,
    image_lru: std::collections::VecDeque<String>,
    cache_size: usize,
    card_image_loading: foldhash::HashSet<String>,
    /// Keys whose last fetch got no answer, with the earliest retry time.
    /// A key leaves only when a fetch resolves (invariant 27).
    interrupted_fetches: foldhash::HashMap<String, Instant>,
    interrupted_fetch_retry_delay: Duration,
    last_card_height: u16,
    last_card_width: u16,
    pending_image_fetches: std::collections::VecDeque<ImageFetchReq>,
    image_fetches_active: usize,
    card_image_tx: mpsc::Sender<(String, ImageFetchOutcome)>,
    card_image_rx: mpsc::Receiver<(String, ImageFetchOutcome)>,
    /// Registers a freshly created per-cache-key `ResizeRequest` receiver
    /// with the resize worker thread (see `spawn_resize_worker`), so the
    /// worker can service many concurrently-alive `ThreadProtocol`s off the
    /// render thread while still routing each `ResizeResponse` back to the
    /// right cache entry (#164). `ResizeRequest`/`ResizeResponse` carry no key
    /// of their own — each cache key needs its own dedicated channel.
    resize_register_tx: ResizeRegisterTx,
    /// Completed off-thread resize+encode results, tagged with the cache key.
    resize_response_rx: ResizeResponseRx,
    image_picker: Option<Picker>,
    halfblock_picker: Option<Picker>,
    cache_size_total: usize,
    image_protocol: Option<String>,
    image_protocol_enabled: bool,
    #[cfg(any(test, feature = "test"))]
    card_image_fetch_calls: u32,
    #[cfg(any(test, feature = "test"))]
    image_protocol_builds: std::cell::Cell<u32>,
}

#[derive(Debug)]
pub enum FetchReservation {
    Duplicate,
    Queued,
    Start(ImageFetchReq),
}

impl ImageCache {
    #[must_use]
    pub fn new(
        cache_size: usize,
        image_protocol: Option<String>,
        image_protocol_enabled: bool,
        card_image_tx: mpsc::Sender<(String, ImageFetchOutcome)>,
        card_image_rx: mpsc::Receiver<(String, ImageFetchOutcome)>,
        resize_register_tx: ResizeRegisterTx,
        resize_response_rx: ResizeResponseRx,
    ) -> Self {
        Self {
            card_image_states: foldhash::HashMap::default(),
            image_lru: std::collections::VecDeque::new(),
            cache_size,
            card_image_loading: foldhash::HashSet::default(),
            interrupted_fetches: foldhash::HashMap::default(),
            interrupted_fetch_retry_delay: INTERRUPTED_FETCH_RETRY_DELAY,
            last_card_height: 0,
            last_card_width: 0,
            pending_image_fetches: std::collections::VecDeque::new(),
            image_fetches_active: 0,
            card_image_tx,
            card_image_rx,
            resize_register_tx,
            resize_response_rx,
            image_picker: None,
            halfblock_picker: None,
            cache_size_total: cache_size.saturating_mul(2),
            image_protocol,
            image_protocol_enabled,
            #[cfg(any(test, feature = "test"))]
            card_image_fetch_calls: 0,
            #[cfg(any(test, feature = "test"))]
            image_protocol_builds: std::cell::Cell::new(0),
        }
    }

    pub(crate) fn initialize_image_pickers(&mut self, image: Picker, halfblock: Picker) {
        self.image_picker = Some(image);
        self.halfblock_picker = Some(halfblock);
    }

    #[cfg(any(test, feature = "test"))]
    pub fn set_image_pickers_for_test(&mut self, image: Picker, halfblock: Picker) {
        self.initialize_image_pickers(image, halfblock);
    }

    #[must_use]
    pub fn image_picker(&self) -> Option<&Picker> {
        self.image_picker.as_ref()
    }

    #[must_use]
    pub fn halfblock_picker(&self) -> Option<&Picker> {
        self.halfblock_picker.as_ref()
    }

    #[must_use]
    pub fn protocol_enabled(&self) -> bool {
        self.image_protocol_enabled
    }

    #[must_use]
    pub fn protocol_override(&self) -> Option<&str> {
        self.image_protocol.as_deref()
    }

    /// Apply the selected protocol and whether image loading is active together.
    pub fn configure_protocol(&mut self, protocol: Option<String>, enabled: bool) {
        self.image_protocol = protocol;
        self.image_protocol_enabled = enabled;
    }

    #[must_use]
    pub fn cache_size(&self) -> usize {
        self.cache_size
    }

    #[must_use]
    pub fn last_card_size(&self) -> (u16, u16) {
        (self.last_card_height, self.last_card_width)
    }

    pub fn record_card_size(&mut self, height: u16, width: u16) {
        self.last_card_height = height;
        self.last_card_width = width;
    }

    #[must_use]
    pub fn has_loading_images(&self) -> bool {
        !self.card_image_loading.is_empty()
    }

    #[must_use]
    pub fn fetch_work_snapshot(&self) -> (foldhash::HashSet<String>, usize, usize) {
        (
            self.card_image_loading.clone(),
            self.image_fetches_active,
            self.pending_image_fetches.len(),
        )
    }

    #[must_use]
    pub fn is_loading(&self, key: &str) -> bool {
        self.card_image_loading.contains(key)
    }

    #[must_use]
    pub fn is_cached(&self, key: &str) -> bool {
        self.card_image_states.contains_key(key)
    }

    /// Whether `key`'s last fetch got no answer and no later fetch has
    /// resolved it. A surface treats this as art still to come.
    #[must_use]
    pub fn is_fetch_interrupted(&self, key: &str) -> bool {
        self.interrupted_fetches.contains_key(key)
    }

    /// Whether `key` is interrupted and its retry time has not come yet.
    #[must_use]
    pub fn is_fetch_retry_waiting(&self, key: &str) -> bool {
        self.interrupted_fetches
            .get(key)
            .is_some_and(|retry_at| Instant::now() < *retry_at)
    }

    #[must_use]
    pub fn image(&self, key: &str) -> Option<&CachedImage> {
        self.card_image_states.get(key)
    }

    pub fn image_mut(&mut self, key: &str) -> Option<&mut CachedImage> {
        self.card_image_states.get_mut(key)
    }

    pub fn insert_image(&mut self, key: String, entry: CachedImage) {
        self.remove_derived_variants_for(&key);
        self.card_image_states.insert(key, entry);
    }

    pub fn remove_image(&mut self, key: &str) -> Option<CachedImage> {
        self.image_lru.retain(|cached| cached != key);
        self.remove_derived_variants_for(key);
        self.card_image_states.remove(key)
    }

    pub fn insert_derived_image(&mut self, key: String, entry: CachedImage) {
        let Some((identity, _)) = key.split_once(DERIVED_SEP) else {
            self.card_image_states.insert(key, entry);
            return;
        };
        self.remove_derived_variants_for_identity(identity);
        self.card_image_states.insert(key, entry);
    }

    fn remove_derived_variants_for(&mut self, cache_key: &str) {
        self.remove_derived_variants_for_identity(crate::cache_identity(cache_key));
    }

    fn remove_derived_variants_for_identity(&mut self, identity: &str) {
        let prefix = format!("{identity}{DERIVED_SEP}");
        self.card_image_states
            .retain(|key, _| !key.starts_with(&prefix));
        self.image_lru.retain(|key| !key.starts_with(&prefix));
    }

    pub fn clear_images_and_loading(&mut self) {
        self.card_image_states.clear();
        self.card_image_loading.clear();
        self.interrupted_fetches.clear();
    }

    pub fn clear_audiobookshelf_images(&mut self) {
        let prefix = crate::AUDIOBOOKSHELF_CACHE_KEY_PREFIX;
        self.card_image_states
            .retain(|key, _| !key.starts_with(prefix));
        self.card_image_loading
            .retain(|key| !key.starts_with(prefix));
        self.interrupted_fetches
            .retain(|key, _| !key.starts_with(prefix));
        self.image_lru.retain(|key| !key.starts_with(prefix));
        self.pending_image_fetches
            .retain(|req| !matches!(req.source, super::ImageSource::Audiobookshelf { .. }));
    }

    pub fn clear_session_image_work(&mut self) {
        self.clear_images_and_loading();
        self.image_lru.clear();
        self.pending_image_fetches.clear();
        self.image_fetches_active = 0;
    }

    pub fn reserve_fetch(&mut self, req: ImageFetchReq, limit: usize) -> FetchReservation {
        let key = &req.cache_key;
        if self.card_image_loading.contains(key)
            || self.card_image_states.contains_key(key)
            || self.is_fetch_retry_waiting(key)
        {
            return FetchReservation::Duplicate;
        }
        self.card_image_loading.insert(req.cache_key.clone());
        if self.image_fetches_active >= limit {
            self.pending_image_fetches.push_back(req);
            FetchReservation::Queued
        } else {
            FetchReservation::Start(req)
        }
    }

    pub fn start_fetch(&mut self) {
        self.image_fetches_active += 1;
    }

    pub fn take_pending_fetch(&mut self, limit: usize) -> Option<ImageFetchReq> {
        if self.image_fetches_active >= limit {
            return None;
        }
        let req = self.pending_image_fetches.pop_front()?;
        Some(req)
    }

    /// The fetch could not start (no client yet). The receiver balances the
    /// in-flight count when it handles this message.
    pub fn fetch_start_failed(&mut self, key: String) {
        let _ = self
            .card_image_tx
            .send((key, ImageFetchOutcome::Interrupted));
    }

    /// Record a fetch that got no answer. No cache entry is written, because
    /// an entry with no image means the art does not exist.
    pub fn fetch_interrupted(&mut self, key: String) {
        self.card_image_loading.remove(&key);
        self.image_fetches_active = self.image_fetches_active.saturating_sub(1);
        self.interrupted_fetches
            .insert(key, Instant::now() + self.interrupted_fetch_retry_delay);
    }

    pub fn complete_fetch(&mut self, key: String, entry: CachedImage) {
        self.card_image_loading.remove(&key);
        self.interrupted_fetches.remove(&key);
        self.image_fetches_active = self.image_fetches_active.saturating_sub(1);
        self.remove_derived_variants_for(&key);
        if entry.img.is_some() {
            self.image_lru.retain(|cached| cached != &key);
            self.image_lru.push_back(key.clone());
            while self.image_lru.len() > self.cache_size_total {
                let Some(evict) = self.image_lru.pop_front() else {
                    break;
                };
                self.remove_image(&evict);
            }
        }
        self.card_image_states.insert(key, entry);
    }

    pub fn try_recv_card_image(&self) -> Result<(String, ImageFetchOutcome), mpsc::TryRecvError> {
        self.card_image_rx.try_recv()
    }

    pub fn try_recv_resize_response(
        &self,
    ) -> Result<(String, ratatui_image::thread::ResizeResponse), mpsc::TryRecvError> {
        self.resize_response_rx.try_recv()
    }

    /// Rebuild the resize worker after its thread exited — the only owner-
    /// visible sign of a caught `resize_encode` panic is the shared response
    /// channel closing (M-PANIC-CONTINUATION, issue #895). Spawns a fresh
    /// worker and drops every cached protocol: the old worker took each
    /// entry's per-key `ResizeRequest` receiver with it, so no cached
    /// protocol can receive resize results any more; every entry re-encodes
    /// and re-registers lazily on its next paint (via
    /// `cached_image_protocol_for_suffix_mut`'s missing-suffix rebuild path)
    /// from the retained source image.
    pub fn respawn_resize_worker(&mut self) {
        let (register_tx, response_rx) = crate::resize::spawn_resize_worker();
        self.resize_register_tx = register_tx;
        self.resize_response_rx = response_rx;
        for entry in self.card_image_states.values_mut() {
            entry.protocols.clear();
        }
    }

    #[must_use]
    pub fn card_image_tx(&self) -> &mpsc::Sender<(String, ImageFetchOutcome)> {
        &self.card_image_tx
    }

    pub(crate) fn resize_register_tx(&self) -> &ResizeRegisterTx {
        &self.resize_register_tx
    }

    #[cfg(any(test, feature = "test"))]
    pub fn record_protocol_build(&self) {
        self.image_protocol_builds
            .set(self.image_protocol_builds.get() + 1);
    }

    #[cfg(not(any(test, feature = "test")))]
    pub fn record_protocol_build(&self) {}

    pub fn reserve_card_image_fetch(
        &mut self,
        req: ImageFetchReq,
        limit: usize,
    ) -> FetchReservation {
        let reservation = self.reserve_fetch(req, limit);
        if !matches!(&reservation, FetchReservation::Duplicate) {
            self.record_fetch_reservation();
        }
        reservation
    }

    #[cfg(any(test, feature = "test"))]
    fn record_fetch_reservation(&mut self) {
        self.card_image_fetch_calls += 1;
    }

    #[cfg(not(any(test, feature = "test")))]
    fn record_fetch_reservation(&mut self) {
        let _ = self;
    }

    #[cfg(any(test, feature = "test"))]
    pub fn card_image_fetch_calls(&self) -> u32 {
        self.card_image_fetch_calls
    }

    #[cfg(any(test, feature = "test"))]
    pub fn image_protocol_builds(&self) -> u32 {
        self.image_protocol_builds.get()
    }

    #[cfg(any(test, feature = "test"))]
    pub fn set_cache_capacity_for_test(&mut self, total: usize) {
        self.cache_size_total = total;
    }
}

impl std::fmt::Debug for ImageCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageCache").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::{CachedImage, INTERRUPTED_FETCH_RETRY_DELAY, ImageCache};
    use std::sync::mpsc;

    fn cache(capacity: usize) -> ImageCache {
        let (card_image_tx, card_image_rx) = mpsc::channel();
        let (resize_register_tx, _) = mpsc::channel();
        let (_, resize_response_rx) = mpsc::channel();
        let mut cache = ImageCache::new(
            capacity,
            None,
            true,
            card_image_tx,
            card_image_rx,
            resize_register_tx,
            resize_response_rx,
        );
        cache.set_cache_capacity_for_test(capacity);
        cache
    }

    fn image() -> CachedImage {
        CachedImage {
            img: Some(image::DynamicImage::ImageRgba8(
                image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 255])),
            )),
            protocols: std::collections::HashMap::new(),
            cover_box: None,
            applied_logo_key: None,
        }
    }

    /// Issue #895 (M-PANIC-CONTINUATION): after the resize worker exits (a
    /// caught panic closes the shared response channel), the respawn must
    /// install a live worker and drop every cached protocol — the old
    /// worker took their per-key request receivers with it, so each entry
    /// re-encodes and re-registers on its next paint.
    #[test]
    fn respawn_replaces_a_dead_worker_and_clears_cached_protocols() {
        let mut cache = cache(2);
        let mut entry = image();
        let (tx, _rx) = mpsc::channel();
        entry.protocols.insert(
            "halfblock",
            ratatui_image::thread::ThreadProtocol::new(
                tx,
                Some(
                    ratatui_image::picker::Picker::halfblocks()
                        .new_resize_protocol(entry.img.clone().expect("test image")),
                ),
            ),
        );
        cache.insert_image("base:P".into(), entry);
        // The `cache` helper drops the response sender at construction —
        // the same disconnected channel a worker exit produces.
        assert!(matches!(
            cache.try_recv_resize_response(),
            Err(mpsc::TryRecvError::Disconnected)
        ));

        cache.respawn_resize_worker();

        assert!(
            matches!(
                cache.try_recv_resize_response(),
                Err(mpsc::TryRecvError::Empty)
            ),
            "the respawned worker's response channel must be live"
        );
        assert!(
            cache
                .image("base:P")
                .is_some_and(|entry| entry.protocols.is_empty()),
            "cached protocols must be dropped so they re-register with the new worker"
        );
    }

    fn fetch_req(key: &str) -> super::ImageFetchReq {
        super::ImageFetchReq {
            cache_key: key.into(),
            item_id: "item".into(),
            series_id: String::new(),
            types: Vec::new(),
            source: super::super::ImageSource::Emby,
        }
    }

    /// User report 2026-10-30: three episodes that timed out at 05:57 showed
    /// the no-image placeholder for the rest of the run. A key with no answer
    /// keeps no entry, waits out the delay, and can then fetch again.
    #[rstest::rstest]
    #[case::before_the_delay(INTERRUPTED_FETCH_RETRY_DELAY, false)]
    #[case::after_the_delay(std::time::Duration::ZERO, true)]
    fn interrupted_fetch_writes_no_entry_and_retries_after_the_delay(
        #[case] delay: std::time::Duration,
        #[case] retries: bool,
    ) {
        let mut cache = cache(2);
        cache.interrupted_fetch_retry_delay = delay;
        cache.reserve_fetch(fetch_req("item:P"), 6);

        cache.fetch_interrupted("item:P".into());

        assert!(!cache.is_cached("item:P"));
        assert!(cache.is_fetch_interrupted("item:P"));
        assert_eq!(
            matches!(
                cache.reserve_fetch(fetch_req("item:P"), 6),
                super::FetchReservation::Start(_)
            ),
            retries
        );
    }

    /// Review a05a530ba: derived overlay variants share the cache budget
    /// with base images — inserting a new variant replaces the old one
    /// without displacing bases, and removing a base drops its variants.
    #[test]
    fn review_a05a530ba_derived_variants_replace_and_die_with_their_base() {
        let mut cache = cache(2);
        cache.complete_fetch("base:P".into(), image());
        cache.complete_fetch("other:P".into(), image());

        cache.insert_derived_image("base:P:t:8x4:old".into(), image());
        cache.insert_derived_image("base:P:t:8x4:new".into(), image());

        assert!(cache.is_cached("base:P"));
        assert!(cache.is_cached("other:P"));
        assert!(!cache.is_cached("base:P:t:8x4:old"));
        assert!(cache.is_cached("base:P:t:8x4:new"));

        cache.remove_image("base:P");

        assert!(!cache.is_cached("base:P"));
        assert!(!cache.is_cached("base:P:t:8x4:new"));
        assert!(cache.is_cached("other:P"));
    }

    #[test]
    fn reported_movie_primary_reservation_does_not_block_queue_backdrop_fetch() {
        // Regression: a Primary-only reservation under {id}:P must not pin the queue card's landscape bytes.
        use crate::ImageSource;
        use crate::cache::FetchReservation;

        let mut cache = cache(2);
        let primary = super::ImageFetchReq {
            cache_key: "item:P".into(),
            item_id: "item".into(),
            series_id: String::new(),
            types: vec!["Primary".into()],
            source: ImageSource::Emby,
        };
        let landscape = super::ImageFetchReq {
            cache_key: "item:QB".into(),
            item_id: "item".into(),
            series_id: String::new(),
            types: vec!["Backdrop".into(), "Primary".into()],
            source: ImageSource::Emby,
        };

        assert!(matches!(
            cache.reserve_card_image_fetch(primary, 2),
            FetchReservation::Start(_)
        ));
        assert!(matches!(
            cache.reserve_card_image_fetch(landscape, 2),
            FetchReservation::Start(_)
        ));
    }
}
