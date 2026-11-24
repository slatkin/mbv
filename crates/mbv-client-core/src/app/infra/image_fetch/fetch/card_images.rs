use super::{
    App, ImageFetchReq, ImageSource, Instant, MAX_IMAGE_FETCHES, NAV_IDLE_DELAY,
    audiobookshelf_book_cover_cache_key, audiobookshelf_cover_cache_key,
};

impl App {
    pub fn fetch_card_image(
        &mut self,
        cache_key: String,
        item_id: String,
        series_id: String,
        types: &[&str],
    ) {
        self.queue_card_image_fetch(cache_key, item_id, series_id, types);
    }

    fn queue_card_image_fetch(
        &mut self,
        cache_key: String,
        item_id: String,
        series_id: String,
        types: &[&str],
    ) {
        let req = ImageFetchReq {
            cache_key,
            item_id,
            series_id,
            types: types.iter().map(ToString::to_string).collect(),
            source: ImageSource::Emby,
            rendition: mbv_images::ImageRendition::Hero,
        };
        if let mbv_images::cache::FetchReservation::Start(req) =
            self.images.reserve_card_image_fetch(req, MAX_IMAGE_FETCHES)
        {
            self.spawn_image_fetch(req);
        }
    }

    /// Triggers the plain (uncropped) Audiobookshelf cover fetch for
    /// `item_id` — the queue card's entry. The Library hero uses the
    /// hero-scoped key instead (`App::audiobookshelf_cover_key`), because it
    /// re-encodes its entry from a cover-fit crop.
    pub(in crate::app) fn fetch_audiobookshelf_cover(
        &mut self,
        server_url: String,
        item_id: String,
    ) {
        let cache_key =
            audiobookshelf_cover_cache_key(&server_url, &item_id, self.image_protocol_suffix());
        self.fetch_audiobookshelf_image(cache_key, server_url, item_id);
    }

    /// Book-shaped sibling of `fetch_audiobookshelf_cover` using the isolated
    /// `:bookcover:` cache key.
    pub(in crate::app) fn fetch_audiobookshelf_book_cover(
        &mut self,
        server_url: String,
        item_id: String,
    ) {
        let cache_key = audiobookshelf_book_cover_cache_key(
            &server_url,
            &item_id,
            self.image_protocol_suffix(),
        );
        self.fetch_audiobookshelf_image(cache_key, server_url, item_id);
    }

    pub(in crate::app) fn fetch_audiobookshelf_image(
        &mut self,
        cache_key: String,
        server_url: String,
        item_id: String,
    ) {
        if !self.images.protocol_enabled()
            || self.images.is_loading(&cache_key)
            || self.images.is_cached(&cache_key)
            || self.images.is_fetch_retry_waiting(&cache_key)
        {
            return;
        }
        let Some(api_key) = mbv_config::load_service_secret(mbv_queue::ServiceKind::Audiobookshelf)
        else {
            return;
        };
        let req = ImageFetchReq {
            cache_key,
            item_id,
            series_id: String::new(),
            types: Vec::new(),
            source: ImageSource::Audiobookshelf {
                server_url,
                api_key,
            },
            rendition: mbv_images::ImageRendition::Hero,
        };
        if let mbv_images::cache::FetchReservation::Start(req) =
            self.images.reserve_fetch(req, MAX_IMAGE_FETCHES)
        {
            self.spawn_image_fetch(req);
        }
    }

    pub fn navigation_idle(&self) -> bool {
        self.last_nav_at.elapsed() >= NAV_IDLE_DELAY
    }

    pub fn stamp_navigation(&mut self) {
        self.last_nav_at = Instant::now();
    }

    pub fn fetch_list_card_image_when_idle(
        &mut self,
        cache_key: String,
        item_id: String,
        series_id: String,
        types: &[&str],
    ) {
        if !self.navigation_idle() {
            return;
        }
        self.fetch_card_image(cache_key, item_id, series_id, types);
    }

    /// Queue the in-view row fetches (design D7): the waiting row queue is
    /// replaced, so a row that left the view before its fetch started is
    /// dropped, and the requests with a free row slot start now. The Hero's
    /// queue drains first (design D7), so a Hero fetch never waits on a row.
    pub fn fetch_row_images(&mut self, requests: Vec<ImageFetchReq>) {
        for req in self.images.replace_pending_row_fetches(requests) {
            self.spawn_image_fetch(req);
        }
    }

    /// Drop the waiting in-view row fetches (design D7): each `RowsInView` set
    /// replaces the queue, so a row that left the view before its fetch started
    /// is never fetched, and its loading mark clears so a later reserve of the
    /// same key is not refused as a duplicate.
    pub fn drop_pending_row_fetches(&mut self) {
        self.images.replace_pending_row_fetches(Vec::new());
    }

    /// Pre-warm the neighbour album artwork the Grouped Music tree resolved
    /// from its latest completed paint (task 6.5, design D4). The typed
    /// targets are the tree's ordered stable album identities (an opaque
    /// `id\0index` target for duplicate rows); the shell derives the album ID
    /// and walks the shared `{id}:P` album-art chain the hero projection
    /// consumes — it re-resolves no tree cursor or window. Each fetch keeps
    /// the existing idle gate (`fetch_list_card_image_when_idle`), so rapid
    /// navigation suppresses the whole window.
    pub fn prefetch_neighbour_album_art(&mut self, targets: &[String]) {
        if !self.images.protocol_enabled() {
            return;
        }
        for target in targets {
            // The tree's duplicate-row targets are opaque `id\0index` strings;
            // the album ID before the occurrence suffix is what the `{id}:P`
            // album-art chain keys on (the same split
            // `music_artist_detail::target_album_id` performs for the track
            // caches).
            let album_id = target.split('\0').next().unwrap_or(target);
            if album_id.is_empty() {
                continue;
            }
            self.fetch_list_card_image_when_idle(
                format!("{album_id}:P"),
                album_id.to_string(),
                String::new(),
                mbv_ui_model::playback::MUSIC_ALBUM_IMAGE_TYPES,
            );
        }
    }
}
