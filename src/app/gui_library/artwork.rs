//! The GUI Library Hero's artwork (add-gui-library-panel design D5): the
//! picked item's art from the image cache, as the server sent it, and the
//! neighbour warm that fills the cache around the pick. The fetches
//! are issued here, as the TUI's hero projection issues its own, so the window
//! content never requests an image itself.

use mbv_components::library_panel::content::{
    ArtworkShape, ArtworkSource, painted_shape_for_pixels,
};
use mbv_emby_model::EmbyItem;
use mbv_render::components::widgets::MUSIC_ALBUM_IMAGE_TYPES;

use crate::app::App;
use mbv_images::SourceImage;

/// The neighbour warm window's bounds (the TUI's grouped-Music rule,
/// `openspec/specs/music-library-hero/spec.md`): one entry behind the picked
/// one and three ahead of it, never the picked entry itself.
const HERO_WARM_BEHIND: usize = 1;
const HERO_WARM_AHEAD: usize = 3;

/// The Music Hero's artwork source for `album_id`: the shared album chain
/// under the album's `{id}:P` key, as the shared hero policy's album arm
/// declares it (`music_album_artwork`,
/// `crates/mbv-components/src/library_panel/hero.rs`). One derivation for the
/// Hero's own fetch and the neighbour warm, so a warmed entry is the entry
/// the Hero reads.
pub(super) fn music_album_source(album_id: &str) -> ArtworkSource {
    ArtworkSource::Emby {
        item_id: album_id.to_owned(),
        series_id: String::new(),
        image_types: MUSIC_ALBUM_IMAGE_TYPES
            .iter()
            .map(ToString::to_string)
            .collect(),
        cache_key: mbv_images::emby_card_cache_key(album_id, ""),
    }
}

/// The Emby-side fetch parts of a Hero source — `(cache_key, item_id,
/// series_id, image_types)` — or `None` for a source this Client's Emby fetch
/// path does not cover, such as a Service-scoped Audiobookshelf cover. One
/// rule for every caller that turns a Hero's declared source into a fetch.
fn emby_fetch_parts(source: ArtworkSource) -> Option<(String, String, String, Vec<String>)> {
    match source {
        ArtworkSource::Emby {
            item_id,
            series_id,
            image_types,
            cache_key,
        } => Some((cache_key, item_id, series_id, image_types)),
        ArtworkSource::AudiobookshelfCover { .. } => None,
    }
}

/// What the Library Hero paints as its square artwork (design D5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuiLibraryArtwork {
    /// The album's cached artwork under `key`, as the server sent it. The
    /// window sizes and crops it: the TUI crops at its artwork box, and the
    /// window's `object-fit: cover` crops at the Hero's.
    Image { key: String, source: SourceImage },
    /// The album has no cached art and no fetch in flight.
    Placeholder,
    /// The album's art fetch is in flight or awaits its retry.
    Loading,
}

impl App {
    /// The image cache's source under `cache_key`, as the server sent it
    /// (`gui-change-driven-sections` design D7): the read-only read the GUI
    /// core answers `mbv-art` card requests with. It starts no fetch — the
    /// projections that own the fetches (`gui_library_artwork`,
    /// `queue_card_artwork`) do that — so an art request answers only what the
    /// cache already holds. The source's bytes clone once per request.
    #[must_use]
    pub fn gui_art_source(&self, cache_key: &str) -> Option<SourceImage> {
        self.images
            .image(cache_key)
            .and_then(|entry| entry.source.clone())
    }

    /// Pre-warm the Hero artwork of the entries neighbouring the picked one in
    /// the painted order: the warm window is the TUI's grouped-Music rule
    /// (`App::prefetch_neighbour_album_art`) over this Client's own
    /// projections — one entry behind, three ahead, never the picked entry,
    /// whose art the Hero's own fetch covers. `ids` is the visible order's
    /// identity per entry, `picked` is the picked entry's index in it, and
    /// `source_of` derives an entry's Hero source from the same producer the
    /// Hero's own fetch reads. Every fetch runs through the shared
    /// navigation-idle gate, so fast navigation warms nothing, and only an
    /// Emby-chained source warms: a Service-scoped Audiobookshelf cover has no
    /// key this path fetches.
    pub(in crate::app::gui_library) fn warm_gui_hero_neighbours(
        &mut self,
        ids: &[&str],
        picked: usize,
        source_of: impl Fn(&Self, &str) -> Option<ArtworkSource>,
    ) {
        if !self.images.images_enabled() || picked >= ids.len() {
            return;
        }
        let start = picked.saturating_sub(HERO_WARM_BEHIND);
        let end = (picked + HERO_WARM_AHEAD + 1).min(ids.len());
        for (offset, id) in ids[start..end].iter().enumerate() {
            if start + offset == picked {
                continue;
            }
            let Some((cache_key, item_id, series_id, image_types)) =
                source_of(self, id).and_then(emby_fetch_parts)
            else {
                continue;
            };
            let types: Vec<&str> = image_types.iter().map(String::as_str).collect();
            self.fetch_list_card_image_when_idle(cache_key, item_id, series_id, &types);
        }
    }

