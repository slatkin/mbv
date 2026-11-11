//! The Queue playback visual slot's artwork projection every Client
//! consumes (`add-tauri-gui-client` design D10): the paint state the
//! queue-only Panel mode's visual slot holds, with the cached image it
//! paints. The TUI's slot paints the cached image through its terminal
//! protocol; a Client that paints no terminal protocols reads the state here
//! and hands the server's bytes to its own image pipeline.

use crate::app::App;
use mbv_images::SourceImage;
use mbv_ui_model::playback::NowPlayingTitleSite;
use mbv_ui_model::playback_target::NowPlayingStatus;

/// What the Queue playback visual slot paints for one sync (design D10).
///
/// The image variant carries the slot's cached artwork as the server sent it
/// ([`SourceImage`]): the bytes the TUI's slot decodes and paints, so a
/// Client shows the same pixels with no re-encode between. The fetch's own
/// size cap bounds a frame's payload. The placeholder variant paints the bundled
/// [`mbv_images::QUEUE_CARD_PLACEHOLDER_BYTES`] — the visual slot's
/// placeholder is not a cache entry in a Client that builds no terminal
/// protocols, so the bytes travel to the frame instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueueCardArtwork {
    /// The slot paints the cached artwork under `key`.
    Image { key: String, source: SourceImage },
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
        let source = self
            .images
            .image(&cache_key)
            .and_then(|entry| entry.source.clone());
        let Some(source) = source else {
            // The slot's key has no fetched art yet: a fetch in flight or
            // awaiting its retry paints the loading fill, and a fetch that
            // resolved empty paints the placeholder — the TUI's slot's
            // resolution (`render_queue_playback_slot`).
            if self.images.is_loading(&cache_key) || self.images.is_fetch_interrupted(&cache_key) {
                return QueueCardArtwork::Loading;
            }
            return QueueCardArtwork::Placeholder;
        };
        QueueCardArtwork::Image {
            key: cache_key,
            source,
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
                source: Some(test_source(&img)),
                img: Some(img),
                protocols: std::collections::HashMap::new(),
                cover_box: None,
                applied_logo_key: None,
            },
        );
    }
}

/// The server-bytes stand-in for a test's decoded image: its PNG encode.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub(crate) fn test_source(img: &image::DynamicImage) -> SourceImage {
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .expect("PNG encode of an in-memory cursor cannot fail");
    SourceImage::sniff(buf.into_inner()).expect("an encoded PNG is a recognised image")
}
