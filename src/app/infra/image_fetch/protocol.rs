use super::super::super::App;
use mbv_images::{
    ImageFetchReq, ImageSource, QUEUE_CARD_PLACEHOLDER_BYTES, QUEUE_CARD_PLACEHOLDER_KEY,
    RENDER_FILTER, cover_fill_hero_box,
};
use mbv_theme as palette;
use ratatui_image::picker::Picker;
use std::io::Read as IoRead;

fn color_rgb(color: ratatui::style::Color) -> [u8; 3] {
    match color {
        ratatui::style::Color::Rgb(r, g, b) => [r, g, b],
        _ => unreachable!("playback overlay roles are RGB colours"),
    }
}

/// Resolve the overlay's foreground roles. `title` is read only for the bottom
/// row of a two-part title: it paints the warm `TEXT_EMPHASIS` cream without
/// touching the shared `PLAYBACK_TITLE_FG` role the playback strip owns. The
/// context row — and a one-part title, which paints as the top text — keeps
/// `PLAYBACK_CONTEXT_FG`.
fn title_overlay_colours() -> mbv_images::title_overlay::TitleOverlayColours {
    mbv_images::title_overlay::TitleOverlayColours {
        context: color_rgb(mbv_theme::PLAYBACK_CONTEXT_FG),
        title: color_rgb(mbv_theme::TEXT_EMPHASIS),
    }
}

fn compose_title_overlay_bitmap(
    source: &image::DynamicImage,
    logo: Option<&image::DynamicImage>,
    font: ratatui_image::FontSize,
    cols: u16,
    rows: u16,
    text: mbv_images::title_overlay::TitleOverlayText<'_>,
) -> image::DynamicImage {
    let base = source.resize_exact(
        u32::from(cols) * u32::from(font.width.max(1)),
        u32::from(rows) * u32::from(font.height.max(1)),
        RENDER_FILTER,
    );
    let colours = title_overlay_colours();
    mbv_images::title_overlay::compose_title_overlay(&base, logo, font, text, colours)
}

impl App {
    /// Pre-warm nearby movie poster images for the migrated browser owner.
    /// The caller supplies the projected item window and the owner's
    /// authoritative cursor; only a selected, non-folder Movie enables the
    /// surrounding prefetch.
    pub(in crate::app) fn fetch_nearby_movie_posters(
        &mut self,
        items: &[mbv_emby_model::EmbyItem],
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
        if self.images.is_cached(QUEUE_CARD_PLACEHOLDER_KEY) {
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
            .insert_image(QUEUE_CARD_PLACEHOLDER_KEY.to_string(), entry);
    }

    fn picker_and_suffix(&self) -> Option<(&Picker, &'static str)> {
        if self.dim_halfblock_forced() {
            self.images.halfblock_picker().map(|p| (p, "halfblock"))
        } else {
            self.images
                .image_picker()
                .map(|p| (p, self.images.configured_protocol_name()))
        }
    }

    /// Whether the dimmed backdrop has forced the halfblock fallback picker
    /// over a differently-configured protocol (#451). The fallback picker
    /// measures cells on its hardcoded font grid, not the terminal font size
    /// the configured picker detected, so a paint made under the fallback is
    /// not geometry-comparable with the configured protocol's. Consumers that
    /// feed geometry feedback loops (the queue card's size checkpoint and its
    /// title-overlay sizing) must hold the configured protocol's geometry
    /// while this is up.
    pub(in crate::app) fn dim_halfblock_forced(&self) -> bool {
        self.dim_backdrop_active
            && self.images.protocol_enabled()
            && !self.images.is_halfblock_configured()
    }

    /// The suffix of the protocol currently active: the halfblock picker's
    /// while a dimmed backdrop is up, else the configured picker's.
    pub(in crate::app) fn current_protocol_suffix(&self) -> &'static str {
        self.picker_and_suffix().map_or("halfblock", |(_, s)| s)
    }

