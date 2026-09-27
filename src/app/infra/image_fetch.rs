use super::super::{App, LibEvent, PAGE_SIZE};
use crate::app::state::app_struct::{LevelFillAction, LevelFillState};
use mbv_images::{audiobookshelf_hero_book_cover_cache_key, audiobookshelf_hero_cover_cache_key};
use mbv_render::{PANE_PAD_X, PANE_PAD_Y};
use std::time::{Duration, Instant};

pub(in crate::app) const NAV_IMAGE_FETCH_IDLE_DELAY: Duration = Duration::from_millis(150);
const MAX_IMAGE_FETCHES: usize = 6;

fn wide_landscape_hero_eligible(
    artwork: &mbv_components::library_panel::HeroArtwork,
    panel_area: ratatui::layout::Rect,
) -> bool {
    artwork.shape == mbv_components::library_panel::ArtworkShape::Landscape
        && mbv_render::wide_hero_fits(panel_area)
}
impl App {
    /// One panel hero's projected image state (task 5.10, design D9): the
    /// projection — never the painter — issues every fetch, then projects the
    /// state painting reads. `panel_area` is the Library panel's `RootFrame`
    /// content area; the Wide header's cover-fit box is keyed by the box
    /// `hero_artwork_box` derives from it, so a resize, split drag, or
    /// Workspace shrink re-encodes at the new box size on the next sync pass
    /// and the placeholder shows for at most that one frame.
    fn fetch_hero_logo(
        &mut self,
        artwork: &mbv_components::library_panel::HeroArtwork,
        panel_area: ratatui::layout::Rect,
    ) -> Option<String> {
        use mbv_components::library_panel::content::ArtworkSource;
        if !wide_landscape_hero_eligible(artwork, panel_area) {
            return None;
        }
        let Some(ArtworkSource::Emby {
            item_id,
            series_id,
            image_types,
            cache_key,
        }) = artwork.decoration.as_ref()
        else {
            return None;
        };
        let types: Vec<&str> = image_types.iter().map(String::as_str).collect();
        self.fetch_card_image(
            cache_key.clone(),
            item_id.clone(),
            series_id.clone(),
            &types,
        );
        Some(cache_key.clone())
    }

    pub(in crate::app) fn project_hero_image(
        &mut self,
        facts: &mbv_components::library_panel::HeroFacts,
        workspace_present: bool,
        panel_area: ratatui::layout::Rect,
        list_pane_width: Option<u16>,
        overlay_box: Option<(u16, u16)>,
    ) -> mbv_render::components::tv_wide::HeroImageState {
        use mbv_components::library_panel::content::ArtworkSource;
        use mbv_render::components::tv_wide::HeroImageState as State;
        let artwork = &facts.artwork;
        let Some(source) = &artwork.source else {
            return State::None;
        };
        if !self.images.images_enabled() {
            return State::None;
        }
        // The one fetch per key (fetch dedupes on its own reservation set).
        let cache_key: Option<String> = match source {
            ArtworkSource::Emby {
                item_id,
                series_id,
                image_types,
                cache_key,
                ..
            } => {
                let types: Vec<&str> = image_types.iter().map(String::as_str).collect();
                self.fetch_card_image(
                    cache_key.clone(),
                    item_id.clone(),
                    series_id.clone(),
                    &types,
                );
                Some(cache_key.clone())
            }
            ArtworkSource::AudiobookshelfCover {
                library_item_id,
                book,
            } => {
                if *book {
                    self.audiobookshelf_book_cover_key(library_item_id)
                } else {
                    self.audiobookshelf_cover_key(library_item_id)
                }
            }
        };
        let Some(cache_key) = cache_key else {
            return State::None;
        };
        if self.images.card_image_loading.contains(&cache_key) {
            return State::Loading;
        }
        let Some(entry) = self.images.card_image_states.get(&cache_key) else {
            return State::Loading;
        };
        let Some(source_img) = entry.img.as_ref() else {
            // Resolved-empty fetch: no artwork exists; the placeholder is
            // final.
            return State::None;
        };
        let decoded = {
            use image::GenericImageView;
            Some(source_img.dimensions())
        };
        // The fit rule follows the arm on every surface: Landscape artwork
        // is cover-fit (centre-cropped, no margin) for the box that paints
        // it — the Wide Hero pane's or the Library Hero overlay's;
        // Portrait/Square artwork fit-resizes (the whole image, aspect
        // preserved) through the plain protocol.
        let landscape =
            artwork.painted_shape() == mbv_components::library_panel::ArtworkShape::Landscape;
        if !landscape {
            // Drop any stale cover crop so the plain fit protocol rebuilds.
            if let Some(entry) = self.images.card_image_states.get_mut(&cache_key) {
                if entry.cover_box.take().is_some() {
                    entry.protocols.clear();
                }
            }
        } else if mbv_render::wide_hero_fits(panel_area) {
            if let Some(panes) = mbv_render::arrangements::library::wide_library_panes(
                panel_area,
                PANE_PAD_X,
                PANE_PAD_Y,
                list_pane_width,
                true,
            ) {
                let box_cells = mbv_components::library_panel::hero_header::hero_artwork_box(
                    panes.hero_area,
                    facts,
                    workspace_present,
                    self.terminal_height,
                );
                // The optional Logo never delays the base image: it is
                // reserved only here, once a decoded base exists to decorate,
                // and only for a Movie Landscape hero at Wide geometry. A base
                // that resolved empty, and every other presentation, stays
                // undecorated and issues no Logo request.
                let logo_cache_key = self.fetch_hero_logo(artwork, panel_area);
                if !self.ensure_hero_cover_protocol(
                    &cache_key,
                    (box_cells.width, box_cells.height),
                    logo_cache_key.as_deref(),
                ) {
                    return State::Loading;
                }
            }
        } else if let Some(box_cells) = overlay_box {
            // The Library Hero overlay paints the same reserved-box flow; a
            // Landscape hero there is cover-fit for the overlay's own box
            // (its provider-link row stays plain, so no Logo).
            if !self.ensure_hero_cover_protocol(&cache_key, box_cells, None) {
                return State::Loading;
            }
        }
        State::Ready { cache_key, decoded }
    }
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
            self.current_protocol_suffix(),
        );
        if self.images.images_enabled() {
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
    pub(in crate::app) fn audiobookshelf_book_cover_key(
        &mut self,
        library_item_id: &str,
    ) -> Option<String> {
        let setup = self.config.lock().unwrap().audiobookshelf_setup.clone()?;
        let cache_key = audiobookshelf_hero_book_cover_cache_key(
            &setup.server_url,
            library_item_id,
            self.current_protocol_suffix(),
        );
        if self.images.images_enabled() {
            self.fetch_audiobookshelf_image(
                cache_key.clone(),
                setup.server_url.clone(),
                library_item_id.to_string(),
            );
        }
        Some(cache_key)
    }
}

mod fetch;
mod protocol;

#[cfg(test)]
mod tests {
    use crate::app::tests::make_app_stub;
    use mbv_images::{composite_landscape_logo, series_image_cache_key};
    use std::time::Instant;

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
        assert!(!app.images.card_image_loading.contains("recent-nav:P"));
        assert!(!app.images.card_image_states.contains_key("recent-nav:P"));
    }
}
