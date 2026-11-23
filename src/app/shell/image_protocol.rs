//! The TUI Client's image-protocol acceptance (`extract-client-core` design
//! D3): the ratatui and `ratatui-image` halves of the image cache that the
//! shell alone runs — encoding a fetched entry under the active terminal
//! protocol and painting it. The cache, the fetch, and the completion drain
//! stay in `crate::app::infra::image_fetch`, so a Client with no terminal
//! protocol holds the same cache.

use super::Model;
use mbv_images::{QUEUE_CARD_PLACEHOLDER_BYTES, QUEUE_CARD_PLACEHOLDER_KEY, cover_fill_hero_box};
use mbv_theme as palette;
use mbv_tui_support::{RENDER_FILTER, ToCellSize, ToRatatui};
use ratatui_image::picker::Picker;

#[cfg(test)]
mod tests;

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
        context: color_rgb(mbv_theme::Role::PlaybackContextFg.color().to_ratatui()),
        title: color_rgb(mbv_theme::Role::TextEmphasis.color().to_ratatui()),
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
    mbv_images::title_overlay::compose_title_overlay(
        &base,
        logo,
        font.to_cell_size(),
        text,
        colours,
    )
}

/// The picker that encodes the active protocol suffix, and that suffix:
/// the halfblock picker's while a dimmed backdrop is up, else the
/// configured picker's. `None` until the pickers are initialized.
fn picker_and_suffix(model: &Model) -> Option<(&Picker, &'static str)> {
    if model.app.dim_halfblock_forced() {
        model
            .image_protocols
            .halfblock_picker()
            .map(|p| (p, "halfblock"))
    } else {
        model
            .image_protocols
            .image_picker()
            .map(|p| (p, model.image_protocols.configured_protocol_name()))
    }
}

/// [`picker_and_suffix`] without the picker, for a caller that only
/// needs the suffix: `None` until the pickers are initialized.
fn current_protocol_suffix_opt(model: &Model) -> Option<&'static str> {
    picker_and_suffix(model).map(|(_, suffix)| suffix)
}

/// The size `key`'s cached protocol paints within `available` (a card box in
/// cells, `width` and `height`), if it is ready.
fn title_protocol_size(
    model: &mut Model,
    key: &str,
    available: (u16, u16),
) -> Option<ratatui::layout::Size> {
    // While the dimmed backdrop forces the halfblock fallback, measure on
    // the configured protocol's grid: the overlay's cache key encodes the
    // measured cols/rows, so a fallback-measured variant would be a new,
    // never-painted key and flip the title site back to the header row
    // until the fallback paint landed (user-reported: the now-playing
    // title flashed to the header for a moment on every modal open).
    let suffix = if model.app.dim_halfblock_forced() {
        model.image_protocols.configured_protocol_name()
    } else {
        model.app.image_protocol_suffix()
    };
    let available = ratatui::layout::Size {
        width: available.0,
        height: available.1,
    };
    model
        .cached_image_protocol_for_suffix_mut(key, suffix)?
        .size_for(ratatui_image::Resize::Scale(Some(RENDER_FILTER)), available)
}