    /// Returns the protocol to render `bare_key` with under the currently
    /// active suffix, lazily re-encoding the retained source image when that
    /// suffix's protocol isn't cached yet (#451). The re-encode runs off the
    /// render thread (via the resize worker), so the first frame after a
    /// protocol switch still shows the placeholder while it completes.
    pub(in crate::app) fn cached_image_protocol_mut(
        &mut self,
        bare_key: &str,
    ) -> Option<&mut ratatui_image::thread::ThreadProtocol> {
        let suffix = self.current_protocol_suffix();
        self.cached_image_protocol_for_suffix_mut(bare_key, suffix)
    }

    /// `cached_image_protocol_mut` against an explicit protocol suffix:
    /// resolves (lazily re-encoding from the retained source) the protocol
    /// built for `suffix` rather than the currently active one. The queue
    /// card's title-overlay sizing uses the configured suffix while the
    /// dimmed backdrop forces the halfblock fallback, so the overlay's cache
    /// key — which encodes the measured cols/rows — does not change when a
    /// modal opens.
    pub(in crate::app) fn cached_image_protocol_for_suffix_mut(
        &mut self,
        bare_key: &str,
        suffix: &'static str,
    ) -> Option<&mut ratatui_image::thread::ThreadProtocol> {
        let picker = self.images.picker_for_suffix(suffix)?;
        let reencode = self
            .images
            .image(bare_key)
            .is_some_and(|e| e.img.is_some() && !e.protocols.contains_key(suffix));
        if reencode {
            let (img, cover_box, stored_logo_key) = self
                .images
                .image(bare_key)
                .and_then(|e| {
                    e.img
                        .clone()
                        .map(|img| (img, e.cover_box, e.applied_logo_key.clone()))
                })
                .expect("img present, just checked");
            let logo_key = self.images.ready_logo_key(stored_logo_key.as_deref());
            // A hero entry's protocols carry the cover-fit crop (task 5.10,
            // design D5): rebuild from the source through the same cover step
            // so a suffix switch keeps the cropped aspect.
            let img = match cover_box {
                Some((w, h)) => {
                    let (px_w, px_h) = self.hero_box_pixels(w, h);
                    cover_fill_hero_box(&img, px_w, px_h)
                }
                None => img,
            };
            let img = self.images.decorate_with_logo(img, logo_key.as_deref());
            let proto = self.images.build_protocol(bare_key, suffix, picker, img);
            if let Some(entry) = self.images.image_mut(bare_key) {
                entry.protocols.insert(suffix, proto);
                entry.applied_logo_key = logo_key;
            }
        }
        self.images.image_mut(bare_key)?.protocols.get_mut(suffix)
    }

    /// The active picker's terminal font size — the cell-to-pixel ratio the
    /// hero cover fit's box pixels derive from. Falls back to the picker
    /// constructor's default when no picker is active.
    pub(in crate::app) fn image_font_size(&self) -> ratatui_image::FontSize {
        self.picker_and_suffix()
            .map_or(ratatui_image::FontSize::new(10, 20), |(picker, _)| {
                picker.font_size()
            })
    }

    /// The font the queue card's title-overlay bitmap is composed at: always
    /// the configured protocol's detected font size, never the dimmed
    /// backdrop's halfblock fallback. `title_protocol_size` measures the
    /// overlay on that same configured grid while the fallback is forced, and
    /// the variant key encodes cols/rows but not the font — composing the
    /// same key at the fallback's hardcoded grid would bake a stretched
    /// bitmap into a key that outlives the modal.
    fn title_overlay_font_size(&self) -> ratatui_image::FontSize {
        self.images.image_picker().map_or(
            ratatui_image::FontSize::new(10, 20),
            ratatui_image::picker::Picker::font_size,
        )
    }

    /// One hero artwork box's pixel size from its cell size (task 5.10,
    /// design D5's cover-fit input).
    pub(in crate::app) fn hero_box_pixels(&self, box_w: u16, box_h: u16) -> (u32, u32) {
        let font = self.image_font_size();
        (
            u32::from(box_w) * u32::from(font.width.max(1)),
            u32::from(box_h) * u32::from(font.height.max(1)),
        )
    }

