use super::{CachedImage, ImageFetchOutcome, ImageFetchReq};
use crate::resize::{ResizeRegisterTx, ResizeResponseRx};
use crate::title_overlay::DERIVED_SEP;
use ratatui_image::picker::Picker;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How long an interrupted key waits before a surface may fetch it again.
/// Retries are demand-driven: only a key a surface still asks for runs again.
const INTERRUPTED_FETCH_RETRY_DELAY: Duration = Duration::from_secs(10);

/// How many row images the memory cache keeps (design D8): a fixed budget,
/// separate from `image_cache_size`, so a long scroll never evicts a Hero or
/// queue-card image.
const ROW_IMAGE_CACHE_CAPACITY: usize = 512;

/// How many of the image fetch slots row fetches may hold (design D7). The
/// other slots stay free for a Hero or queue-card fetch, which starts first.
const ROW_IMAGE_FETCH_LIMIT: usize = 4;

pub struct ImageCache {
    card_image_states: foldhash::HashMap<String, CachedImage>,
    image_lru: std::collections::VecDeque<String>,
    /// Recency of the row images alone (design D8); row inserts evict only
    /// from this queue.
    row_image_lru: std::collections::VecDeque<String>,
    cache_size: usize,
    card_image_loading: foldhash::HashSet<String>,
    /// Keys whose last fetch got no answer, with the earliest retry time.
    /// A key leaves only when a fetch resolves (invariant 27).
    interrupted_fetches: foldhash::HashMap<String, Instant>,
    interrupted_fetch_retry_delay: Duration,
    last_card_height: u16,
    last_card_width: u16,
    pending_image_fetches: std::collections::VecDeque<ImageFetchReq>,
    /// Waiting row fetches (design D7), drained only after the Hero queue and
    /// only while `ROW_IMAGE_FETCH_LIMIT` slots or fewer are in flight.
    pending_row_fetches: std::collections::VecDeque<ImageFetchReq>,
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
            row_image_lru: std::collections::VecDeque::new(),
            cache_size,
            card_image_loading: foldhash::HashSet::default(),
            interrupted_fetches: foldhash::HashMap::default(),
            interrupted_fetch_retry_delay: INTERRUPTED_FETCH_RETRY_DELAY,
            last_card_height: 0,
            last_card_width: 0,
            pending_image_fetches: std::collections::VecDeque::new(),
            pending_row_fetches: std::collections::VecDeque::new(),
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
            self.pending_image_fetches.len() + self.pending_row_fetches.len(),
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

    /// Whether any interrupted key's retry time falls in `(after, until]`: the
    /// retry passed since the caller last looked. A surface that waits on an
    /// interrupted fetch polls this with the instant of its previous look.
    #[must_use]
    pub fn retry_passed_between(&self, after: Instant, until: Instant) -> bool {
        self.interrupted_fetches
            .values()
            .any(|retry_at| *retry_at > after && *retry_at <= until)
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
        self.row_image_lru.retain(|cached| cached != key);
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
        self.pending_row_fetches.clear();
    }

    pub fn clear_audiobookshelf_images(&mut self) {
        fn audiobookshelf(key: &str) -> bool {
            key.starts_with(crate::AUDIOBOOKSHELF_CACHE_KEY_PREFIX)
                || key.starts_with(crate::ABS_ROW_CACHE_KEY_PREFIX)
        }
        self.card_image_states.retain(|key, _| !audiobookshelf(key));
        self.card_image_loading.retain(|key| !audiobookshelf(key));
        self.interrupted_fetches
            .retain(|key, _| !audiobookshelf(key));
        self.image_lru.retain(|key| !audiobookshelf(key));
        self.row_image_lru.retain(|key| !audiobookshelf(key));
        self.pending_image_fetches
            .retain(|req| !matches!(req.source, super::ImageSource::Audiobookshelf { .. }));
        self.pending_row_fetches
            .retain(|req| !matches!(req.source, super::ImageSource::Audiobookshelf { .. }));
    }

