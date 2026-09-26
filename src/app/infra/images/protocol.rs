use super::cache::ImageCache;
use super::{
    cover_fill_hero_box, mem_key, palette, App, CachedImage, ImageFetchReq, ImageSource, IoRead,
    Picker, MAX_IMAGE_FETCHES, QUEUE_CARD_PLACEHOLDER_BYTES, QUEUE_CARD_PLACEHOLDER_KEY,
    RENDER_FILTER,
};
impl App {
    /// Pre-warm nearby movie poster images for the migrated browser owner.
    /// The caller supplies the projected item window and the owner's
    /// authoritative cursor; only a selected, non-folder Movie enables the
    /// surrounding prefetch.
    pub(in crate::app) fn fetch_nearby_movie_posters(
        &mut self,
        items: &[mbv_core::api::EmbyItem],
        cursor: usize,
    ) {
        const PREFETCH_AHEAD: usize = 3;
        const PREFETCH_BEHIND: usize = 1;
        if !items
            .get(cursor)
            .is_some_and(|item| item.item_type == "Movie" && !item.is_folder)
        {
            return;
        }
        let start = cursor.saturating_sub(PREFETCH_BEHIND);
        let end = (cursor + PREFETCH_AHEAD + 1).min(items.len());
        let prefetch: Vec<(String, String, String)> = items[start..end]
            .iter()
            .enumerate()
            .filter(|(i, item)| start + i != cursor && item.item_type == "Movie" && !item.is_folder)
            .map(|(_, item)| {
                (
                    format!("{}:cmp_primary", item.id),
                    item.id.clone(),
                    item.series_id.clone(),
                )
            })
            .collect();
        if self.images.images_enabled() {
            for (cache_key, item_id, series_id) in prefetch {
                self.fetch_list_card_image_when_idle(cache_key, item_id, series_id, &["Primary"]);
            }
        }
    }

    pub(in crate::app) fn ensure_placeholder_card_image(&mut self) {
        if self
            .images
            .card_image_states
            .contains_key(QUEUE_CARD_PLACEHOLDER_KEY)
        {
            return;
        }
        if self.picker_and_suffix().is_none() {
            return;
        }
        let Ok(img) = image::load_from_memory(QUEUE_CARD_PLACEHOLDER_BYTES) else {
            return;
        };
        let entry = self.images.build_cached_image(
            QUEUE_CARD_PLACEHOLDER_KEY,
            Some(img),
            self.current_protocol_suffix(),
        );
        self.images
            .card_image_states
            .insert(QUEUE_CARD_PLACEHOLDER_KEY.to_string(), entry);
    }

    fn picker_and_suffix(&self) -> Option<(&Picker, &'static str)> {
        let use_halfblock = self.dim_backdrop_active
            && self.images.image_protocol_enabled
            && !self.images.is_halfblock_configured();
        if use_halfblock {
            self.images
                .halfblock_picker
                .as_ref()
                .map(|p| (p, "halfblock"))
        } else {
            self.images
                .image_picker
                .as_ref()
                .map(|p| (p, self.images.configured_protocol_name()))
        }
    }

    /// The suffix of the protocol currently active: the halfblock picker's
    /// while a dimmed backdrop is up, else the configured picker's.
    pub(in crate::app) fn current_protocol_suffix(&self) -> &'static str {
        self.picker_and_suffix().map_or("halfblock", |(_, s)| s)
    }
}

impl ImageCache {
    fn build_image_picker(&self) -> Picker {
        use ratatui_image::picker::ProtocolType;
        let protocol_override = self.image_protocol.clone();
        let mut picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
        let proto = protocol_override
            .as_deref()
            .and_then(|s| match s.to_lowercase().as_str() {
                "sixel" => Some(ProtocolType::Sixel),
                "kitty" => Some(ProtocolType::Kitty),
                "iterm2" => Some(ProtocolType::Iterm2),
                "halfblocks" => Some(ProtocolType::Halfblocks),
                _ => None,
            });
        if let Some(proto) = proto {
            picker.set_protocol_type(proto);
        }
        picker
    }

