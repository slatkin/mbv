use super::{CachedImage, ImageFetchReq};
use crate::resize::{ResizeRegisterTx, ResizeResponseRx};
use crate::title_overlay::DERIVED_SEP;
use ratatui_image::picker::Picker;
use std::sync::mpsc;

pub struct ImageCache {
    card_image_states: std::collections::HashMap<String, CachedImage>,
    image_lru: std::collections::VecDeque<String>,
    cache_size: usize,
    card_image_loading: std::collections::HashSet<String>,
    last_card_height: u16,
    last_card_width: u16,
    painted_title_overlay_key: Option<String>,
    pending_image_fetches: std::collections::VecDeque<ImageFetchReq>,
    image_fetches_active: usize,
    card_image_tx: mpsc::Sender<(String, Option<image::DynamicImage>)>,
    card_image_rx: mpsc::Receiver<(String, Option<image::DynamicImage>)>,
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
        card_image_tx: mpsc::Sender<(String, Option<image::DynamicImage>)>,
        card_image_rx: mpsc::Receiver<(String, Option<image::DynamicImage>)>,
        resize_register_tx: ResizeRegisterTx,
        resize_response_rx: ResizeResponseRx,
    ) -> Self {
        Self {
            card_image_states: std::collections::HashMap::new(),
            image_lru: std::collections::VecDeque::new(),
            cache_size,
            card_image_loading: std::collections::HashSet::new(),
            last_card_height: 0,
            last_card_width: 0,
            painted_title_overlay_key: None,
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

    pub fn record_painted_title_overlay(&mut self, key: Option<String>) {
        self.painted_title_overlay_key = key;
    }

    #[must_use]
    pub fn painted_title_overlay_key(&self) -> Option<&str> {
        self.painted_title_overlay_key.as_deref()
    }

    #[must_use]
    pub fn has_loading_images(&self) -> bool {
        !self.card_image_loading.is_empty()
    }

    #[must_use]
    pub fn fetch_work_snapshot(&self) -> (std::collections::HashSet<String>, usize, usize) {
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
        self.clear_painted_if(|painted| painted == key);
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
        self.clear_painted_if(|painted| painted.starts_with(&prefix));
    }

    fn clear_painted_if(&mut self, pred: impl Fn(&str) -> bool) {
        if self.painted_title_overlay_key.as_deref().is_some_and(pred) {
            self.painted_title_overlay_key = None;
        }
    }

    pub fn clear_images_and_loading(&mut self) {
        self.card_image_states.clear();
        self.card_image_loading.clear();
        self.painted_title_overlay_key = None;
    }

    pub fn clear_audiobookshelf_images(&mut self) {
        let prefix = crate::AUDIOBOOKSHELF_CACHE_KEY_PREFIX;
        self.card_image_states
            .retain(|key, _| !key.starts_with(prefix));
        self.clear_painted_if(|painted| painted.starts_with(prefix));
        self.card_image_loading
            .retain(|key| !key.starts_with(prefix));
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
        if self.card_image_loading.contains(key) || self.card_image_states.contains_key(key) {
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

    pub fn fetch_start_failed(&mut self, key: String) {
        self.image_fetches_active = self.image_fetches_active.saturating_sub(1);
        let _ = self.card_image_tx.send((key, None));
    }

    pub fn complete_fetch(&mut self, key: String, entry: CachedImage) {
        self.card_image_loading.remove(&key);
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

    pub fn try_recv_card_image(
        &self,
    ) -> Result<(String, Option<image::DynamicImage>), mpsc::TryRecvError> {
        self.card_image_rx.try_recv()
    }

    pub fn try_recv_resize_response(
        &self,
    ) -> Result<(String, ratatui_image::thread::ResizeResponse), mpsc::TryRecvError> {
        self.resize_response_rx.try_recv()
    }

    #[must_use]
    pub fn card_image_tx(&self) -> &mpsc::Sender<(String, Option<image::DynamicImage>)> {
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
    use super::{CachedImage, ImageCache};
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