    /// The size `key`'s cached protocol paints within `available`, if it is ready.
    fn title_protocol_size(
        &mut self,
        key: &str,
        available: ratatui::layout::Size,
    ) -> Option<ratatui::layout::Size> {
        // While the dimmed backdrop forces the halfblock fallback, measure on
        // the configured protocol's grid: the overlay's cache key encodes the
        // measured cols/rows, so a fallback-measured variant would be a new,
        // never-painted key and flip the title site back to the header row
        // until the fallback paint landed (user-reported: the now-playing
        // title flashed to the header for a moment on every modal open).
        let suffix = if self.dim_halfblock_forced() {
            self.images.configured_protocol_name()
        } else {
            self.current_protocol_suffix()
        };
        self.cached_image_protocol_for_suffix_mut(key, suffix)?
            .size_for(ratatui_image::Resize::Scale(Some(RENDER_FILTER)), available)
    }

    /// Compose and encode the title artwork at the size the base card protocol paints.
    pub(in crate::app) fn ensure_title_overlay_protocol(
        &mut self,
        cache_key: &str,
        available: ratatui::layout::Size,
        parts: &mbv_queue::PlaybackTitleParts,
        logo_cache_key: Option<&str>,
        item_kind: &str,
    ) -> Option<String> {
        let Some(entry) = self.images.image(cache_key) else {
            self.title_log_gate
                .log_decision(cache_key, item_kind, "NoBaseEntry", None);
            return None;
        };
        let source_dimensions = entry.img.as_ref().map(image::GenericImageView::dimensions);
        let Some(size) = self.title_protocol_size(cache_key, available) else {
            self.title_log_gate.log_decision(
                cache_key,
                item_kind,
                "NoBaseProtocolSize",
                source_dimensions,
            );
            return None;
        };
        let (cols, rows) = (size.width, size.height);
        let context = parts.context.as_ref().map(|part| part.text.as_str());
        let overlay_text = mbv_images::title_overlay::TitleOverlayText {
            context,
            title: &parts.title.text,
        };
        // The ready logo joins the variant key (design D7): a late-arriving
        // logo builds a new variant, while an absent or failed one leaves the
        // text variant valid.
        let ready_logo_key = self.images.ready_logo_key(logo_cache_key);
        let key = mbv_images::title_overlay::title_overlay_cache_key(
            cache_key,
            cols,
            rows,
            overlay_text,
            ready_logo_key.as_deref(),
        );
        if !self.images.is_cached(&key)
            && !self.build_title_overlay_variant(
                cache_key,
                &key,
                cols,
                rows,
                overlay_text,
                ready_logo_key.as_deref(),
                item_kind,
            )
        {
            return None;
        }
        if self.title_protocol_size(&key, available).is_none() {
            self.title_log_gate.log_decision(
                cache_key,
                item_kind,
                "VariantNotReady",
                source_dimensions,
            );
            return None;
        }
        Some(key)
    }

    /// Compose the overlay variant from the base entry's source and the ready
    /// logo, then register it under `key` (design D3/D7). Returns whether the
    /// variant was composed; a missing base bitmap logs and leaves the text
    /// site to the caller.
    fn build_title_overlay_variant(
        &mut self,
        cache_key: &str,
        key: &str,
        cols: u16,
        rows: u16,
        text: mbv_images::title_overlay::TitleOverlayText<'_>,
        ready_logo_key: Option<&str>,
        item_kind: &str,
    ) -> bool {
        let Some(source) = self
            .images
            .image(cache_key)
            .and_then(|entry| entry.img.as_ref())
        else {
            self.title_log_gate
                .log_decision(cache_key, item_kind, "BaseImageMissing", None);
            return false;
        };
        let source_dimensions = (source.width(), source.height());
        let logo = ready_logo_key
            .and_then(|key| self.images.image(key))
            .and_then(|entry| entry.img.as_ref());
        let composed = compose_title_overlay_bitmap(
            source,
            logo,
            self.title_overlay_font_size(),
            cols,
            rows,
            text,
        );
        let suffix = self.current_protocol_suffix();
        let entry = self.images.build_cached_image(key, Some(composed), suffix);
        self.images.insert_derived_image(key.to_owned(), entry);
        tracing::debug!(
            name: "queue.title_overlay.built",
            target: "queue_art",
            key,
            base_width = source_dimensions.0,
            base_height = source_dimensions.1,
            box_cols = cols,
            box_rows = rows,
            "built queue title overlay"
        );
        true
    }