    pub(in crate::app) fn init_image_pickers(&mut self) {
        let picker = self.build_image_picker();
        log::debug!(
            target: "startup",
            "image picker: protocol={:?} font_size={:?}",
            picker.protocol_type(),
            picker.font_size()
        );
        self.image_picker = Some(picker);
        self.halfblock_picker = Some(Picker::halfblocks());
    }

    /// The picker that encodes the given protocol suffix.
    fn picker_for_suffix(&self, suffix: &'static str) -> Option<&Picker> {
        if suffix == "halfblock" {
            self.halfblock_picker
                .as_ref()
                .or(self.image_picker.as_ref())
        } else {
            self.image_picker.as_ref()
        }
    }

    /// Builds a fresh cache entry for a just-fetched image: keeps the decoded
    /// source so it can be re-encoded with a different protocol picker later
    /// (#451), and encodes the protocol for the active suffix.
    /// `img: None` records a resolved-but-empty fetch (the "no art" marker
    /// renderers branch on).
    pub(in crate::app) fn build_cached_image(
        &self,
        bare_key: &str,
        img: Option<image::DynamicImage>,
        suffix: &'static str,
    ) -> CachedImage {
        let mut entry = CachedImage {
            img,
            protocols: std::collections::HashMap::new(),
            cover_box: None,
            applied_logo_key: None,
        };
        if let Some(img) = entry.img.clone() {
            if let Some(picker) = self.picker_for_suffix(suffix) {
                let proto = self.build_protocol(bare_key, suffix, picker, img);
                entry.protocols.insert(suffix, proto);
            }
        }
        entry
    }

    /// Returns the protocol to render `bare_key` with under the currently
    /// active suffix, lazily re-encoding the retained source image when that
    /// suffix's protocol isn't cached yet (#451). The re-encode runs off the
    /// render thread (via the resize worker), so the first frame after a
    /// protocol switch still shows the placeholder while it completes.
    pub(in crate::app) fn cached_image_protocol_mut(
        &mut self,
        bare_key: &str,
        suffix: &'static str,
    ) -> Option<&mut ratatui_image::thread::ThreadProtocol> {
        let picker = self.picker_for_suffix(suffix)?;
        let reencode = self
            .card_image_states
            .get(bare_key)
            .is_some_and(|e| e.img.is_some() && !e.protocols.contains_key(suffix));
        if reencode {
            let (img, cover_box, stored_logo_key) = self
                .card_image_states
                .get(bare_key)
                .and_then(|e| {
                    e.img
                        .clone()
                        .map(|img| (img, e.cover_box, e.applied_logo_key.clone()))
                })
                .expect("img present, just checked");
            let logo_key = self.ready_logo_key(stored_logo_key.as_deref());
            // A hero entry's protocols carry the cover-fit crop (task 5.10,
            // design D5): rebuild from the source through the same cover step
            // so a suffix switch keeps the cropped aspect.
            let img = match cover_box {
                Some((w, h)) => {
                    let (px_w, px_h) = self.hero_box_pixels(w, h, suffix);
                    crate::app::infra::images::cover_fill_hero_box(&img, px_w, px_h)
                }
                None => img,
            };
            let img = self.decorate_with_logo(img, logo_key.as_deref());
            let proto = self.build_protocol(bare_key, suffix, picker, img);
            if let Some(entry) = self.card_image_states.get_mut(bare_key) {
                entry.protocols.insert(suffix, proto);
                entry.applied_logo_key = logo_key;
            }
        }
        self.card_image_states
            .get_mut(bare_key)?
            .protocols
            .get_mut(suffix)
    }

    fn build_protocol(
        &self,
        bare_key: &str,
        suffix: &'static str,
        picker: &Picker,
        img: image::DynamicImage,
    ) -> ratatui_image::thread::ThreadProtocol {
        #[cfg(test)]
        self.image_protocol_builds
            .set(self.image_protocol_builds.get() + 1);
        let mem_key = mem_key(bare_key, suffix);
        let (req_tx, req_rx) = std::sync::mpsc::channel::<ratatui_image::thread::ResizeRequest>();
        let _ = self.resize_register_tx.send((mem_key, req_rx));
        ratatui_image::thread::ThreadProtocol::new(req_tx, Some(picker.new_resize_protocol(img)))
    }

