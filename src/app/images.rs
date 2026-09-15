use super::{App, LibEvent, PAGE_SIZE};
use crate::app::palette;
use crate::app::render::components::widgets::RENDER_FILTER;
use crate::app::render::{PANE_PAD_X, PANE_PAD_Y};
use ratatui_image::picker::Picker;
use std::io::Read as IoRead;
use std::time::{Duration, Instant};

pub(super) const NAV_IMAGE_FETCH_IDLE_DELAY: Duration = Duration::from_millis(150);

pub(super) fn mem_key(cache_key: &str, suffix: &str) -> String {
    format!("{cache_key}@{suffix}")
}

/// Prefix shared by every Audiobookshelf-sourced cache key, used to filter
/// or clear Audiobookshelf entries from the image caches.
pub(super) const AUDIOBOOKSHELF_CACHE_KEY_PREFIX: &str = "audiobookshelf:";

/// Cache key for an Audiobookshelf cover under `server`, keyed by the
/// library item's `id` and the active protocol `suffix`.
pub(super) fn audiobookshelf_cover_cache_key(server: &str, id: &str, suffix: &str) -> String {
    format!("{AUDIOBOOKSHELF_CACHE_KEY_PREFIX}{server}:cover:{id}:{suffix}")
}

/// Cache key for an Audiobookshelf book cover. Distinct from the podcast
/// cover key (`:book:`, not `:cover:`) so a book and a podcast sharing an
/// id never share artwork state (book-browsing spec).
pub(super) fn audiobookshelf_book_cover_cache_key(server: &str, id: &str, suffix: &str) -> String {
    format!("{AUDIOBOOKSHELF_CACHE_KEY_PREFIX}{server}:bookcover:{id}:{suffix}")
}

/// The infix opening a Series artwork key: `{id}{SERIES_IMAGE_CACHE_KEY_INFIX}{types}`.
/// No other cache-key namespace uses it, which is what lets the image-completion
/// gate recognise the whole Series family from the key alone.
pub(in crate::app) const SERIES_IMAGE_CACHE_KEY_INFIX: &str = ":ser:";

/// Cache key for Series artwork under the `{id}:ser:{types}` scheme.
/// Shared by the panel's Emby artwork projection and shell-side
/// prefetch/loading lookups so they cannot format the key differently and
/// silently miss each other's cache entries. Formats only;
/// chain ownership stays with the callers.
pub(in crate::app) fn series_image_cache_key(item_id: &str, image_types: &[&str]) -> String {
    format!(
        "{item_id}{SERIES_IMAGE_CACHE_KEY_INFIX}{}",
        image_types.join(",")
    )
}

const MAX_IMAGE_FETCHES: usize = 6;
const MAX_ALBUM_ARTIST_FETCHES: usize = 6;

/// Cache key under which the bundled queue card placeholder is stored in
/// `card_image_states`. Never touches `card_image_loading`, so it never triggers
/// the transient "Loading…" treatment — it is decoded synchronously from the
/// bundled bytes the first time it's needed and then just sits in the cache.
pub(super) const QUEUE_CARD_PLACEHOLDER_KEY: &str = "__power_card_placeholder__";

/// Fixed steady-state placeholder shown in the queue card when no
/// queue-card artwork is available.
static QUEUE_CARD_PLACEHOLDER_BYTES: &[u8] =
    include_bytes!("../../assets/power-card-placeholder.webp");

/// One `card_image_states` cache entry: the decoded source image (retained so
/// it can be re-encoded with a different protocol picker without refetching —
/// e.g. the halfblock picker used while a backdrop is dimmed, #451) plus one
/// encoded `ThreadProtocol` per protocol suffix (e.g. `sixel`, `halfblock`).
/// The active suffix's protocol is built on fetch; the others are created
/// lazily on first render under that suffix.
pub(super) struct CachedImage {
    /// `None` marks a fetch that resolved without artwork.
    pub img: Option<image::DynamicImage>,
    pub protocols: std::collections::HashMap<&'static str, ratatui_image::thread::ThreadProtocol>,
    /// The hero artwork box (cells) the current protocols were built from
    /// (task 5.10, design D5's cover fit): the panel hero projection
    /// `resize_to_fill`s the source to the box's pixel size before encoding,
    /// keyed by the box, so a box change rebuilds the protocol at the new
    /// size. `None` for every non-hero cache entry (plain `Resize::Scale`).
    pub cover_box: Option<(u16, u16)>,
}

