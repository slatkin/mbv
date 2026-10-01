use super::{CachedImage, ImageFetchReq};
use crate::resize::{ResizeRegisterTx, ResizeResponseRx};
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
        self.card_image_states.insert(key, entry);
    }

    pub fn remove_image(&mut self, key: &str) -> Option<CachedImage> {
        self.image_lru.retain(|cached| cached != key);
        if self.painted_title_overlay_key.as_deref() == Some(key) {
            self.painted_title_overlay_key = None;
        }
        self.card_image_states.remove(key)
    }

    pub fn insert_derived_image(&mut self, key: String, entry: CachedImage) {
        self.image_lru.retain(|cached| cached != &key);
        self.image_lru.push_back(key.clone());
        while self.image_lru.len() > self.cache_size_total {
            let Some(evict) = self.image_lru.pop_front() else {
                break;
            };
            if self.painted_title_overlay_key.as_deref() == Some(evict.as_str()) {
                self.painted_title_overlay_key = None;
            }
            self.card_image_states.remove(&evict);
        }
        self.card_image_states.insert(key, entry);
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
        if self
            .painted_title_overlay_key
            .as_deref()
            .is_some_and(|key| key.starts_with(prefix))
        {
            self.painted_title_overlay_key = None;
        }
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
        if entry.img.is_some() {
            self.image_lru.retain(|cached| cached != &key);
            self.image_lru.push_back(key.clone());
            while self.image_lru.len() > self.cache_size_total {
                let Some(evict) = self.image_lru.pop_front() else {
                    break;
                };
                if self.painted_title_overlay_key.as_deref() == Some(evict.as_str()) {
                    self.painted_title_overlay_key = None;
                }
                self.card_image_states.remove(&evict);
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