    pub(in crate::app) fn is_halfblock_configured(&self) -> bool {
        self.image_protocol
            .as_deref()
            .is_some_and(|s| s.eq_ignore_ascii_case("halfblocks"))
            || self.image_picker.as_ref().is_some_and(|p| {
                p.protocol_type() == ratatui_image::picker::ProtocolType::Halfblocks
            })
    }

    pub(in crate::app) fn configured_protocol_name(&self) -> &'static str {
        use ratatui_image::picker::ProtocolType;
        match self.image_picker.as_ref().map(Picker::protocol_type) {
            Some(ProtocolType::Sixel) => "sixel",
            Some(ProtocolType::Kitty) => "kitty",
            Some(ProtocolType::Iterm2) => "iterm2",
            Some(ProtocolType::Halfblocks) | None => "halfblock",
        }
    }
}

impl App {
    /// Spawn queued image fetches until the in-flight limit is reached. Called
    /// whenever an in-flight fetch completes and frees a slot (see the card-image
    /// receiver in `images.rs`).
    pub(in crate::app) fn drain_image_fetches(&mut self) {
        while self.images.image_fetches_active < MAX_IMAGE_FETCHES {
            let Some(req) = self.images.pending_image_fetches.pop_front() else {
                break;
            };
            self.spawn_image_fetch(req);
        }
    }

    pub(super) fn spawn_image_fetch(&mut self, req: ImageFetchReq) {
        self.images.image_fetches_active += 1;
        let (server_url, token) = if matches!(req.source, ImageSource::Emby) {
            let Some(client) = self.emby_client() else {
                self.images.image_fetches_active =
                    self.images.image_fetches_active.saturating_sub(1);
                let _ = self.images.card_image_tx.send((req.cache_key, None));
                return;
            };
            let c = client.lock().unwrap();
            (c.config.server_url.clone(), c.token.clone())
        } else {
            (String::new(), String::new())
        };
        let tx = self.images.card_image_tx.clone();
        std::thread::spawn(move || {
            // catch_unwind so a panic during fetch/decode still reports a result,
            // freeing the in-flight slot and the loading reservation (H9). Exactly
            // one message is sent per spawn, so the receiver can balance the count.
            let cache_key = req.cache_key.clone();
            let cache_key_outer = cache_key.clone();
            let tx_outer = tx.clone();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let bytes = fetch_image_bytes(req, &server_url, &token);
                // Decode off the UI thread; the main loop only builds the protocol.
                let img = bytes.and_then(|b| image::load_from_memory(&b).ok());
                let _ = tx.send((cache_key, img));
            }));
            if result.is_err() {
                let _ = tx_outer.send((cache_key_outer, None));
            }
        });
    }
}

impl ImageCache {
    pub(in crate::app) fn images_enabled(&self) -> bool {
        self.image_protocol_enabled
    }

    /// The active picker's terminal font size — the cell-to-pixel ratio the
    /// hero cover fit's box pixels derive from. Falls back to the halfblock
    /// picker and then the picker constructor's default.
    pub(in crate::app) fn image_font_size(&self, suffix: &'static str) -> ratatui_image::FontSize {
        self.picker_for_suffix(suffix)
            .map_or(ratatui_image::FontSize::new(10, 20), Picker::font_size)
    }

    /// One hero artwork box's pixel size from its cell size (task 5.10,
    /// design D5's cover-fit input).
    pub(in crate::app) fn hero_box_pixels(
        &self,
        box_w: u16,
        box_h: u16,
        suffix: &'static str,
    ) -> (u32, u32) {
        let font = self.image_font_size(suffix);
        (
            u32::from(box_w) * u32::from(font.width.max(1)),
            u32::from(box_h) * u32::from(font.height.max(1)),
        )
    }

