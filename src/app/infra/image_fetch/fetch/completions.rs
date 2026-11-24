//! The card-image fetch's drain and completion (`extract-client-core` design
//! D10): the one loop both Clients run — spawning queued requests inside the
//! in-flight limit, then folding each completion into the image cache. The
//! protocol encoding of a completed entry is the TUI Client shell's alone
//! (the TUI shell's `image_protocol`); this module queues the entry's
//! decoded source as a neutral record (`App::queue_protocol_encode`) and reaches
//! no shell type, so a Client with no terminal protocol keeps the same cache.

use super::super::super::super::App;
use super::super::{CardImageDrain, MAX_IMAGE_FETCHES};
use super::transport::{FetchedBytes, decode_fetched, fetch_image_bytes};
use crate::app::MusicEvent;
use mbv_images::{CachedImage, ImageFetchOutcome, ImageFetchReq, ImageSource};

impl App {
    /// Spawn queued image fetches until the in-flight limit is reached. Called
    /// whenever an in-flight fetch completes and frees a slot (see the card-image
    /// receiver in `images.rs`).
    pub(in crate::app) fn drain_image_fetches(&mut self) {
        while let Some(req) = self.images.take_pending_fetch(MAX_IMAGE_FETCHES) {
            self.spawn_image_fetch(req);
        }
    }

    /// Drain card-image completions into the image cache (design D10): the
    /// decoded source lands in the cache under its key, and — for a Client that
    /// paints terminal protocols — the entry awaits the shell's encoding
    /// (`Model`'s protocol store, design D2 rule 3 of
    /// `split-tui-gui-clients`). An interrupted fetch stays open to its retry,
    /// and a resolved artist-artwork request emits its typed event into the lib
    /// channel. The one loop both Clients run; the result reports whether a
    /// Series-family key completed, which only the TUI Client answers by
    /// re-pushing its TV workspace content (`Model::drain_card_image_completions`).
    pub fn drain_card_image_completions(&mut self) -> CardImageDrain {
        let mut drained = CardImageDrain::default();
        while let Ok((cache_key, outcome)) = self.images.try_recv_card_image() {
            drained.drained = true;
            // The infix is the only Series-family marker, so both live chains
            // (Wide's Thumb-first, narrow's `Primary`) gate without a suffix
            // list that can drift from the key the painter builds.
            drained.series_key_changed |=
                cache_key.contains(mbv_images::SERIES_IMAGE_CACHE_KEY_INFIX);
            let interrupted = matches!(outcome, ImageFetchOutcome::Interrupted);
            let (img_opt, source) = match outcome {
                ImageFetchOutcome::Image { img, source } => (img, Some(source)),
                ImageFetchOutcome::Empty | ImageFetchOutcome::Interrupted => (None, None),
            };
            // The entry stores before its encoding: the shell registers an
            // encoding only for a key the cache holds an entry for.
            let entry = CachedImage {
                img: img_opt,
                source,
                cover_box: None,
                applied_logo_key: None,
            };
            // Artist artwork: a fetch reserved through the typed artist
            // request identity completes as its own typed event; the generic
            // image cache stays provider/generic.
            let artist_completion =
                self.artist_artwork_requests
                    .remove(&cache_key)
                    .map(|identity| {
                        crate::app::LibEvent::Music(MusicEvent::ArtistArtworkFetched {
                            destination: identity.destination,
                            generation: mbv_core::service_runtime::SetupGeneration::new(
                                identity.generation,
                            ),
                            artist_id: identity.artist_id,
                            revision: identity.revision,
                            cache_key: cache_key.clone(),
                            available: entry.has_art(),
                        })
                    });
            if interrupted && artist_completion.is_none() {
                self.images.fetch_interrupted(cache_key);
            } else {
                // The shell encodes a key once its entry is in place; only the
                // key travels, so this tick clones no bitmap.
                let has_art = entry.has_art();
                self.images.complete_fetch(cache_key.clone(), entry);
                if has_art {
                    self.queue_protocol_encode(&cache_key);
                }
            }
            if let Some(event) = artist_completion {
                let _ = self.channels.lib_tx.send(event);
            }
        }
        drained
    }

    pub(super) fn spawn_image_fetch(&mut self, req: ImageFetchReq) {
        self.images.start_fetch();
        let (server_url, token) = if matches!(req.source, ImageSource::Emby) {
            let Some(client) = self.emby_client() else {
                self.images.fetch_start_failed(req.cache_key);
                return;
            };
            let c = client.lock().unwrap();
            (c.config.server_url.clone(), c.token.clone())
        } else {
            (String::new(), String::new())
        };
        let tx = self.images.card_image_tx().clone();
        let decode = self.image_decode;
        std::thread::spawn(move || {
            // catch_unwind so a panic during fetch/decode still reports a result,
            // freeing the in-flight slot and the loading reservation (H9). Exactly
            // one message is sent per spawn, so the receiver can balance the count.
            let cache_key = req.cache_key.clone();
            let cache_key_outer = cache_key.clone();
            let tx_outer = tx.clone();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                // Decode off the UI thread; the main loop only builds the protocol.
                let outcome = match fetch_image_bytes(req, &server_url, &token) {
                    FetchedBytes::Bytes(bytes) => decode_fetched(bytes, decode),
                    FetchedBytes::Missing => ImageFetchOutcome::Empty,
                    FetchedBytes::Interrupted => ImageFetchOutcome::Interrupted,
                };
                let _ = tx.send((cache_key, outcome));
            }));
            if result.is_err() {
                let _ = tx_outer.send((cache_key_outer, ImageFetchOutcome::Empty));
            }
        });
    }
}
