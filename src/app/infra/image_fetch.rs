use super::super::{App, PAGE_SIZE};
use mbv_images::{audiobookshelf_hero_book_cover_cache_key, audiobookshelf_hero_cover_cache_key};
use std::time::Duration;
#[cfg(test)]
use std::time::Instant;

pub(in crate::app) const NAV_IDLE_DELAY: Duration = Duration::from_millis(150);
const MAX_IMAGE_FETCHES: usize = 6;

/// What one card-image completion drain changed (design D10): whether any
/// completion arrived, and whether one landed under a Series-family key. Only
/// a Series completion moves the TV workspace content — its painted key was
/// reserved when the placeholder painted, so a re-push is the only way the
/// cached image reaches the screen — and only the TUI Client paints that
/// workspace.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CardImageDrain {
    /// Whether any card-image completion arrived.
    pub drained: bool,
    /// Whether a completion's key is a Series-family key
    /// (`mbv_images::SERIES_IMAGE_CACHE_KEY_INFIX`): the TUI Client re-pushes
    /// its TV workspace content then.
    pub series_key_changed: bool,
}

impl App {
    /// Triggers the Audiobookshelf cover fetch for `library_item_id` and
    /// returns its image cache key, or `None` with no server configured.
    /// This is the hero projection's own entry (task 5.10): the key is
    /// hero-scoped so a plain consumer of the same cover — the queue card
    /// painting a playing episode of this show — keeps its uncropped
    /// encoding instead of the hero's cover-fit crop, and neither flashes.
    pub(in crate::app) fn audiobookshelf_cover_key(
        &mut self,
        library_item_id: &str,
    ) -> Option<String> {
        let setup = self.config.lock().unwrap().audiobookshelf_setup.clone()?;
        let cache_key = audiobookshelf_hero_cover_cache_key(
            &setup.server_url,
            library_item_id,
            self.image_protocol_suffix(),
        );
        if self.images.protocol_enabled() {
            self.fetch_audiobookshelf_image(
                cache_key.clone(),
                setup.server_url.clone(),
                library_item_id.to_string(),
            );
        }
        Some(cache_key)
    }

    /// Triggers the Audiobookshelf book-cover fetch for `library_item_id` and
    /// returns its isolated image cache key, or `None` with no server
    /// configured. The book-browsing spec requires book artwork to remain
    /// isolated from podcast artwork (line 124), and the hero scope keeps it
    /// off the queue card's entry (see [`audiobookshelf_cover_key`]).
    ///
    /// # Panics
    ///
    /// Panics when the configuration lock is unavailable.
    pub fn audiobookshelf_book_cover_key(&mut self, library_item_id: &str) -> Option<String> {
        let setup = self.config.lock().unwrap().audiobookshelf_setup.clone()?;
        let cache_key = audiobookshelf_hero_book_cover_cache_key(
            &setup.server_url,
            library_item_id,
            self.image_protocol_suffix(),
        );
        if self.images.protocol_enabled() {
            self.fetch_audiobookshelf_image(
                cache_key.clone(),
                setup.server_url.clone(),
                library_item_id.to_string(),
            );
        }
        Some(cache_key)
    }

    /// The configured Audiobookshelf server's URL, or `None` with no Service
    /// setup: the row cache key's server identity (design D6).
    pub(in crate::app) fn audiobookshelf_server_url(&self) -> Option<String> {
        self.config
            .lock()
            .unwrap()
            .audiobookshelf_setup
            .as_ref()
            .map(|setup| setup.server_url.clone())
    }

    /// The `Row` request for one Audiobookshelf cover (design D6): the row
    /// cache key holding the library item's update time, the book's width or
    /// the podcast cover's, and the Service credentials. `None` with no server
    /// configured or no stored credential.
    pub(in crate::app) fn audiobookshelf_row_request(
        &self,
        library_item_id: &str,
        book: bool,
        updated_at_ms: u64,
    ) -> Option<mbv_images::ImageFetchReq> {
        let setup = self.config.lock().unwrap().audiobookshelf_setup.clone()?;
        let api_key = mbv_config::load_service_secret(mbv_queue::ServiceKind::Audiobookshelf)?;
        Some(mbv_images::ImageFetchReq {
            cache_key: mbv_images::abs_row_cache_key(
                &setup.server_url,
                library_item_id,
                updated_at_ms,
            ),
            item_id: library_item_id.to_string(),
            series_id: String::new(),
            types: Vec::new(),
            source: mbv_images::ImageSource::Audiobookshelf {
                server_url: setup.server_url,
                api_key,
            },
            rendition: mbv_images::ImageRendition::Row(mbv_images::RowRendition {
                square: !book,
                emby: None,
            }),
        })
    }
}

pub(in crate::app) mod fetch;

#[cfg(test)]
mod tests {
    use super::Instant;
    use crate::app::tests::make_app_stub;
    use mbv_images::{composite_landscape_logo, series_image_cache_key};

    /// A 4:3 source filled into a 16:9 box is cropped top and bottom (design
    /// D5: the artwork fills its box; the excess is cropped, centred). The
    /// source has white bands in its top and bottom eighths so a squashed or
    /// letterboxed fit would show white at the box edges; only the cover
    /// crop removes them.
    #[test]
    fn landscape_logo_preserves_aspect_inset_blends_and_leaves_outside_unchanged() {
        let base = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            100,
            50,
            image::Rgba([20, 40, 60, 255]),
        ));
        let mut logo = image::RgbaImage::from_pixel(10, 20, image::Rgba([0, 0, 0, 0]));
        for y in 0..20 {
            for x in 0..5 {
                logo.put_pixel(x, y, image::Rgba([220, 100, 20, 128]));
            }
        }
        let composed = composite_landscape_logo(&base, &image::DynamicImage::ImageRgba8(logo));
        let pixels = composed.as_rgba8().unwrap();
        // 10:20 contains into 60:10 as 5:10; 5% insets round to (5, 3).
        assert_eq!(pixels.get_pixel(5, 3).0, [116, 68, 40, 255]);
        assert_eq!(pixels.get_pixel(6, 3).0, [137, 76, 39, 255]);
        // The transparent half of the non-uniform Logo and the surrounding art
        // remain the original pixels, pinning both the aspect fit and boundary.
        assert_eq!(pixels.get_pixel(10, 3).0, [20, 40, 60, 255]);
        assert_eq!(pixels.get_pixel(4, 2).0, [20, 40, 60, 255]);
        assert_eq!(pixels.get_pixel(99, 49).0, [20, 40, 60, 255]);
    }

    #[test]
    fn series_image_cache_key_pins_both_live_chains() {
        assert_eq!(
            series_image_cache_key("abc", &["Primary"]),
            "abc:ser:Primary"
        );
        assert_eq!(
            series_image_cache_key("abc", &["Thumb", "Primary", "Backdrop", "Logo"]),
            "abc:ser:Thumb,Primary,Backdrop,Logo"
        );
    }

    #[test]
    fn recent_navigation_blocks_list_card_image_fetch() {
        let mut app = make_app_stub();
        app.last_nav_at = Instant::now();
        app.fetch_list_card_image_when_idle(
            "recent-nav:P".into(),
            "recent-nav".into(),
            String::new(),
            &["Primary"],
        );
        assert!(!app.images.is_loading("recent-nav:P"));
        assert!(!app.images.is_cached("recent-nav:P"));
    }
}