    /// Resolve an optional Logo cache key to the key of a Logo that has decoded
    /// pixels to composite: a pending, absent, or failed Logo is not a
    /// decoration input, so the base-only protocol stays valid.
    fn ready_logo_key(&self, logo_cache_key: Option<&str>) -> Option<String> {
        logo_cache_key
            .filter(|key| {
                self.card_image_states
                    .get(*key)
                    .is_some_and(|entry| entry.img.is_some())
            })
            .map(str::to_owned)
    }

    /// Paint the ready Logo at `logo_cache_key` over `img`, or return `img`
    /// unchanged when there is none (design D3).
    fn decorate_with_logo(
        &self,
        img: image::DynamicImage,
        logo_cache_key: Option<&str>,
    ) -> image::DynamicImage {
        let Some(logo) = logo_cache_key
            .and_then(|key| self.card_image_states.get(key))
            .and_then(|entry| entry.img.as_ref())
        else {
            return img;
        };
        crate::app::infra::images::composite_landscape_logo(&img, logo)
    }

    /// Ensure the hero cover-fit protocol for `cache_key` matches
    /// `box_cells` (task 5.10, design D5): the protocol is rebuilt from the
    /// decoded source through `cover_fill_hero_box` at the box's pixel size
    /// whenever the box changed (resize, split drag, Workspace shrink — the
    /// next sync pass's "re-encode request keyed by the new box size"), and
    /// the painters show the placeholder for at most that one frame.
    /// Returns whether a ready protocol is available.
    pub(in crate::app) fn ensure_hero_cover_protocol(
        &mut self,
        cache_key: &str,
        box_cells: (u16, u16),
        logo_cache_key: Option<&str>,
        suffix: &'static str,
    ) -> bool {
        let Some(entry) = self.card_image_states.get(cache_key) else {
            return false;
        };
        let Some(source) = entry.img.clone() else {
            return false;
        };
        let desired_logo_key = self.ready_logo_key(logo_cache_key);
        if entry.cover_box == Some(box_cells)
            && entry.applied_logo_key == desired_logo_key
            && !entry.protocols.is_empty()
        {
            return true;
        }
        let Some(picker) = self.picker_for_suffix(suffix).cloned() else {
            return false;
        };
        let (px_w, px_h) = self.hero_box_pixels(box_cells.0, box_cells.1, suffix);
        let cropped = self.decorate_with_logo(
            cover_fill_hero_box(&source, px_w, px_h),
            desired_logo_key.as_deref(),
        );
        let bare_key = cache_key.to_string();
        let proto = self.build_protocol(&bare_key, suffix, &picker, cropped);
        if let Some(entry) = self.card_image_states.get_mut(cache_key) {
            entry.protocols.clear();
            entry.protocols.insert(suffix, proto);
            entry.cover_box = Some(box_cells);
            entry.applied_logo_key = desired_logo_key;
        }
        true
    }

    /// Paints the panel's retained hero image paint (task 5.10, design D9):
    /// the cached protocol rendered into the projected box (cover-fit keyed
    /// by the box at the projection), or — while that one frame's encode is
    /// still completing — the shared loading placeholder in the same box, so
    /// the placeholder never shows for more than the one frame the spec
    /// allows.
    pub(in crate::app) fn paint_panel_hero_image(
        &mut self,
        f: &mut ratatui::Frame,
        paint: &crate::app::components::library_panel::PanelHeroImagePaint,
        suffix: &'static str,
    ) {
        if paint.area.width == 0 || paint.area.height == 0 {
            return;
        }
        if let Some(state) = self.cached_image_protocol_mut(&paint.cache_key, suffix) {
            type SImg = ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
            let avail = ratatui::layout::Size {
                width: paint.area.width,
                height: paint.area.height,
            };
            if let Some(actual) =
                state.size_for(ratatui_image::Resize::Scale(Some(RENDER_FILTER)), avail)
            {
                let img_rect = ratatui::layout::Rect {
                    x: paint.area.x + paint.area.width.saturating_sub(actual.width) / 2,
                    y: paint.area.y,
                    width: actual.width,
                    height: actual.height,
                };
                f.render_stateful_widget(
                    SImg::default().resize(ratatui_image::Resize::Scale(Some(RENDER_FILTER))),
                    img_rect,
                    state,
                );
                return;
            }
        }
        f.render_widget(
            ratatui::widgets::Block::default().style(ratatui::style::Style::default().bg(
                palette::surface_colors(palette::Surface::ArtworkLoadingPlaceholder, false).fill,
            )),
            paint.area,
        );
    }
}

