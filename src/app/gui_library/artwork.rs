//! The GUI Library Hero's artwork (add-gui-library-panel design D5): the
//! picked album's art from the image cache, display-encoded for the window.
//! The fetch is issued here, as the TUI's hero projection issues its own, so
//! the window content never requests an image itself.

use mbv_emby_model::EmbyItem;
use mbv_render::components::widgets::MUSIC_ALBUM_IMAGE_TYPES;

use super::super::queue_card_artwork::encode_for_display;
use crate::app::App;

/// What the Library Hero paints as its square artwork (design D5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuiLibraryArtwork {
    /// The album's cached artwork under `key`, display-encoded.
    Image { key: String, jpeg: Vec<u8> },
    /// The album has no cached art and no fetch in flight.
    Placeholder,
    /// The album's art fetch is in flight or awaits its retry.
    Loading,
}

impl App {
    /// Project the Hero's artwork for the album `album_id` (design D5): the
    /// `AudioChild` album chain under the album's `{id}:P` key, fetched here
    /// and encoded from the image cache once per key. A Client that paints no
    /// terminal protocols reads this state and encodes it for its own
    /// presentation, as [`App::queue_card_artwork`] does for the visual slot.
    #[must_use]
    pub fn gui_library_artwork(&mut self, album_id: &str) -> GuiLibraryArtwork {
        let cache_key = mbv_images::emby_card_cache_key(album_id, "");
        if album_id.is_empty() || !self.images.images_enabled() {
            return GuiLibraryArtwork::Placeholder;
        }
        self.fetch_card_image(
            cache_key.clone(),
            album_id.to_string(),
            String::new(),
            MUSIC_ALBUM_IMAGE_TYPES,
        );
        if self
            .images
            .image(&cache_key)
            .is_none_or(|entry| entry.img.is_none())
        {
            // No decoded art yet: a fetch in flight or awaiting its retry
            // paints the loading fill, and a fetch that resolved empty paints
            // the placeholder — the queue card's resolution.
            if self.images.is_loading(&cache_key) || self.images.is_fetch_interrupted(&cache_key) {
                return GuiLibraryArtwork::Loading;
            }
            return GuiLibraryArtwork::Placeholder;
        }
        self.encode_gui_library_art(&cache_key)
    }

    /// The display-encoded artwork for `cache_key`'s cached image, encoded
    /// once per key and memoized (`gui_library_art_encoded`).
    fn encode_gui_library_art(&mut self, cache_key: &str) -> GuiLibraryArtwork {
        if self
            .gui_library_art_encoded
            .as_ref()
            .is_none_or(|(key, _)| key != cache_key)
        {
            let Some(entry) = self
                .images
                .image(cache_key)
                .and_then(|entry| entry.img.clone())
            else {
                return GuiLibraryArtwork::Placeholder;
            };
            let encoded = encode_for_display(&entry);
            self.gui_library_art_encoded = Some((cache_key.to_owned(), encoded));
        }
        let Some((key, encoded)) = self
            .gui_library_art_encoded
            .as_ref()
            .filter(|(key, _)| key == cache_key)
        else {
            return GuiLibraryArtwork::Placeholder;
        };
        GuiLibraryArtwork::Image {
            key: key.clone(),
            jpeg: encoded.clone(),
        }
    }

    /// One Home row's card art (add-gui-library-panel design D12, the
    /// USER-APPROVED GUI Home design): the queue card's chain and cache
    /// key for `item` — the now-playing card's conventions, so a Home row
    /// whose item plays shares its cache entry — fetched here and encoded
    /// from the image cache once per key. A fetch in flight paints the
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
        self.resolve_gui_home_art(&cache_key)
    }

    /// The Home Hero's artwork under the artwork policy's declared source
    /// (add-gui-library-panel design D12): the policy chain and cache key
    /// the TUI hero fetches, fetched here and encoded from the image cache
    /// once per key. The same resolution the row art runs.
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
        self.resolve_gui_home_art(cache_key)
    }

    /// The cached art under `cache_key`, display-encoded once per key
    /// (`gui_home_art_encoded`), or the empty fetch's placeholder and the
    /// in-flight fetch's loading state — the queue card's resolution.
    fn resolve_gui_home_art(&mut self, cache_key: &str) -> GuiLibraryArtwork {
        if self
            .images
            .image(cache_key)
            .is_none_or(|entry| entry.img.is_none())
        {
            if self.images.is_loading(cache_key) || self.images.is_fetch_interrupted(cache_key) {
                return GuiLibraryArtwork::Loading;
            }
            return GuiLibraryArtwork::Placeholder;
        }
        if !self.gui_home_art_encoded.contains_key(cache_key) {
            let Some(entry) = self
                .images
                .image(cache_key)
                .and_then(|entry| entry.img.clone())
            else {
                return GuiLibraryArtwork::Placeholder;
            };
            let encoded = encode_for_display(&entry);
            self.gui_home_art_encoded
                .insert(cache_key.to_owned(), encoded);
        }
        self.gui_home_art_encoded
            .get(cache_key)
            .map_or(GuiLibraryArtwork::Placeholder, |jpeg| {
                GuiLibraryArtwork::Image {
                    key: cache_key.to_owned(),
                    jpeg: jpeg.clone(),
                }
            })
    }
}