/// Compose the overlay variant from the base entry's source and the ready
/// logo, then register it under `key` (design D3/D7). Returns whether the
/// variant was composed; a missing base bitmap logs and leaves the text
/// site to the caller.
fn build_title_overlay_variant(
    model: &mut Model,
    cache_key: &str,
    key: &str,
    cols: u16,
    rows: u16,
    text: mbv_images::title_overlay::TitleOverlayText<'_>,
    ready_logo_key: Option<&str>,
    item_kind: &str,
) -> bool {
    let Some(source) = model
        .app
        .images
        .image(cache_key)
        .and_then(|entry| entry.img.as_ref())
    else {
        model
            .app
            .title_log_gate
            .log_decision(cache_key, item_kind, "BaseImageMissing", None);
        return false;
    };
    let source_dimensions = (source.width(), source.height());
    let logo = ready_logo_key
        .and_then(|key| model.app.cached_image(key))
        .and_then(|entry| entry.img.as_ref());
    let composed = compose_title_overlay_bitmap(
        source,
        logo,
        ratatui_image::FontSize::new(model.app.image_font_size().0, model.app.image_font_size().1),
        cols,
        rows,
        text,
    );
    let suffix = model.app.image_protocol_suffix();
    let built = model
        .image_protocols
        .build_entry(key, Some(composed), None, suffix);
    model
        .app
        .images
        .insert_derived_image(key.to_owned(), built.entry);
    if let Some(protocol) = built.protocol {
        model
            .image_protocols
            .insert_protocol(model.app.image_cache(), key, suffix, protocol);
    }
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

impl Model {
    pub(in crate::app) fn ensure_placeholder_card_image(&mut self) {
        if self.app.image_cached(QUEUE_CARD_PLACEHOLDER_KEY) {
            return;
        }
        let Some(suffix) = current_protocol_suffix_opt(self) else {
            return;
        };
        let Ok(img) = image::load_from_memory(QUEUE_CARD_PLACEHOLDER_BYTES) else {
            return;
        };
        let built =
            self.image_protocols
                .build_entry(QUEUE_CARD_PLACEHOLDER_KEY, Some(img), None, suffix);
        self.app
            .images
            .insert_image(QUEUE_CARD_PLACEHOLDER_KEY.to_string(), built.entry);
        if let Some(protocol) = built.protocol {
            self.image_protocols.insert_protocol(
                self.app.image_cache(),
                QUEUE_CARD_PLACEHOLDER_KEY,
                suffix,
                protocol,
            );
        }
    }

    /// The suffix of the protocol currently active: the halfblock picker's
    /// while a dimmed backdrop is up, else the configured picker's.
    pub(in crate::app) fn current_protocol_suffix(&self) -> &'static str {
        picker_and_suffix(self).map_or("halfblock", |(_, s)| s)
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
        let picker = self.image_protocols.picker_for_suffix(suffix).cloned()?;
        let reencode = self.app.cached_image(bare_key).is_some_and(|e| {
            e.img.is_some()
                && !self
                    .image_protocols
                    .contains(self.app.image_cache(), bare_key, suffix)
        });
        if reencode {
            let (img, cover_box, stored_logo_key) = self
                .app
                .images
                .image(bare_key)
                .and_then(|e| {
                    e.img
                        .clone()
                        .map(|img| (img, e.cover_box, e.applied_logo_key.clone()))
                })
                .expect("img present, just checked");
            let logo_key = self
                .image_protocols
                .ready_logo_key(self.app.image_cache(), stored_logo_key.as_deref());
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
            let img = self.image_protocols.decorate_with_logo(
                self.app.image_cache(),
                img,
                logo_key.as_deref(),
            );
            let proto = self
                .image_protocols
                .build_protocol(bare_key, suffix, &picker, img);
            self.image_protocols
                .insert_protocol(self.app.image_cache(), bare_key, suffix, proto);
            if let Some(entry) = self.app.images.image_mut(bare_key) {
                entry.applied_logo_key = logo_key;
            }
        }
        self.image_protocols
            .protocol_mut(self.app.image_cache(), bare_key, suffix)
    }

    /// One hero artwork box's pixel size from its cell size (task 5.10,
    /// design D5's cover-fit input).
    pub(in crate::app) fn hero_box_pixels(&self, box_w: u16, box_h: u16) -> (u32, u32) {
        let (font_w, font_h) = self.app.active_image_font_size();
        (
            u32::from(box_w) * u32::from(font_w.max(1)),
            u32::from(box_h) * u32::from(font_h.max(1)),
        )
    }

    /// Compose and encode the title artwork at the size the base card protocol
    /// paints. `available` is the card box in cells (`width`, `height`), passed
    /// as plain numbers so the caller of this core path needs no ratatui value.
    pub(in crate::app) fn ensure_title_overlay_protocol(
        &mut self,
        cache_key: &str,
        available: (u16, u16),
        parts: &mbv_queue::PlaybackTitleParts,
        logo_cache_key: Option<&str>,
        item_kind: &str,
    ) -> Option<String> {
        let Some(entry) = self.app.cached_image(cache_key) else {
            self.app
                .title_log_gate
                .log_decision(cache_key, item_kind, "NoBaseEntry", None);
            return None;
        };
        let source_dimensions = entry.dimensions();
        let Some(size) = title_protocol_size(self, cache_key, available) else {
            self.app.title_log_gate.log_decision(
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
        let ready_logo_key = self
            .image_protocols
            .ready_logo_key(self.app.image_cache(), logo_cache_key);
        let key = mbv_images::title_overlay::title_overlay_cache_key(
            cache_key,
            cols,
            rows,
            overlay_text,
            ready_logo_key.as_deref(),
        );
        if !self.app.image_cached(&key)
            && !build_title_overlay_variant(
                self,
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
        if title_protocol_size(self, &key, available).is_none() {
            self.app.title_log_gate.log_decision(
                cache_key,
                item_kind,
                "VariantNotReady",
                source_dimensions,
            );
            return None;
        }
        Some(key)
    }

    /// Ensure the hero cover-fit protocol for `cache_key` matches
    /// `box_cells` (task 5.10, design D5): the protocol is rebuilt from the
    /// decoded source through `cover_fill_hero_box` at the box's pixel size
    /// whenever the box changed (resize, split drag, Workspace shrink — the
    /// next sync pass's "re-encode request keyed by the new box size"), and
    /// the painters show the placeholder for at most that one frame.
    /// Returns whether a ready protocol is available.
    ///
    /// `resizing` is the resize drag hold (`Model::refresh_resize_drag_hold`,
    /// invariant 20): while it is on, a changed box reports not ready (the
    /// painter shows the placeholder) instead of re-encoding inline.
    pub(in crate::app) fn ensure_hero_cover_protocol(
        &mut self,
        cache_key: &str,
        box_cells: (u16, u16),
        logo_cache_key: Option<&str>,
        resizing: bool,
    ) -> bool {
        let Some(entry) = self.app.cached_image(cache_key) else {
            return false;
        };
        let Some(source) = entry.img.clone() else {
            return false;
        };
        let desired_logo_key = self
            .image_protocols
            .ready_logo_key(self.app.image_cache(), logo_cache_key);
        if entry.cover_box == Some(box_cells)
            && entry.applied_logo_key == desired_logo_key
            && self
                .image_protocols
                .has_any(self.app.image_cache(), cache_key)
        {
            return true;
        }
        if resizing {
            // Hold, never rebuild: the settled box re-encodes once the hold
            // lifts. A box that happens to match the encoding still paints.
            return false;
        }
        let Some((picker, suffix)) =
            picker_and_suffix(self).map(|(picker, suffix)| (picker.clone(), suffix))
        else {
            return false;
        };
        let (px_w, px_h) = self.hero_box_pixels(box_cells.0, box_cells.1);
        let cropped = self.image_protocols.decorate_with_logo(
            self.app.image_cache(),
            cover_fill_hero_box(&source, px_w, px_h),
            desired_logo_key.as_deref(),
        );
        let bare_key = cache_key.to_string();
        let proto = self
            .image_protocols
            .build_protocol(&bare_key, suffix, &picker, cropped);
        self.image_protocols.clear_key(cache_key);
        self.image_protocols
            .insert_protocol(self.app.image_cache(), cache_key, suffix, proto);
        if let Some(entry) = self.app.images.image_mut(cache_key) {
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
        paint: &mbv_ui_model::library_hero::PanelHeroImagePaint,
    ) {
        if paint.area.width == 0 || paint.area.height == 0 {
            return;
        }
        if let Some(state) = self.cached_image_protocol_mut(&paint.cache_key) {
            type SImg = ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
            let area = mbv_render::layout::presentation_to_rect(paint.area);
            let avail = ratatui::layout::Size {
                width: area.width,
                height: area.height,
            };
            if let Some(actual) =
                state.size_for(ratatui_image::Resize::Scale(Some(RENDER_FILTER)), avail)
            {
                let img_rect = ratatui::layout::Rect {
                    x: area.x + area.width.saturating_sub(actual.width) / 2,
                    y: area.y,
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
            ratatui::widgets::Block::default().style(
                ratatui::style::Style::default().bg(palette::surface_colors(
                    palette::Surface::ArtworkLoadingPlaceholder,
                    false,
                )
                .fill
                .to_ratatui()),
            ),
            mbv_render::layout::presentation_to_rect(paint.area),
        );
    }
}
