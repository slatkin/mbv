//! The terminal image protocol (design D7): the pickers, the per-cache-key
//! protocols, and the encode. `mbv-images` keeps the decoded art, the cache,
//! and the pixel processing; this store speaks `ratatui-image` on its behalf.
//!
//! The store is the only place a `ratatui-image` value lives. A cache entry
//! holds no protocol, and the cache holds no picker, so a Client that paints
//! no terminal protocol (`mbv-gui`) builds no encoding at all.
//!
//! A stored protocol never outlives the cache entry it was encoded from
//! (invariant 38). The store tags each key's protocols with the generation the
//! cache held for that key when they were built
//! ([`ImageCache::entry_generation`]), which changes whenever the cache stores
//! another entry under the key or drops it. So a key the cache evicted,
//! replaced, or cleared reads as having no protocol, and
//! [`ImageProtocols::prune`] drops the stored encodings of every key whose
//! entry moved on.

use mbv_images::cache::ImageCache;
use mbv_images::{CachedImage, SourceImage, mem_key};
use ratatui_image::picker::Picker;
use ratatui_image::thread::{ResizeRequest, ResizeResponse, ThreadProtocol};
use std::collections::HashMap;
use std::sync::mpsc;

use crate::resize::{ResizeRegisterTx, ResizeResponseRx, spawn_resize_worker};

/// A just-fetched cache entry and the protocol the store encoded for it.
pub struct BuiltEntry {
    pub entry: CachedImage,
    /// The protocol to record for the entry's cache key, once the caller has
    /// stored `entry` in the [`ImageCache`].
    pub protocol: Option<ThreadProtocol>,
}

impl std::fmt::Debug for BuiltEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuiltEntry")
            .field("entry", &self.entry)
            .field("protocol", &self.protocol.is_some())
            .finish()
    }
}

/// One cache key's protocols, tagged with the cache entry generation they were
/// encoded from.
struct EntryProtocols {
    generation: u64,
    by_suffix: HashMap<&'static str, ThreadProtocol>,
}

/// One Client's terminal image protocols: the pickers, the resize worker
/// channels, and the per-cache-key protocols the painters render.
pub struct ImageProtocols {
    /// Each cache key's protocols. A key's protocols are valid only while its
    /// tagged generation matches the cache's generation for the key, so the
    /// store never hands out an encoding of an entry the cache evicted or
    /// replaced.
    protocols: HashMap<String, EntryProtocols>,
    image_picker: Option<Picker>,
    halfblock_picker: Option<Picker>,
    /// Registers a freshly created per-cache-key `ResizeRequest` receiver
    /// with the resize worker thread (see `spawn_resize_worker`), so the
    /// worker can service many concurrently-alive `ThreadProtocol`s off the
    /// render thread while still routing each `ResizeResponse` back to the
    /// right cache entry (#164). `ResizeRequest`/`ResizeResponse` carry no key
    /// of their own — each cache key needs its own dedicated channel.
    resize_register_tx: ResizeRegisterTx,
    /// Completed off-thread resize+encode results, tagged with the cache key.
    resize_response_rx: ResizeResponseRx,
    /// How many protocols this store has encoded. Only tests read it, through
    /// [`ImageProtocols::protocol_builds`].
    protocol_builds: std::cell::Cell<u32>,
}

impl ImageProtocols {
    /// A store with a live resize worker.
    #[must_use]
    pub fn new() -> Self {
        let (resize_register_tx, resize_response_rx) = spawn_resize_worker();
        Self::with_worker(resize_register_tx, resize_response_rx)
    }

    fn with_worker(
        resize_register_tx: ResizeRegisterTx,
        resize_response_rx: ResizeResponseRx,
    ) -> Self {
        Self {
            protocols: HashMap::new(),
            image_picker: None,
            halfblock_picker: None,
            resize_register_tx,
            resize_response_rx,
            protocol_builds: std::cell::Cell::new(0),
        }
    }

    /// Populate `image_picker` (terminal-detected, with the config override)
    /// and `halfblock_picker` (the #451 dimmed-backdrop fallback).
    /// `protocol_override` is the configured protocol (`ImageCache`'s
    /// `protocol_override()`): a `Some` naming one of `sixel`, `kitty`,
    /// `iterm2`, or `halfblocks` replaces the detected protocol, and `None` or
    /// any other value keeps it.
    ///
    /// MUST run before the `TuiRealm` crossterm listener starts
    /// (`Application::init`): `Picker::from_query_stdio` writes a `CSI 16 t`
    /// cell-size query to the terminal and reads the reply with a raw stdin
    /// read. If the listener is already draining stdin it eats the reply, the
    /// picker falls back to a wrong cell size, and Kitty renders images clipped
    /// on the right/bottom (#654).
    pub fn init_image_pickers(&mut self, protocol_override: Option<&str>) {
        let picker = build_image_picker(protocol_override);
        tracing::debug!(
            name: "startup.image_picker.initialized",
            target: "startup",
            protocol = ?picker.protocol_type(),
            font_size = ?picker.font_size(),
            "image picker initialized"
        );
        self.initialize_image_pickers(picker, Picker::halfblocks());
    }

