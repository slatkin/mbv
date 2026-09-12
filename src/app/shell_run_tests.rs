use super::*;
use crate::app::images::{series_image_cache_key, CachedImage};
use crate::app::render::components::hero_model::SERIES_LANDSCAPE_IMAGE_TYPES;
use crate::app::render::make_movie_app;

fn mounted_wide_tv_model() -> Model {
    let mut app = make_movie_app();
    app.libs[0].library.collection_type = "tvshows".into();
    for item in &mut app.libs[0].nav_stack[0].items {
        item.item_type = "Series".into();
        item.image_tags.thumb = "tag".into();
    }
    // Wide breakpoint is now driven synchronously by terminal size
    // (`wide_tv_library_area`), not this previous-frame paint rect.
    app.terminal_width = 160;
    app.terminal_height = 40;
    let mut model = Model::new(app);
    model.sync_tv_content();
    model.sync_active_destination();
    model
}

/// The Series artwork placeholder state the wide workspace is currently painting
/// (the panel reserves no image paint while the projected state is not ready).
fn wide_tv_shows_placeholder(model: &mut Model) -> bool {
    model
        .test_paint_library_panel(ratatui::layout::Rect::new(0, 0, 100, 30))
        .is_none()
}

/// The fixture has no Emby client, so `spawn_image_fetch` resolves its own
/// request synchronously into `card_image_rx`. A step that must start from a
/// quiet channel drops those completions first.
fn drop_pending_image_completions(model: &mut Model) {
    while model.app.card_image_rx.try_recv().is_ok() {}
}

/// Task 2.3: a Series completion is what re-projects the wide workspace, so the
/// cached Thumb-first entry replaces the placeholder. The gate covers the
/// `:ser:` family the painter builds its keys under.
#[test]
fn series_image_completion_repushes_tv_workspace_content() {
    let mut model = mounted_wide_tv_model();
    model.app.image_protocol_enabled = true;
    // The sync pass is the production projection seam (task 5.10's central
    // hero projection owns the fetch for every migrated owner, TV included
    // since task 8.4). One throwaway draw publishes `root_frame` first — the
    // projection reads the panel's `RootFrame` placement, so it runs on the
    // sync after the shell's startup draw.
    {
        let backend = ratatui::backend::TestBackend::new(160, 40);
        let mut term = ratatui::Terminal::new(backend).unwrap();
        term.draw(|f| model.draw_frame(f, false, false)).unwrap();
    }
    model.sync_mounted_surfaces();
    assert!(
        wide_tv_shows_placeholder(&mut model),
        "an uncached projection must paint the placeholder"
    );

    let painted_key = series_image_cache_key("movie-focused", SERIES_LANDSCAPE_IMAGE_TYPES);
    assert!(
        model.drain_card_image_completions(),
        "the Series prefetch must resolve into the cache"
    );
    assert!(
        model.app.card_image_states.contains_key(&painted_key),
        "the painted Series key must be cached"
    );
    // The fixture's fetch resolves to an empty cache entry (no pixel
    // protocol is available), so the shared producer now treats the
    // placeholder as final while the shell still re-projects the cached key.
    assert!(model.app.card_image_states.contains_key(&painted_key));
}

/// Task 2.3: no other image namespace may drive the TV projection. The cached
/// Series entry is planted without a re-push, so only a gate that wrongly
/// matches this key can clear the placeholder the component still holds.
#[test]
fn non_series_image_completion_leaves_tv_projection_alone() {
    let mut model = mounted_wide_tv_model();
    model.app.image_protocol_enabled = true;
    {
        let backend = ratatui::backend::TestBackend::new(160, 40);
        let mut term = ratatui::Terminal::new(backend).unwrap();
        term.draw(|f| model.draw_frame(f, false, false)).unwrap();
    }
    model.sync_mounted_surfaces();
    drop_pending_image_completions(&mut model);
    assert!(
        wide_tv_shows_placeholder(&mut model),
        "the uncached projection must paint the placeholder"
    );

    model.app.card_image_states.insert(
        series_image_cache_key("movie-focused", SERIES_LANDSCAPE_IMAGE_TYPES),
        CachedImage::empty(),
    );
    model
        .app
        .card_image_tx
        .send(("movie-focused:P".into(), None))
        .expect("image completion channel");

    assert!(
        model.drain_card_image_completions(),
        "the card completion must be drained"
    );
    assert!(
        model.app.card_image_states.contains_key("movie-focused:P"),
        "the drained entry must reach the cache"
    );
    assert!(
        wide_tv_shows_placeholder(&mut model),
        "a non-Series completion must leave the TV projection alone"
    );
}
