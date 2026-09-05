use crate::app::components::browser_narrow::NarrowInlineHero;
use crate::app::images::{series_image_cache_key, CachedImage};
use crate::app::render::components::hero_model::SERIES_LANDSCAPE_IMAGE_TYPES;
use crate::app::render::make_movie_app;
use crate::app::App;

/// The narrow Series hero's own image-loading flag — the single piece of state
/// `detail_series_view` turns into a placeholder (see `detail_series_tests`).
fn narrow_series_image_loading(app: &mut App) -> bool {
    let Some(NarrowInlineHero::Series { image_loading, .. }) =
        app.narrow_browse_extras(0, 0).inline_hero
    else {
        panic!("a Series selection must resolve the narrow Series inline hero");
    };
    image_loading
}

/// Task 2.2: the narrow lookup reads the `Primary` chain its painter requests,
/// so the placeholder clears exactly when the painted entry lands — and never
/// rides on Wide's Thumb-first entry, whose bytes differ.
#[test]
fn narrow_series_inline_hero_reads_the_primary_chain_it_paints() {
    let mut app = make_movie_app();
    app.libs[0].library.collection_type = "tvshows".into();
    for item in &mut app.libs[0].nav_stack[0].items {
        item.item_type = "Series".into();
    }
    app.image_protocol_enabled = true;

    assert!(
        narrow_series_image_loading(&mut app),
        "an uncached narrow Series selection must reserve the placeholder"
    );

    app.card_image_states.insert(
        series_image_cache_key("movie-focused", SERIES_LANDSCAPE_IMAGE_TYPES),
        CachedImage::empty(),
    );
    assert!(
        narrow_series_image_loading(&mut app),
        "Wide's Thumb-first entry must not satisfy the narrow lookup"
    );

    app.card_image_states.insert(
        series_image_cache_key("movie-focused", &["Primary"]),
        CachedImage::empty(),
    );
    assert!(
        !narrow_series_image_loading(&mut app),
        "the painted Primary entry must clear the narrow placeholder"
    );
}

/// Images off must never claim a fetch is in flight, cached or not.
#[test]
fn narrow_series_inline_hero_reports_no_loading_when_images_are_off() {
    let mut app = make_movie_app();
    app.libs[0].library.collection_type = "tvshows".into();
    for item in &mut app.libs[0].nav_stack[0].items {
        item.item_type = "Series".into();
    }
    app.image_protocol_enabled = false;

    assert!(!narrow_series_image_loading(&mut app));
}