    fn initialize_image_pickers(&mut self, image: Picker, halfblock: Picker) {
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

    /// The picker that encodes the given protocol suffix.
    #[must_use]
    pub fn picker_for_suffix(&self, suffix: &'static str) -> Option<&Picker> {
        if suffix == "halfblock" {
            self.halfblock_picker().or(self.image_picker())
        } else {
            self.image_picker()
        }
    }

    /// Whether the configured protocol is halfblocks, from the override or
    /// from terminal detection.
    #[must_use]
    pub fn is_halfblock_configured(&self, protocol_override: Option<&str>) -> bool {
        protocol_override.is_some_and(|s| s.eq_ignore_ascii_case("halfblocks"))
            || self.image_picker().is_some_and(|p| {
                p.protocol_type() == ratatui_image::picker::ProtocolType::Halfblocks
            })
    }

    /// The configured protocol's name: the cache-key suffix its protocol is
    /// encoded under, and the name logs report.
    #[must_use]
    pub fn configured_protocol_name(&self) -> &'static str {
        use ratatui_image::picker::ProtocolType;
        match self.image_picker().map(Picker::protocol_type) {
            Some(ProtocolType::Sixel) => "sixel",
            Some(ProtocolType::Kitty) => "kitty",
            Some(ProtocolType::Iterm2) => "iterm2",
            Some(ProtocolType::Halfblocks) | None => "halfblock",
        }
    }

    /// Builds a fresh cache entry for a just-fetched image: keeps the decoded
    /// source so it can be re-encoded with a different protocol picker later
    /// (#451), and encodes the protocol for the active suffix.
    /// `img: None` with a `source` is a source-only fetch (design D14): no
    /// protocol is encoded, since no picker paints in such a Client.
    /// `img: None` with no `source` records a resolved-but-empty fetch (the
    /// "no art" marker renderers branch on).
    ///
    /// The caller stores `entry` in the [`ImageCache`] under `bare_key`, then
    /// records `protocol` with [`ImageProtocols::insert_protocol`].
    #[must_use]
    pub fn build_entry(
        &mut self,
        bare_key: &str,
        img: Option<image::DynamicImage>,
        source: Option<SourceImage>,
        suffix: &'static str,
    ) -> BuiltEntry {
        let entry = CachedImage {
            img,
            source,
            cover_box: None,
            applied_logo_key: None,
        };
        let mut protocol = None;
        if let Some(img) = entry.img.clone()
            && let Some(picker) = self.picker_for_suffix(suffix).cloned()
        {
            protocol = Some(self.build_protocol(bare_key, suffix, &picker, img));
        }
        BuiltEntry { entry, protocol }
    }

    /// Encode `img` as `suffix`'s protocol for `bare_key`, registering its
    /// dedicated resize channel with the worker.
    #[must_use]
    pub fn build_protocol(
        &mut self,
        bare_key: &str,
        suffix: &'static str,
        picker: &Picker,
        img: image::DynamicImage,
    ) -> ThreadProtocol {
        self.record_protocol_build();
        let mem_key = mem_key(bare_key, suffix);
        let (req_tx, req_rx) = mpsc::channel::<ResizeRequest>();
        let _ = self.resize_register_tx.send((mem_key, req_rx));
        ThreadProtocol::new(req_tx, Some(picker.new_resize_protocol(img)))
    }

    /// Record `protocol` as `bare_key`'s encoding for `suffix`, tagged with
    /// the cache entry's current generation. A protocol for a key the cache
    /// holds no entry for is dropped: nothing paints it. Recording also drops
    /// every protocol whose entry the cache has since replaced or dropped
    /// (invariant 38).
    pub fn insert_protocol(
        &mut self,
        cache: &ImageCache,
        bare_key: &str,
        suffix: &'static str,
        protocol: ThreadProtocol,
    ) {
        let Some(generation) = cache.entry_generation(bare_key) else {
            return;
        };
        self.prune(cache);
        self.protocols
            .entry(bare_key.to_owned())
            .or_insert_with(|| EntryProtocols {
                generation,
                by_suffix: HashMap::new(),
            })
            .by_suffix
            .insert(suffix, protocol);
    }

    /// `bare_key`'s protocol for `suffix`, when one is cached for the entry the
    /// cache still holds.
    #[must_use]
    pub fn protocol_mut(
        &mut self,
        cache: &ImageCache,
        bare_key: &str,
        suffix: &str,
    ) -> Option<&mut ThreadProtocol> {
        self.drop_key_if_stale(cache, bare_key);
        self.protocols.get_mut(bare_key)?.by_suffix.get_mut(suffix)
    }