    /// Project the Hero's artwork for the album `album_id` (design D5): the
    /// `AudioChild` album chain under the album's `{id}:P` key, fetched here
    /// and read from the image cache. A Client that paints no terminal
    /// protocols reads this state and hands the server's bytes to its own
    /// presentation, as [`App::queue_card_artwork`] does for the visual slot.
    #[must_use]
    pub fn gui_library_artwork(&mut self, album_id: &str) -> GuiLibraryArtwork {
        if album_id.is_empty() || !self.images.images_enabled() {
            return GuiLibraryArtwork::Placeholder;
        }
        let Some((cache_key, item_id, series_id, image_types)) =
            emby_fetch_parts(music_album_source(album_id))
        else {
            return GuiLibraryArtwork::Placeholder;
        };
        let types: Vec<&str> = image_types.iter().map(String::as_str).collect();
        self.fetch_card_image(cache_key.clone(), item_id, series_id, &types);
        self.resolve_gui_art(&cache_key)
    }

    /// One Home row's card art (add-gui-library-panel design D12, the
    /// USER-APPROVED GUI Home design): the queue card's chain and cache
    /// key for `item` — the now-playing card's conventions, so a Home row
    /// whose item plays shares its cache entry — fetched here and read from
    /// the image cache. A fetch in flight paints the
    /// loading fill, and a fetch that resolved empty paints the
    /// placeholder — the queue card's resolution.
    pub(in crate::app::gui_library) fn gui_home_row_artwork(
        &mut self,
        item: &EmbyItem,
    ) -> GuiLibraryArtwork {
        let cache_key = crate::app::state::projection::card::card_cache_key(item);
        let types = crate::app::state::projection::card::card_image_types(item);
        self.fetch_card_image(
            cache_key.clone(),
            item.id.clone(),
            item.series_id.clone(),
            types,
        );
        self.resolve_gui_art(&cache_key)
    }

    /// The Hero's art box: the declared policy arm, re-armed by the fetched
    /// image's own pixels once the cache holds them — the rule the TUI's
    /// [`mbv_components::library_panel::HeroArtwork::painted_shape`] applies
    /// (design D5). A `Primary` chain that serves a landscape still (an
    /// episode), or a `Thumb` chain that falls through to the portrait
    /// poster, therefore paints the box its image needs instead of the box
    /// its declaration named: without this the GUI painted a 16:9 still
    /// inside a 3:4 box and a poster inside a 16:9 band, where the TUI
    /// painted neither.
    pub(in crate::app::gui_library) fn gui_hero_shape(
        &self,
        declared: ArtworkShape,
        art: Option<&GuiLibraryArtwork>,
    ) -> ArtworkShape {
        let Some(GuiLibraryArtwork::Image { key, .. }) = art else {
            return declared;
        };
        let Some((width, height)) = self
            .images
            .image(key)
            .and_then(mbv_images::CachedImage::dimensions)
        else {
            return declared;
        };
        painted_shape_for_pixels(width, height)
    }

    /// The Hero's artwork for the artwork policy's declared Emby source
    /// (add-gui-library-panel design D12, D15): the one dispatch every
    /// destination's Emby-backed Hero runs, so a source the design excludes
    /// paints no art in every Hero.
    pub(in crate::app::gui_library) fn gui_emby_hero_art(
        &mut self,
        source: Option<&ArtworkSource>,
    ) -> Option<GuiLibraryArtwork> {
        match source {
            Some(ArtworkSource::Emby {
                item_id,
                series_id,
                image_types,
                cache_key,
            }) => Some(self.gui_home_hero_artwork(cache_key, item_id, series_id, image_types)),
            Some(ArtworkSource::AudiobookshelfCover { .. }) | None => None,
        }
    }

    /// The Home Hero's artwork under the artwork policy's declared source
    /// (add-gui-library-panel design D12): the policy chain and cache key
    /// the TUI hero fetches, fetched here and read from the image cache.
    pub(in crate::app::gui_library) fn gui_home_hero_artwork(
        &mut self,
        cache_key: &str,
        item_id: &str,
        series_id: &str,
        image_types: &[String],
    ) -> GuiLibraryArtwork {
        let types: Vec<&str> = image_types.iter().map(String::as_str).collect();
        self.fetch_card_image(
            cache_key.to_owned(),
            item_id.to_owned(),
            series_id.to_owned(),
            &types,
        );
        self.resolve_gui_art(cache_key)
    }

    /// The cached art under `cache_key`, as the server sent it, or the empty
    /// fetch's placeholder and the in-flight fetch's loading state — the
    /// queue card's resolution. One resolution for every Library art: the
    /// Hero's box and a row's box differ only in how the window sizes the
    /// image, never in the bytes it holds.
    pub(in crate::app::gui_library) fn resolve_gui_art(
        &self,
        cache_key: &str,
    ) -> GuiLibraryArtwork {
        let source = self
            .images
            .image(cache_key)
            .and_then(|entry| entry.source.clone());
        match source {
            Some(source) => GuiLibraryArtwork::Image {
                key: cache_key.to_owned(),
                source,
            },
            None if self.images.is_loading(cache_key)
                || self.images.is_fetch_interrupted(cache_key) =>
            {
                GuiLibraryArtwork::Loading
            }
            None => GuiLibraryArtwork::Placeholder,
        }
    }
}
