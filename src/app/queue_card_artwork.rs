//! The Queue playback visual slot's artwork projection every Client
//! consumes (`add-tauri-gui-client` design D10): the paint state the
//! queue-only Panel mode's visual slot holds, with the cached image it
//! paints encoded for a window's display. The TUI's slot paints the same
//! cached image through its terminal protocol; a Client that paints no
//! terminal protocols reads the state here and encodes it for its own
//! presentation.

use crate::app::App;
use mbv_ui_model::playback::NowPlayingTitleSite;
use mbv_ui_model::playback_target::NowPlayingStatus;

/// What the Queue playback visual slot paints for one sync (design D10).
///
/// The image variant carries the slot's cached artwork encoded for display:
/// the same decoded pixels the TUI's slot paints, downscaled to the
/// [`ARTWORK_ENCODE_BOX`] box, so a frame's payload stays bounded whatever
/// the source's resolution. The placeholder variant paints the bundled
/// [`mbv_images::QUEUE_CARD_PLACEHOLDER_BYTES`] — the visual slot's
/// placeholder is not a cache entry in a Client that builds no terminal
/// protocols, so the bytes travel to the frame instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueueCardArtwork {
    /// The slot paints the cached artwork under `key`, display-encoded.
    Image { key: String, jpeg: Vec<u8> },
    /// The slot paints the bundled placeholder: a fetch resolved empty, or
    /// the slot's item carries no resolvable artwork.
    Placeholder,
    /// A fetch for the slot's key is in flight or awaits its retry: the
    /// slot paints the loading fill.
    Loading,
    /// The slot paints nothing: idle, the user's hide toggle, images off,
    /// or the visualizer owning the slot.
    Hidden,
}

/// The display encode box: the artwork encodes to fit within this square
/// (logical points at the GUI's reference density, covering a 2x scale
/// factor), so the encode's cost and a frame's payload stay bounded
/// whatever the source's resolution.
const ARTWORK_ENCODE_BOX: u32 = 448;

/// The JPEG quality the display encode writes: photographic artwork at the
/// display box keeps no visible loss above it, and the payload stays an
/// order of magnitude under a lossless PNG's.
const ARTWORK_ENCODE_QUALITY: u8 = 85;

/// JPEG-encode a decoded image at the display quality.
///
/// # Panics
///
/// Panics when the in-memory JPEG encode fails: a `Cursor` over a `Vec`
/// cannot fail to write, so a failure is a bug.
pub(in crate::app) fn encode_jpeg(img: &image::DynamicImage) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    let mut encoder =
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, ARTWORK_ENCODE_QUALITY);
    encoder
        .encode_image(img)
        .expect("JPEG encode of an in-memory cursor cannot fail");
    buf.into_inner()
}

impl App {
    /// Project the Queue playback visual slot's artwork (design D10). The
    /// sync the TUI's queue-column sync pass runs — `Model::sync_queue`'s
    /// card block, gated on the slot's shown state — runs here too: the
    /// projection issues the slot's fetches and the title site's decision,
    /// and the returned state is what the slot paints after them. A Client
    /// reads it once per frame build.
    ///
    /// The GUI's App records no card-size checkpoint (no terminal protocol
    /// paints), so the title site's transient size gate holds it on the
    /// header row and the title overlay's composed variants never build:
    /// the artwork stays the plain base art the slot's fetch resolves.
    #[must_use]
    pub fn queue_card_artwork(&mut self) -> QueueCardArtwork {
        if self.visual_slot_shown() {
            // The GUI has no resize drag: the hold is always off.
            self.refresh_queue_card_image(false);
        } else if self.now_playing_status() != NowPlayingStatus::Idle {
            // The slot is hidden while playing: the artwork can no longer
            // carry the title, so the header is its only home again — the
            // same reset the TUI's sync runs.
            self.queue_card_projection.title_site = NowPlayingTitleSite::Header;
        }
        if !self.visual_slot_shown() || self.visualizer_enabled || !self.images.images_enabled() {
            return QueueCardArtwork::Hidden;
        }
        let Some(cache_key) = self.queue_card_projection.cache_key.clone() else {
            return QueueCardArtwork::Placeholder;
        };
        let decoded = self
            .images
            .image(&cache_key)
            .and_then(|entry| entry.img.as_ref());
        if decoded.is_none() {
            // The slot's key has no decoded art yet: a fetch in flight or
            // awaiting its retry paints the loading fill, and a fetch that
            // resolved empty paints the placeholder — the TUI's slot's
            // resolution (`render_queue_playback_slot`).
            if self.images.is_loading(&cache_key) || self.images.is_fetch_interrupted(&cache_key) {
                return QueueCardArtwork::Loading;
            }
            return QueueCardArtwork::Placeholder;
        }
        self.encode_card_artwork(&cache_key)
    }

    /// The display-encoded artwork for `cache_key`'s cached image, encoded
    /// once per key and memoized (`card_artwork_encoded`).
    fn encode_card_artwork(&mut self, cache_key: &str) -> QueueCardArtwork {
        if self
            .card_artwork_encoded
            .as_ref()
            .is_none_or(|(key, _)| key != cache_key)
        {
            let Some(entry) = self
                .images
                .image(cache_key)
                .and_then(|entry| entry.img.clone())
            else {
                return QueueCardArtwork::Placeholder;
            };
            let encoded = encode_for_display(&entry);
            self.card_artwork_encoded = Some((cache_key.to_owned(), encoded));
        }
        let Some((key, encoded)) = self
            .card_artwork_encoded
            .as_ref()
            .filter(|(key, _)| key == cache_key)
        else {
            return QueueCardArtwork::Placeholder;
        };
        QueueCardArtwork::Image {
            key: key.clone(),
            jpeg: encoded.clone(),
        }
    }

    /// Enable card images for a Client's test: the auto protocol, enabled —
    /// the display path consults the cache's enabled gate alone.
    #[cfg(any(test, feature = "test"))]
    pub fn enable_card_images_for_test(&mut self) {
        self.images.configure_protocol(None, true);
    }

    /// Seed the card image cache with a decoded image under `key`, as a
    /// completed fetch would, so a Client's test reads the artwork state
    /// with no fetch in flight.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_card_image_for_test(&mut self, key: String, img: image::DynamicImage) {
        self.images.insert_image(
            key,
            mbv_images::CachedImage {
                img: Some(img),
                protocols: std::collections::HashMap::new(),
                cover_box: None,
                applied_logo_key: None,
            },
        );
    }
}

/// Encode `img` for display: downscaled to fit the [`ARTWORK_ENCODE_BOX`]
/// box with the card's render filter, then JPEG-encoded.
///
/// # Panics
///
/// Panics when the in-memory JPEG encode fails: a `Cursor` over a `Vec`
/// cannot fail to write, so a failure is a bug.
pub(in crate::app) fn encode_for_display(img: &image::DynamicImage) -> Vec<u8> {
    let resized = img.resize(
        ARTWORK_ENCODE_BOX,
        ARTWORK_ENCODE_BOX,
        image::imageops::FilterType::Triangle,
    );
    encode_jpeg(&resized)
}
