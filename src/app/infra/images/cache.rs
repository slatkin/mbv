use super::{CachedImage, ImageFetchReq};
use crate::app::infra::resize::{ResizeRegisterTx, ResizeResponseRx};
use ratatui_image::picker::Picker;
use std::sync::mpsc;

pub(in crate::app) struct ImageCache {
    pub(in crate::app) card_image_states: std::collections::HashMap<String, CachedImage>,
    pub(in crate::app) image_lru: std::collections::VecDeque<String>,
    pub(in crate::app) cache_size: usize,
    pub(in crate::app) card_image_loading: std::collections::HashSet<String>,
    pub(in crate::app) last_card_height: u16,
    pub(in crate::app) last_card_width: u16,
    pub(in crate::app) pending_image_fetches: std::collections::VecDeque<ImageFetchReq>,
    pub(in crate::app) image_fetches_active: usize,
    pub(in crate::app) card_image_tx: mpsc::Sender<(String, Option<image::DynamicImage>)>,
    pub(in crate::app) card_image_rx: mpsc::Receiver<(String, Option<image::DynamicImage>)>,
    /// Registers a freshly created per-cache-key `ResizeRequest` receiver
    /// with the resize worker thread (see `spawn_resize_worker`), so the
    /// worker can service many concurrently-alive `ThreadProtocol`s off the
    /// render thread while still routing each `ResizeResponse` back to the
    /// right `card_image_states` entry (#164). `ResizeRequest`/`ResizeResponse`
    /// carry no key of their own — that's why each cache key gets its own
    /// dedicated channel instead of sharing one globally.
    pub(in crate::app) resize_register_tx: ResizeRegisterTx,
    /// Completed off-thread resize+encode results, tagged with the
    /// `card_image_states` cache key they belong to. Drained once per
    /// event-loop tick alongside `card_image_rx` (#164).
    pub(in crate::app) resize_response_rx: ResizeResponseRx,
    pub(in crate::app) image_picker: Option<Picker>,
    pub(in crate::app) halfblock_picker: Option<Picker>,
    pub(in crate::app) cache_size_total: usize,
    pub(in crate::app) image_protocol: Option<String>,
    pub(in crate::app) image_protocol_enabled: bool,
    /// Test-only instrumentation: counts every reservation `queue_card_image_fetch`
    /// makes past its dedup guard, so a broken guard is visible even when the
    /// fixture has no Emby client (`spawn_image_fetch` balances
    /// `image_fetches_active` back to its prior value synchronously in that
    /// case, hiding a redundant reservation from the other counters).
    #[cfg(test)]
    pub(in crate::app) card_image_fetch_calls: u32,
    /// Test-only instrumentation: counts protocol construction so hero cache
    /// validity tests can distinguish reuse from a rebuild.
    #[cfg(test)]
    pub(in crate::app) image_protocol_builds: std::cell::Cell<u32>,
}

impl ImageCache {
    pub(in crate::app) fn new(
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
            #[cfg(test)]
            card_image_fetch_calls: 0,
            #[cfg(test)]
            image_protocol_builds: std::cell::Cell::new(0),
        }
    }
}