impl CachedImage {
    /// An entry for a fetch that resolved with no image.
    #[cfg(test)]
    pub(super) fn empty() -> Self {
        Self {
            img: None,
            protocols: std::collections::HashMap::new(),
            cover_box: None,
        }
    }
}

/// A pending card-image fetch, queued when the in-flight limit is reached.
pub(super) struct ImageFetchReq {
    pub cache_key: String,
    pub item_id: String,
    pub series_id: String,
    pub types: Vec<String>,
    pub source: ImageSource,
}

#[derive(Debug, Clone)]
pub(super) enum ImageSource {
    Emby,
    Audiobookshelf { server_url: String, api_key: String },
}

/// Cover fit for a hero artwork box (design D5, task 5.4): the decoded
/// source image is scaled to cover the box's pixel size and centre-cropped,
/// so the box shows no margin; painting then uses the existing
/// `Resize::Scale`. The box's size comes from the panel's paint-free
/// `hero_artwork_box` (task 5.5); the shell projection that calls this is
/// keyed by the box (task 5.10, `App::ensure_hero_cover_protocol`).
pub(in crate::app) fn cover_fill_hero_box(
    source: &image::DynamicImage,
    box_w: u32,
    box_h: u32,
) -> image::DynamicImage {
    let (w, h) = (box_w.max(1), box_h.max(1));
    source.resize_to_fill(w, h, image::imageops::FilterType::Lanczos3)
}

impl App {
    /// One panel hero's projected image state (task 5.10, design D9): the
    /// projection — never the painter — issues every fetch, then projects the
    /// state painting reads. `panel_area` is the Library panel's `RootFrame`
    /// content area; the Wide header's cover-fit box is keyed by the box
    /// `hero_artwork_box` derives from it, so a resize, split drag, or
    /// Workspace shrink re-encodes at the new box size on the next sync pass
    /// and the placeholder shows for at most that one frame.
    pub(in crate::app) fn project_hero_image(
        &mut self,
        facts: &crate::app::components::library_panel::HeroFacts,
        workspace_present: bool,
        panel_area: ratatui::layout::Rect,
        list_pane_width: Option<u16>,
    ) -> crate::app::components::library_panel::HeroImageState {
        use crate::app::components::library_panel::content::ArtworkSource;
        use crate::app::components::library_panel::content::HeroImageState as State;
        let artwork = &facts.artwork;
        let Some(source) = &artwork.source else {
            return State::None;
        };
        if !self.images_enabled() {
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
                let types: Vec<&str> = image_types.iter().map(|s| s.as_str()).collect();
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
            } => match *book {
                true => self.audiobookshelf_book_cover_key(library_item_id),
                false => self.audiobookshelf_cover_key(library_item_id),
            },
        };
        let Some(cache_key) = cache_key else {
            return State::None;
        };
        if self.card_image_loading.contains(&cache_key) {
            return State::Loading;
        }
        let Some(entry) = self.card_image_states.get(&cache_key) else {
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
        // The Wide header is cover-fit: re-encode keyed by the box size.
        if crate::app::render::wide_hero_fits(panel_area) {
            if let Some(panes) = crate::app::render::arrangements::library::wide_library_panes(
                panel_area,
                PANE_PAD_X,
                PANE_PAD_Y,
                list_pane_width,
            ) {
                let box_cells =
                    crate::app::components::library_panel::hero_header::hero_artwork_box(
                        panes.hero_area,
                        facts,
                        workspace_present,
                    );
                if !self.ensure_hero_cover_protocol(&cache_key, (box_cells.width, box_cells.height))
                {
                    return State::Loading;
                }
            }
        }
        State::Ready { cache_key, decoded }
    }
}

impl App {
    /// Triggers the Audiobookshelf cover fetch for `library_item_id` and
    /// returns its image cache key, or `None` with no server configured.
    /// The hero projection (task 5.10) and the un-migrated painters'
    /// `paint_home_image` Audiobookshelf arm both resolve the key here, so
    /// the fetch dedupes on the one reservation.
    pub(in crate::app) fn audiobookshelf_cover_key(
        &mut self,
        library_item_id: &str,
    ) -> Option<String> {
        let setup = self.config.lock().unwrap().audiobookshelf_setup.clone()?;
        if self.images_enabled() {
            self.fetch_audiobookshelf_cover(setup.server_url.clone(), library_item_id.to_string());
        }
        Some(audiobookshelf_cover_cache_key(
            &setup.server_url,
            library_item_id,
            self.current_protocol_suffix(),
        ))
    }

