use super::super::{App, LibEvent, PAGE_SIZE};

pub(in crate::app) mod cache;
use crate::app::palette;
use crate::app::render::components::widgets::RENDER_FILTER;
use crate::app::render::{PANE_PAD_X, PANE_PAD_Y};
use crate::app::state::app_struct::{LevelFillAction, LevelFillState};
use ratatui_image::picker::Picker;
use std::io::Read as IoRead;
use std::time::{Duration, Instant};

pub(in crate::app) const NAV_IMAGE_FETCH_IDLE_DELAY: Duration = Duration::from_millis(150);

fn wide_landscape_hero_eligible(
    artwork: &crate::app::components::library_panel::HeroArtwork,
    panel_area: ratatui::layout::Rect,
) -> bool {
    artwork.shape == crate::app::components::library_panel::ArtworkShape::Landscape
        && crate::app::render::wide_hero_fits(panel_area)
}

pub(in crate::app) fn mem_key(cache_key: &str, suffix: &str) -> String {
    format!("{cache_key}@{suffix}")
}

/// Prefix shared by every Audiobookshelf-sourced cache key, used to filter
/// or clear Audiobookshelf entries from the image caches.
pub(in crate::app) const AUDIOBOOKSHELF_CACHE_KEY_PREFIX: &str = "audiobookshelf:";

/// Cache key for an Audiobookshelf cover under `server`, keyed by the
/// library item's `id` and the active protocol `suffix`.
pub(in crate::app) fn audiobookshelf_cover_cache_key(
    server: &str,
    id: &str,
    suffix: &str,
) -> String {
    format!("{AUDIOBOOKSHELF_CACHE_KEY_PREFIX}{server}:cover:{id}:{suffix}")
}

/// Cache key for an Audiobookshelf book cover. Distinct from the podcast
/// cover key (`:book:`, not `:cover:`) so a book and a podcast sharing an
/// id never share artwork state (book-browsing spec).
pub(in crate::app) fn audiobookshelf_book_cover_cache_key(
    server: &str,
    id: &str,
    suffix: &str,
) -> String {
    format!("{AUDIOBOOKSHELF_CACHE_KEY_PREFIX}{server}:bookcover:{id}:{suffix}")
}

/// Cache key for an Audiobookshelf cover as the Library hero draws it:
/// `{server}:hero:cover:{id}:{suffix}`, the plain cover key under a hero
/// scope.
///
/// The hero re-encodes the entry's protocol from a cover-fit crop of its
/// artwork box (`ensure_hero_cover_protocol`), so one entry can carry either
/// the hero's crop or a plain consumer's `Resize::Scale` — never both. Sharing
/// one key (the queue card paints the same show cover for a playing episode)
/// made each consumer take the other's `ThreadProtocol` on every frame, so the
/// hero fell back to the placeholder block and flashed. Emby's hero and card
/// keys are already distinct for the same reason (`{id}:Backdrop,Primary` vs
/// `{id}:P`).
pub(in crate::app) fn audiobookshelf_hero_cover_cache_key(
    server: &str,
    id: &str,
    suffix: &str,
) -> String {
    format!("{AUDIOBOOKSHELF_CACHE_KEY_PREFIX}{server}:hero:cover:{id}:{suffix}")
}

