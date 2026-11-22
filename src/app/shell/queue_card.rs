//! The queue card's visual slot paint. Shell code — it builds ratatui values
//! and reads the shell's terminal height (design D3). The sync-pass decision
//! that feeds it builds no ratatui value and stays in the core
//! (`state::projection::card`).

use crate::app::App;
use mbv_images::QUEUE_CARD_PLACEHOLDER_KEY;
use mbv_render::components::card::{queue_card_reserved_rect, render_card_painting};
use mbv_tui_support::ToRatatui;
use ratatui::Frame;
use ratatui::layout::Rect;

fn render_card_visualizer(
    app: &mut App,
    f: &mut Frame,
    area: Rect,
    left_align: bool,
) -> (u16, u16, bool) {
    let rect = queue_card_reserved_rect(
        app.images.last_card_size(),
        app.terminal_height,
        area,
        left_align,
    );
    // The row is fixed (it paints the playback panel's band), so the
    // focus bit is not read; the literal keeps that visible at the site.
    let bg = mbv_theme::surface_colors(mbv_theme::Surface::QueueCardVisualizer, false)
        .fill
        .to_ratatui();
    super::visualizer::render_visualizer(app, f, rect, bg);
    (rect.height, rect.width, false)
}

/// Renders the Queue playback panel's visual slot into `area` and returns
/// `(rows_used, cols_used, image_loading)` (task 3.5, D10: the slot moved
/// from the base frame's `render_card` into the panel's paint path; the shell
/// helper resolves the projected slot plus the shell-resolved image protocol
/// handle — no fetch, no source resolution. The projection owns every fetch
/// (task 3.4, D9)).
pub(in crate::app) fn render_queue_playback_slot(
    app: &mut App,
    f: &mut Frame,
    area: Rect,
    left_align: bool,
) -> (u16, u16, bool) {
    let mut projection = app.queue_card_projection.clone();
    if projection.visualizer {
        return render_card_visualizer(app, f, area, left_align);
    }
    if !projection.images_enabled {
        let rect = queue_card_reserved_rect(
            app.images.last_card_size(),
            app.terminal_height,
            area,
            left_align,
        );
        return (rect.height, rect.width, false);
    }
    // The slot's key: the projected artwork key, or the bundled
    // placeholder when the fetch resolved empty or the slot is the
    // placeholder itself. Resolved from the image cache's authority —
    // a read, never a fetch.
    let artwork_key = projection.cache_key.clone().filter(|key| {
        app.images
            .image(key)
            .is_none_or(mbv_images::CachedImage::has_art)
    });
    let placeholder_slot = artwork_key.is_none();
    if placeholder_slot {
        app.ensure_placeholder_card_image();
        // The painter derives `placeholder_slot` from
        // `projection.cache_key`; hand it the resolved-empty fact (a
        // `Some` key whose fetch resolved empty means the placeholder)
        // so the painter's fallback reservation matches the adapter's
        // derivation (review of tasks 3.1-3.4).
        projection.cache_key = None;
    }
    let key = artwork_key.as_deref().unwrap_or(QUEUE_CARD_PLACEHOLDER_KEY);
    let loading =
        !placeholder_slot && (app.images.is_loading(key) || app.images.is_fetch_interrupted(key));
    let last_card = app.images.last_card_size();
    let terminal_height = app.terminal_height;
    let image = app.cached_image_protocol_mut(key);
    let (height, width, loading, painted) = render_card_painting(
        f,
        area,
        left_align,
        &projection,
        loading,
        image,
        last_card,
        terminal_height,
    );
    let fallback_key = projection.plain_cache_key.as_deref().filter(|key| {
        app.images
            .image(key)
            .is_some_and(mbv_images::CachedImage::has_art)
    });
    let mut plain_fallback_painted = false;
    let mut out = (height, width, loading);
    if !painted && let Some(fallback_key) = fallback_key {
        let fallback_loading = app.images.is_loading(fallback_key);
        let fallback_image = app.cached_image_protocol_mut(fallback_key);
        projection.cache_key = Some(fallback_key.to_owned());
        let (height, width, loading, fallback_painted) = render_card_painting(
            f,
            area,
            left_align,
            &projection,
            fallback_loading,
            fallback_image,
            last_card,
            terminal_height,
        );
        plain_fallback_painted = fallback_painted;
        out = (height, width, loading);
    }
    let (height, width, loading) = out;
    // A dimmed backdrop paints through the halfblock fallback (#451),
    // whose hardcoded font grid measures cells differently than the
    // configured protocol's detected font. Recording that paint's size
    // would move the geometry checkpoint — resizing the reservation and
    // pushing the queue rows below the slot — every time a modal or
    // context menu opens, and back when it closes. The checkpoint stays
    // on the configured protocol's last paint; closing the modal
    // repaints the warm configured protocol at that same size.
    if !app.dim_halfblock_forced() {
        app.images.record_card_size(height, width);
    }
    if painted
        && let Some(key) = artwork_key
            .as_deref()
            .filter(|key| key.contains(mbv_images::title_overlay::DERIVED_SEP))
    {
        app.title_log_gate()
            .log_paint(key, "overlay_recorded", "OverlayPainted");
    } else if plain_fallback_painted {
        let key = artwork_key.as_deref().unwrap_or("<none>");
        let reason = app.title_log_decision_reason().unwrap_or("OverlayNotReady");
        app.title_log_gate()
            .log_paint(key, "plain_fallback", reason);
    }
    (height, width, loading)
}

#[cfg(test)]
mod queue_slot_tests;