fn fetch_image_bytes(req: ImageFetchReq, emby_url: &str, token: &str) -> Option<Vec<u8>> {
    let ImageFetchReq {
        cache_key,
        item_id,
        series_id,
        types,
        source,
    } = req;
    match source {
        ImageSource::Audiobookshelf {
            server_url,
            api_key,
        } => fetch_audiobookshelf_image(&cache_key, &server_url, &api_key, &item_id),
        ImageSource::Emby => {
            fetch_emby_image(&cache_key, &item_id, &series_id, &types, emby_url, token)
        }
    }
}

fn fetch_audiobookshelf_image(
    cache_key: &str,
    server_url: &str,
    api_key: &str,
    item_id: &str,
) -> Option<Vec<u8>> {
    if let Some(cached) = crate::config::read_image_disk_cache(cache_key) {
        return Some(cached);
    }
    let client = mbv_core::audiobookshelf::AudiobookshelfClient::new(server_url).ok();
    let result = client.and_then(|client| {
        client
            .cover_bounded(
                api_key,
                item_id,
                mbv_core::audiobookshelf::AudiobookshelfClient::REQUEST_HARD_BOUND,
            )
            .ok()
    });
    if let Some(ref bytes) = result {
        crate::config::write_image_disk_cache(cache_key, bytes);
    }
    result
}

fn fetch_emby_image(
    cache_key: &str,
    item_id: &str,
    series_id: &str,
    types: &[String],
    server_url: &str,
    token: &str,
) -> Option<Vec<u8>> {
    if let Some(cached) = crate::config::read_image_disk_cache(cache_key) {
        // Local-only cache hits power dim-then-undim cycles for warm-cache modals.
        log::debug!(target: "images", "image disk cache hit for {cache_key}");
        return Some(cached);
    }
    let fetched = types
        .iter()
        .find_map(|kind| fetch_emby_image_type(kind, item_id, series_id, server_url, token));
    // Cache original server bytes as-is; Emby already resized these to maxHeight=400.
    if let Some(ref bytes) = fetched {
        crate::config::write_image_disk_cache(cache_key, bytes);
    }
    fetched
}

fn fetch_emby_image_type(
    kind: &str,
    item_id: &str,
    series_id: &str,
    server_url: &str,
    token: &str,
) -> Option<Vec<u8>> {
    if kind == "AudioChild" {
        let child_url = format!(
            "{server_url}/Items?ParentId={item_id}&IncludeItemTypes=Audio&Limit=1&api_key={token}"
        );
        let child_id = fetch_url(&child_url)
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|value| {
                value["Items"]
                    .get(0)
                    .and_then(|item| item["Id"].as_str().map(str::to_owned))
            })?;
        let url = format!(
            "{server_url}/Items/{child_id}/Images/Primary?maxHeight=400&quality=80&api_key={token}"
        );
        return fetch_url(&url);
    }

    let src = match kind {
        "Logo" | "Backdrop" | "Thumb" if !series_id.is_empty() => series_id,
        _ => item_id,
    };
    let image_kind = match kind {
        "Backdrop" => "Backdrop/0",
        "Logo" => "Logo",
        "Thumb" => "Thumb",
        _ => "Primary",
    };
    let url = format!(
        "{server_url}/Items/{src}/Images/{image_kind}?maxHeight=400&quality=80&api_key={token}"
    );
    fetch_url(&url)
}