/// Hero-scoped sibling of [`audiobookshelf_book_cover_cache_key`], with the
/// same crop-vs-plain isolation as [`audiobookshelf_hero_cover_cache_key`].
pub(in crate::app) fn audiobookshelf_hero_book_cover_cache_key(
    server: &str,
    id: &str,
    suffix: &str,
) -> String {
    format!("{AUDIOBOOKSHELF_CACHE_KEY_PREFIX}{server}:hero:bookcover:{id}:{suffix}")
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

/// Cache key under which the bundled queue card placeholder is stored in
/// `card_image_states`. Never touches `card_image_loading`, so it never triggers
/// the transient "Loading…" treatment — it is decoded synchronously from the
/// bundled bytes the first time it's needed and then just sits in the cache.
pub(in crate::app) const QUEUE_CARD_PLACEHOLDER_KEY: &str = "__power_card_placeholder__";

/// Fixed steady-state placeholder shown in the queue card when no
/// queue-card artwork is available.
static QUEUE_CARD_PLACEHOLDER_BYTES: &[u8] =
    include_bytes!("../../../assets/power-card-placeholder.webp");

/// One `card_image_states` cache entry: the decoded source image (retained so
/// it can be re-encoded with a different protocol picker without refetching —
/// e.g. the halfblock picker used while a backdrop is dimmed, #451) plus one
/// encoded `ThreadProtocol` per protocol suffix (e.g. `sixel`, `halfblock`).
/// The active suffix's protocol is built on fetch; the others are created
/// lazily on first render under that suffix.
pub(in crate::app) struct CachedImage {
    /// `None` marks a fetch that resolved without artwork.
    pub img: Option<image::DynamicImage>,
    pub protocols: std::collections::HashMap<&'static str, ratatui_image::thread::ThreadProtocol>,
    /// The hero artwork box (cells) the current protocols were built from
    /// (task 5.10, design D5's cover fit): the panel hero projection
    /// `resize_to_fill`s the source to the box's pixel size before encoding,
    /// keyed by the box, so a box change rebuilds the protocol at the new
    /// size. `None` for every non-hero cache entry (plain `Resize::Scale`).
    pub cover_box: Option<(u16, u16)>,
    /// Cache identity of the Logo applied to the current hero protocols.
    /// `None` means the base-only protocol is valid (including pending/failed
    /// Logo fetches).
    pub applied_logo_key: Option<String>,
}

impl CachedImage {
    /// An entry for a fetch that resolved with no image.
    #[cfg(test)]
    pub(in crate::app) fn empty() -> Self {
        Self {
            img: None,
            protocols: std::collections::HashMap::new(),
            cover_box: None,
            applied_logo_key: None,
        }
    }
}

/// A pending card-image fetch, queued when the in-flight limit is reached.
pub(in crate::app) struct ImageFetchReq {
    pub cache_key: String,
    pub item_id: String,
    pub series_id: String,
    pub types: Vec<String>,
    pub source: ImageSource,
}

#[derive(Debug, Clone)]
pub(in crate::app) enum ImageSource {
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

/// Logo decoration geometry, as percentages of the landscape hero bitmap
/// (design D3): contain-fit within 60% width / 20% height, then placed at a 5%
/// top-left inset clamped so the resized Logo stays inside the artwork.
const LOGO_MAX_WIDTH_PERCENT: u32 = 60;
const LOGO_MAX_HEIGHT_PERCENT: u32 = 20;
const LOGO_INSET_PERCENT: f32 = 5.0;

fn rounded_dimension(value: f32) -> u32 {
    value.to_string().parse().unwrap_or(u32::MAX)
}

/// Decorate an already cover-fitted landscape bitmap with a transparent Logo.
/// The Logo is contain-fitted into the prescribed bounds, then source-over
/// composited at the rounded, clamped inset.
pub(in crate::app) fn composite_landscape_logo(
    base: &image::DynamicImage,
    logo: &image::DynamicImage,
) -> image::DynamicImage {
    use image::GenericImageView;
    let mut base = base.to_rgba8();
    let (base_w, base_h) = base.dimensions();
    let max_w = ((base_w * LOGO_MAX_WIDTH_PERCENT) / 100).max(1);
    let max_h = ((base_h * LOGO_MAX_HEIGHT_PERCENT) / 100).max(1);
    let logo = logo.resize(max_w, max_h, image::imageops::FilterType::Lanczos3);
    let (logo_w, logo_h) = logo.dimensions();
    let inset_percent = LOGO_INSET_PERCENT / 100.0;
    #[expect(
        clippy::cast_precision_loss,
        reason = "image scale factor through f32; no lossless integer-path conversion exists (approved, issue #804)"
    )]
    let inset_x = rounded_dimension((base_w as f32 * inset_percent).round())
        .min(base_w.saturating_sub(logo_w));
    #[expect(
        clippy::cast_precision_loss,
        reason = "image scale factor through f32; no lossless integer-path conversion exists (approved, issue #804)"
    )]
    let inset_y = rounded_dimension((base_h as f32 * inset_percent).round())
        .min(base_h.saturating_sub(logo_h));
    image::imageops::overlay(
        &mut base,
        &logo.to_rgba8(),
        i64::from(inset_x),
        i64::from(inset_y),
    );
    image::DynamicImage::ImageRgba8(base)
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
        artwork: &crate::app::components::library_panel::HeroArtwork,
        panel_area: ratatui::layout::Rect,
    ) -> Option<String> {
        use crate::app::components::library_panel::content::ArtworkSource;
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
        facts: &crate::app::components::library_panel::HeroFacts,
        workspace_present: bool,
        panel_area: ratatui::layout::Rect,
        list_pane_width: Option<u16>,
        overlay_box: Option<(u16, u16)>,
    ) -> crate::app::components::library_panel::HeroImageState {
        use crate::app::components::library_panel::content::ArtworkSource;
        use crate::app::components::library_panel::content::HeroImageState as State;
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
        let landscape = artwork.painted_shape()
            == crate::app::components::library_panel::ArtworkShape::Landscape;
        if !landscape {
            // Drop any stale cover crop so the plain fit protocol rebuilds.
            if let Some(entry) = self.images.card_image_states.get_mut(&cache_key) {
                if entry.cover_box.take().is_some() {
                    entry.protocols.clear();
                }
            }
        } else if crate::app::render::wide_hero_fits(panel_area) {
            if let Some(panes) = crate::app::render::arrangements::library::wide_library_panes(
                panel_area,
                PANE_PAD_X,
                PANE_PAD_Y,
                list_pane_width,
                true,
            ) {
                let box_cells =
                    crate::app::components::library_panel::hero_header::hero_artwork_box(
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
                let protocol_suffix = self.current_protocol_suffix();
                if !self.images.ensure_hero_cover_protocol(
                    &cache_key,
                    (box_cells.width, box_cells.height),
                    logo_cache_key.as_deref(),
                    protocol_suffix,
                ) {
                    return State::Loading;
                }
            }
        } else if let Some(box_cells) = overlay_box {
            // The Library Hero overlay paints the same reserved-box flow; a
            // Landscape hero there is cover-fit for the overlay's own box
            // (its provider-link row stays plain, so no Logo).
            let protocol_suffix = self.current_protocol_suffix();
            if !self
                .images
                .ensure_hero_cover_protocol(&cache_key, box_cells, None, protocol_suffix)
            {
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
    use super::{composite_landscape_logo, series_image_cache_key};
    use crate::app::tests::make_app_stub;
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