    /// Ensure the hero cover-fit protocol for `cache_key` matches
    /// `box_cells` (task 5.10, design D5): the protocol is rebuilt from the
    /// decoded source through `cover_fill_hero_box` at the box's pixel size
    /// whenever the box changed (resize, split drag, Workspace shrink — the
    /// next sync pass's "re-encode request keyed by the new box size"), and
    /// the painters show the placeholder for at most that one frame.
    /// Returns whether a ready protocol is available.
    ///
    /// `resizing` is the shell-resolved "a resize drag is in progress" input
    /// (queue-column boundary or wide split). A drag changes the box on
    /// every mouse event, and each rebuild cover-fills the source with a
    /// Lanczos3 resize synchronously on the sync thread (user-reported
    /// regression, 2026-10-09: dragging either boundary was super slow while
    /// a landscape hero was up, while non-overlaid heroes dragged fine).
    /// While the drag is live an existing encoding is kept — the painter
    /// scales it into the moving box off-thread — and the drag's final box
    /// re-encodes once the drag ends.
    pub(in crate::app) fn ensure_hero_cover_protocol(
        &mut self,
        cache_key: &str,
        box_cells: (u16, u16),
        logo_cache_key: Option<&str>,
        resizing: bool,
    ) -> bool {
        let Some(entry) = self.images.image(cache_key) else {
            return false;
        };
        let Some(source) = entry.img.clone() else {
            return false;
        };
        let desired_logo_key = self.images.ready_logo_key(logo_cache_key);
        if entry.cover_box == Some(box_cells)
            && entry.applied_logo_key == desired_logo_key
            && !entry.protocols.is_empty()
        {
            return true;
        }
        // No encoding yet: the first paint during a drag must still build.
        // An existing encoding is kept across the drag and re-encodes once at
        // the drag's final box.
        if resizing && !entry.protocols.is_empty() {
            return true;
        }
        let Some((picker, suffix)) = self
            .picker_and_suffix()
            .map(|(picker, suffix)| (picker.clone(), suffix))
        else {
            return false;
        };
        let (px_w, px_h) = self.hero_box_pixels(box_cells.0, box_cells.1);
        let cropped = self.images.decorate_with_logo(
            cover_fill_hero_box(&source, px_w, px_h),
            desired_logo_key.as_deref(),
        );
        let bare_key = cache_key.to_string();
        let proto = self
            .images
            .build_protocol(&bare_key, suffix, &picker, cropped);
        if let Some(entry) = self.images.image_mut(cache_key) {
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
        paint: &mbv_components::library_panel::PanelHeroImagePaint,
    ) {
        if paint.area.width == 0 || paint.area.height == 0 {
            return;
        }
        if let Some(state) = self.cached_image_protocol_mut(&paint.cache_key) {
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

    /// Spawn queued image fetches until the in-flight limit is reached. Called
    /// whenever an in-flight fetch completes and frees a slot (see the card-image
    /// receiver in `images.rs`).
    pub(in crate::app) fn drain_image_fetches(&mut self) {
        while let Some(req) = self.images.take_pending_fetch(super::MAX_IMAGE_FETCHES) {
            self.spawn_image_fetch(req);
        }
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
    let client = mbv_audiobookshelf::AudiobookshelfClient::new(server_url).ok();
    let result = client.and_then(|client| {
        client
            .cover_bounded(
                api_key,
                item_id,
                mbv_audiobookshelf::AudiobookshelfClient::REQUEST_HARD_BOUND,
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
        tracing::debug!(name: "images.disk_cache.hit", target: "images", item = %item_id, "image disk cache hit");
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
    let agent = mbv_net::native_tls_agent(
        mbv_net::HttpService::Emby,
        None,
        Some(std::time::Duration::from_secs(10)),
    );
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
mod protocol_tests;