    pub fn clear_session_image_work(&mut self) {
        self.clear_images_and_loading();
        self.image_lru.clear();
        self.row_image_lru.clear();
        self.pending_image_fetches.clear();
        self.pending_row_fetches.clear();
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

    /// Replace the waiting row fetches with `requests` (design D7): each
    /// `RowsInView` set replaces the queue, so a row that left the view before
    /// its fetch started is dropped. Dropping a request clears its loading
    /// mark (set here when it was queued), so a later reserve of the same key
    /// does not report `Duplicate`. A request already cached, loading, or
    /// waiting out a retry is skipped. Returns the requests to start now, at
    /// most `ROW_IMAGE_FETCH_LIMIT` in flight.
    pub fn replace_pending_row_fetches(
        &mut self,
        requests: Vec<ImageFetchReq>,
    ) -> Vec<ImageFetchReq> {
        let dropped: Vec<ImageFetchReq> = self.pending_row_fetches.drain(..).collect();
        for req in dropped {
            self.card_image_loading.remove(&req.cache_key);
        }
        let mut start = Vec::new();
        for req in requests {
            let key = &req.cache_key;
            if self.card_image_loading.contains(key)
                || self.card_image_states.contains_key(key)
                || self.is_fetch_retry_waiting(key)
            {
                continue;
            }
            self.card_image_loading.insert(key.clone());
            if self.image_fetches_active + start.len() < ROW_IMAGE_FETCH_LIMIT {
                start.push(req);
            } else {
                self.pending_row_fetches.push_back(req);
            }
        }
        start
    }

    /// The next fetch to start: the Hero/card queue drains first, so a waiting
    /// Hero always has a slot; row fetches then run only while fewer than
    /// `ROW_IMAGE_FETCH_LIMIT` fetches are in flight (design D7).
    pub fn take_pending_fetch(&mut self, limit: usize) -> Option<ImageFetchReq> {
        if self.image_fetches_active >= limit {
            return None;
        }
        if let Some(req) = self.pending_image_fetches.pop_front() {
            return Some(req);
        }
        if self.image_fetches_active >= ROW_IMAGE_FETCH_LIMIT {
            return None;
        }
        self.pending_row_fetches.pop_front()
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

    /// Clear every interrupted key's waiting time, as a test's passed retry
    /// would: no cache entry is written, so the next reserve may start the key
    /// again (invariant 27). A test that must observe the retry without waiting
    /// the delay calls this instead of sleeping.
    #[cfg(any(test, feature = "test"))]
    pub fn pass_interrupted_fetch_retries_for_test(&mut self) {
        self.interrupted_fetches.clear();
    }

    pub fn complete_fetch(&mut self, key: String, entry: CachedImage) {
        self.card_image_loading.remove(&key);
        self.interrupted_fetches.remove(&key);
        self.image_fetches_active = self.image_fetches_active.saturating_sub(1);
        self.remove_derived_variants_for(&key);
        if entry.has_art() {
            self.touch_cache_budget(&key);
        }
        self.card_image_states.insert(key, entry);
    }

    /// Mark the cached row image `key` as read now, so the row budget evicts
    /// by last read and not by insertion order: a row the window still paints
    /// outlives rows it scrolled past. A key that is not a cached row image
    /// changes nothing.
    pub fn touch_row_image(&mut self, key: &str) {
        if !crate::is_row_cache_key(key) || !self.card_image_states.contains_key(key) {
            return;
        }
        if let Some(position) = self.row_image_lru.iter().position(|cached| cached == key)
            && let Some(held) = self.row_image_lru.remove(position)
        {
            self.row_image_lru.push_back(held);
        }
    }

    /// Record `key` as the newest entry of its budget's recency queue and drop
    /// the entries that overflow it. Row images have their own budget (design
    /// D8): a row insert evicts row entries only, and a Hero/card insert only
    /// its own.
    fn touch_cache_budget(&mut self, key: &str) {
        let row = crate::is_row_cache_key(key);
        let capacity = if row {
            ROW_IMAGE_CACHE_CAPACITY
        } else {
            self.cache_size_total
        };
        let lru = if row {
            &mut self.row_image_lru
        } else {
            &mut self.image_lru
        };
        lru.retain(|cached| cached != key);
        lru.push_back(key.to_owned());
        let evicted = trim_budget(lru, capacity);
        for evict in evicted {
            self.remove_image(&evict);
        }
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

/// Drop `lru`'s oldest entries past `capacity`, returning their keys.
fn trim_budget(lru: &mut std::collections::VecDeque<String>, capacity: usize) -> Vec<String> {
    let mut evicted = Vec::new();
    while lru.len() > capacity {
        let Some(key) = lru.pop_front() else {
            break;
        };
        evicted.push(key);
    }
    evicted
}

impl std::fmt::Debug for ImageCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageCache").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CachedImage, INTERRUPTED_FETCH_RETRY_DELAY, ImageCache, ROW_IMAGE_CACHE_CAPACITY,
        ROW_IMAGE_FETCH_LIMIT,
    };
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
            source: None,
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
            rendition: super::super::ImageRendition::Hero,
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

    fn row_req(index: usize) -> super::ImageFetchReq {
        fetch_req(&format!("row:item:{index}"))
    }

    fn row_reqs(count: usize) -> Vec<super::ImageFetchReq> {
        (0..count).map(row_req).collect()
    }

    fn insert_rows(cache: &mut ImageCache, count: usize) {
        for index in 0..count {
            cache.complete_fetch(row_req(index).cache_key, image());
        }
    }

    fn start_fetches(cache: &mut ImageCache, count: usize) {
        for _ in 0..count {
            cache.start_fetch();
        }
    }

    /// Design D8: row images have their own 512-entry budget. Filling it
    /// evicts row entries only, so a Hero or queue-card image survives a long
    /// scroll.
    #[test]
    fn row_inserts_never_evict_a_non_row_image() {
        let mut cache = cache(1);
        cache.complete_fetch("item:P".into(), image());

        insert_rows(&mut cache, ROW_IMAGE_CACHE_CAPACITY + 1);

        assert!(cache.is_cached("item:P"), "the non-row image survives");
        assert!(
            cache.is_cached(&row_req(ROW_IMAGE_CACHE_CAPACITY).cache_key),
            "the newest row image is kept"
        );
        assert!(
            !cache.is_cached(&row_req(0).cache_key),
            "the oldest row image at the row budget was evicted"
        );
    }

    /// A row image the window read last survives the budget that evicts the
    /// older inserts: eviction follows the last read, not the insertion order
    /// (the 2026-11-20 TV Library thumbnail reload).
    #[test]
    fn a_touched_row_image_outlives_older_untouched_ones() {
        let mut cache = cache(1);
        insert_rows(&mut cache, ROW_IMAGE_CACHE_CAPACITY);
        let oldest = row_req(0).cache_key;
        cache.touch_row_image(&oldest);

        cache.complete_fetch(row_req(ROW_IMAGE_CACHE_CAPACITY).cache_key, image());

        assert!(cache.is_cached(&oldest), "the read row stays");
        assert!(
            !cache.is_cached(&row_req(1).cache_key),
            "the least recently read row is evicted"
        );
    }

    /// Design D7: a replaced in-view set drops a row request that never
    /// started and clears its loading mark, so its row fetches again when it
    /// comes back into view.
    #[test]
    fn a_replaced_row_set_drops_an_unstarted_request_and_clears_its_loading_mark() {
        let mut cache = cache(2);
        let waiting = row_req(ROW_IMAGE_FETCH_LIMIT).cache_key;

        let started = cache.replace_pending_row_fetches(row_reqs(ROW_IMAGE_FETCH_LIMIT + 1));

        assert_eq!(started.len(), ROW_IMAGE_FETCH_LIMIT);
        assert!(cache.is_loading(&waiting));

        assert!(cache.replace_pending_row_fetches(Vec::new()).is_empty());

        assert!(
            !cache.is_loading(&waiting),
            "dropping the waiting request clears its loading mark"
        );
        let restarted = cache.replace_pending_row_fetches(row_reqs(ROW_IMAGE_FETCH_LIMIT + 1));
        assert_eq!(restarted.len(), 1, "the dropped row fetches again");
    }

    /// Design D7: the Hero/card queue drains before the row queue, and row
    /// fetches stay below their own slot limit so a Hero always fits.
    #[test]
    fn a_waiting_hero_request_is_taken_before_a_waiting_row_request() {
        let mut cache = cache(2);
        start_fetches(&mut cache, ROW_IMAGE_FETCH_LIMIT);
        assert!(matches!(
            cache.reserve_fetch(fetch_req("item:P"), ROW_IMAGE_FETCH_LIMIT),
            super::FetchReservation::Queued
        ));
        assert!(
            cache
                .replace_pending_row_fetches(row_reqs(ROW_IMAGE_FETCH_LIMIT))
                .is_empty(),
            "rows wait while four fetches are in flight"
        );

        let first = cache
            .take_pending_fetch(6)
            .expect("the waiting Hero starts first");
        assert_eq!(first.cache_key, "item:P");
        assert!(
            cache.take_pending_fetch(6).is_none(),
            "no row starts while its slots are taken"
        );

        cache.fetch_interrupted("item:P".into());
        let row = cache
            .take_pending_fetch(6)
            .expect("a freed slot starts the first waiting row");
        assert!(row.cache_key.starts_with("row:"));
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
            rendition: crate::ImageRendition::Hero,
        };
        let landscape = super::ImageFetchReq {
            cache_key: "item:QB".into(),
            item_id: "item".into(),
            series_id: String::new(),
            types: vec!["Backdrop".into(), "Primary".into()],
            source: ImageSource::Emby,
            rendition: crate::ImageRendition::Hero,
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
