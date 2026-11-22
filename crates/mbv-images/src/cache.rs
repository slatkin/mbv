use super::{CachedImage, ImageFetchOutcome, ImageFetchReq};
use crate::title_overlay::DERIVED_SEP;
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
    cache_size_total: usize,
    image_protocol: Option<String>,
    image_protocol_enabled: bool,
    #[cfg(any(test, feature = "test"))]
    card_image_fetch_calls: u32,
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
            cache_size_total: cache_size.saturating_mul(2),
            image_protocol,
            image_protocol_enabled,
            #[cfg(any(test, feature = "test"))]
            card_image_fetch_calls: 0,
        }
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

    #[must_use]
    pub fn card_image_tx(&self) -> &mpsc::Sender<(String, ImageFetchOutcome)> {
        &self.card_image_tx
    }

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
    #[must_use]
    pub fn card_image_fetch_calls(&self) -> u32 {
        self.card_image_fetch_calls
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
mod tests;