    /// Whether `bare_key` has a protocol cached for `suffix` and the entry the
    /// cache still holds.
    #[must_use]
    pub fn contains(&mut self, cache: &ImageCache, bare_key: &str, suffix: &str) -> bool {
        self.drop_key_if_stale(cache, bare_key);
        self.protocols
            .get(bare_key)
            .is_some_and(|stored| stored.by_suffix.contains_key(suffix))
    }

    /// Whether any protocol is cached for `bare_key` and the entry the cache
    /// still holds.
    #[must_use]
    pub fn has_any(&mut self, cache: &ImageCache, bare_key: &str) -> bool {
        self.drop_key_if_stale(cache, bare_key);
        self.protocols
            .get(bare_key)
            .is_some_and(|stored| !stored.by_suffix.is_empty())
    }

    /// Drop every stored protocol whose cache entry the cache has since
    /// replaced or removed. A Client runs it at a sync point after cache
    /// mutation, so a key evicted while no painter asked about it does not
    /// linger in the store.
    pub fn prune(&mut self, cache: &ImageCache) {
        self.protocols
            .retain(|key, stored| cache.entry_generation(key) == Some(stored.generation));
    }

    /// Drop `bare_key`'s protocols when the entry they were encoded from is
    /// gone or has been replaced.
    fn drop_key_if_stale(&mut self, cache: &ImageCache, bare_key: &str) {
        let stale = self
            .protocols
            .get(bare_key)
            .is_some_and(|stored| cache.entry_generation(bare_key) != Some(stored.generation));
        if stale {
            self.protocols.remove(bare_key);
        }
    }

    /// Drop every protocol cached for `bare_key`, so the next paint re-encodes
    /// from the retained source.
    pub fn clear_key(&mut self, bare_key: &str) {
        self.protocols.remove(bare_key);
    }

    /// Resolve an optional Logo cache key to the key of a Logo that has decoded
    /// pixels to composite: a pending, absent, or failed Logo is not a
    /// decoration input, so the base-only protocol stays valid.
    pub fn ready_logo_key(
        &self,
        cache: &ImageCache,
        logo_cache_key: Option<&str>,
    ) -> Option<String> {
        logo_cache_key
            .filter(|key| cache.image(key).is_some_and(|entry| entry.img.is_some()))
            .map(str::to_owned)
    }

    /// Paint the ready Logo at `logo_cache_key` over `img`, or return `img`
    /// unchanged when there is none (design D3).
    #[must_use]
    pub fn decorate_with_logo(
        &self,
        cache: &ImageCache,
        img: image::DynamicImage,
        logo_cache_key: Option<&str>,
    ) -> image::DynamicImage {
        let Some(logo) = logo_cache_key
            .and_then(|key| cache.image(key))
            .and_then(|entry| entry.img.as_ref())
        else {
            return img;
        };
        mbv_images::composite_landscape_logo(&img, logo)
    }

    /// The next completed resize, tagged `"bare@suffix"`.
    pub fn try_recv_resize_response(&self) -> Result<(String, ResizeResponse), mpsc::TryRecvError> {
        self.resize_response_rx.try_recv()
    }

    /// Rebuild the resize worker after its thread exited — the only owner-
    /// visible sign of a caught `resize_encode` panic is the shared response
    /// channel closing (M-PANIC-CONTINUATION, issue #895). Spawns a fresh
    /// worker and drops every cached protocol: the old worker took each
    /// entry's per-key `ResizeRequest` receiver with it, so no cached
    /// protocol can receive resize results any more; every entry re-encodes
    /// and re-registers lazily on its next paint from the retained source
    /// image.
    pub fn respawn_resize_worker(&mut self) {
        let (register_tx, response_rx) = spawn_resize_worker();
        self.resize_register_tx = register_tx;
        self.resize_response_rx = response_rx;
        self.protocols.clear();
    }

    #[must_use]
    pub fn protocol_builds(&self) -> u32 {
        self.protocol_builds.get()
    }

    fn record_protocol_build(&self) {
        self.protocol_builds.set(self.protocol_builds.get() + 1);
    }
}

impl Default for ImageProtocols {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ImageProtocols {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageProtocols").finish_non_exhaustive()
    }
}

/// Query the terminal for its image protocol and apply the configured
/// protocol override when one is set.
fn build_image_picker(protocol_override: Option<&str>) -> Picker {
    use ratatui_image::picker::ProtocolType;
    let mut picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
    let proto = protocol_override.and_then(|s| match s.to_lowercase().as_str() {
        "sixel" => Some(ProtocolType::Sixel),
        "kitty" => Some(ProtocolType::Kitty),
        "iterm2" => Some(ProtocolType::Iterm2),
        "halfblocks" => Some(ProtocolType::Halfblocks),
        _ => None, // "auto" or unknown: use picker's detected protocol
    });
    if let Some(proto) = proto {
        picker.set_protocol_type(proto);
    }
    picker
}

#[cfg(test)]
mod tests;