fn fetch_url(url: &str) -> Option<Vec<u8>> {
    let agent = crate::app::infra::feed_parse::tls_agent(Some(std::time::Duration::from_secs(10)));
    agent.get(url).call().ok().and_then(|response| {
        let mut bytes = Vec::new();
        response
            .into_body()
            .into_reader()
            .take(10 * 1024 * 1024)
            .read_to_end(&mut bytes)
            .ok()?;
        Some(bytes)
    })
}

#[cfg(test)]
mod protocol_tests {
    use super::super::App;
    use super::CachedImage;
    use crate::app::tests::make_app_stub;
    use ratatui_image::picker::{Picker, ProtocolType};

    const BASE_KEY: &str = "hero-base";
    const LOGO_KEY: &str = "hero-logo";
    const BOX: (u16, u16) = (8, 4);

    fn image(width: u32, height: u32) -> image::DynamicImage {
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            width,
            height,
            image::Rgba([20, 40, 60, 255]),
        ))
    }

    fn cached(img: Option<image::DynamicImage>) -> CachedImage {
        CachedImage {
            img,
            protocols: std::collections::HashMap::new(),
            cover_box: None,
            applied_logo_key: None,
        }
    }

    fn app_with_base() -> App {
        let mut app = make_app_stub();
        app.images.image_protocol_enabled = true;
        let mut picker = Picker::halfblocks();
        picker.set_protocol_type(ProtocolType::Kitty);
        app.images.image_picker = Some(picker);
        app.images.halfblock_picker = Some(Picker::halfblocks());
        app.images
            .card_image_states
            .insert(BASE_KEY.to_owned(), cached(Some(image(4, 2))));
        app
    }

    fn build_count(app: &App) -> u32 {
        app.images.image_protocol_builds.get()
    }

    #[test]
    fn arriving_logo_rebuilds_base_only_protocol_once() {
        let mut app = app_with_base();

        assert!(app
            .images
            .ensure_hero_cover_protocol(BASE_KEY, BOX, None, "kitty"));
        assert_eq!(build_count(&app), 1);

        app.images
            .card_image_states
            .insert(LOGO_KEY.to_owned(), cached(Some(image(2, 1))));
        assert!(app
            .images
            .ensure_hero_cover_protocol(BASE_KEY, BOX, Some(LOGO_KEY), "kitty"));
        assert_eq!(build_count(&app), 2);
        assert_eq!(
            app.images
                .card_image_states
                .get(BASE_KEY)
                .and_then(|entry| entry.applied_logo_key.as_deref()),
            Some(LOGO_KEY)
        );

        assert!(app
            .images
            .ensure_hero_cover_protocol(BASE_KEY, BOX, Some(LOGO_KEY), "kitty"));
        assert_eq!(build_count(&app), 2);
    }

    #[test]
    fn failed_or_absent_logo_keeps_base_only_protocol_valid() {
        let mut absent = app_with_base();
        assert!(absent
            .images
            .ensure_hero_cover_protocol(BASE_KEY, BOX, None, "kitty"));
        assert!(absent
            .images
            .ensure_hero_cover_protocol(BASE_KEY, BOX, Some(LOGO_KEY), "kitty"));
        assert_eq!(build_count(&absent), 1);
        assert!(absent
            .images
            .card_image_states
            .get(BASE_KEY)
            .is_some_and(|entry| entry.applied_logo_key.is_none()));

        let mut failed = app_with_base();
        failed
            .images
            .card_image_states
            .insert(LOGO_KEY.to_owned(), CachedImage::empty());
        assert!(failed
            .images
            .ensure_hero_cover_protocol(BASE_KEY, BOX, Some(LOGO_KEY), "kitty"));
        assert!(failed
            .images
            .ensure_hero_cover_protocol(BASE_KEY, BOX, Some(LOGO_KEY), "kitty"));
        assert_eq!(build_count(&failed), 1);
        assert!(failed
            .images
            .card_image_states
            .get(BASE_KEY)
            .is_some_and(|entry| entry.applied_logo_key.is_none()));
    }
}