    /// Triggers the Audiobookshelf book-cover fetch for `library_item_id` and
    /// returns its isolated image cache key, or `None` with no server
    /// configured. The book-browsing spec requires book artwork to remain
    /// isolated from podcast artwork (line 124).
    pub(in crate::app) fn audiobookshelf_book_cover_key(
        &mut self,
        library_item_id: &str,
    ) -> Option<String> {
        let setup = self.config.lock().unwrap().audiobookshelf_setup.clone()?;
        if self.images_enabled() {
            self.fetch_audiobookshelf_book_cover(
                setup.server_url.clone(),
                library_item_id.to_string(),
            );
        }
        Some(audiobookshelf_book_cover_cache_key(
            &setup.server_url,
            library_item_id,
            self.current_protocol_suffix(),
        ))
    }
}

include!("image_fetch.rs");
include!("image_protocol.rs");

#[cfg(test)]
mod tests {
    use super::{cover_fill_hero_box, series_image_cache_key, NAV_IMAGE_FETCH_IDLE_DELAY};
    use crate::app::tests::make_app_stub;
    use std::time::{Duration, Instant};

    /// A 4:3 source filled into a 16:9 box is cropped top and bottom (design
    /// D5: the artwork fills its box; the excess is cropped, centred). The
    /// source has white bands in its top and bottom eighths so a squashed or
    /// letterboxed fit would show white at the box edges; only the cover
    /// crop removes them.
    #[test]
    fn cover_fill_crops_a_4_3_source_into_a_16_9_box() {
        let (w, h) = (800u32, 600u32);
        let mut img = image::DynamicImage::new_rgb8(w, h);
        for (_x, y, pixel) in img.as_mut_rgb8().unwrap().enumerate_pixels_mut() {
            let white = y < h / 8 || y >= h * 7 / 8;
            *pixel = if white {
                image::Rgb([255, 255, 255])
            } else {
                image::Rgb([0, 0, 0])
            };
        }
        let filled = cover_fill_hero_box(&img, 160, 90);
        use image::GenericImageView;
        assert_eq!(filled.dimensions(), (160, 90));
        let brightness = |pixel: &image::Rgb<u8>| {
            let [r, g, b] = pixel.0;
            (u16::from(r) + u16::from(g) + u16::from(b)) / 3
        };
        let rgb = filled.as_rgb8().unwrap();
        assert!(brightness(rgb.get_pixel(80, 0)) < 64, "top band cropped");
        assert!(
            brightness(rgb.get_pixel(80, 89)) < 64,
            "bottom band cropped"
        );
    }

    #[test]
    fn wide_hero_projection_encodes_the_capped_box_height() {
        use crate::app::components::library_panel::content::HeroImageState as State;
        use crate::app::components::library_panel::{ArtworkShape, HeroArtwork, HeroFacts};

        let facts = HeroFacts {
            title: "Dune".into(),
            meta_rows: vec!["2021".into()],
            links: Vec::new(),
            artwork: HeroArtwork {
                shape: ArtworkShape::Landscape,
                source: None,
                image: State::None,
            },
        };
        let area = ratatui::layout::Rect::new(0, 0, 113, 60);
        let box_cells = crate::app::components::library_panel::hero_header::hero_artwork_box(
            area, &facts, false,
        );
        assert_eq!(box_cells.height, 25);

        let source = image::DynamicImage::new_rgb8(80, 60);
        let encoded = cover_fill_hero_box(&source, 160, u32::from(box_cells.height) * 20);
        use image::GenericImageView;
        assert_eq!(encoded.dimensions(), (160, 500));
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
        assert!(!app.card_image_loading.contains("recent-nav:P"));
        assert!(!app.card_image_states.contains_key("recent-nav:P"));
    }

    #[test]
    fn idle_navigation_allows_list_card_image_fetch() {
        let mut app = make_app_stub();
        app.last_nav_at = Instant::now() - NAV_IMAGE_FETCH_IDLE_DELAY - Duration::from_millis(1);
        app.fetch_list_card_image_when_idle(
            "idle-nav:P".into(),
            "idle-nav".into(),
            String::new(),
            &["Primary"],
        );
        assert!(
            app.card_image_loading.contains("idle-nav:P")
                || app.card_image_states.contains_key("idle-nav:P")
        );
    }
}
