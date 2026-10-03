//! Queue artwork ownership: movies fetch landscape art, episodes never show
//! series art; overlay composition stays off the tick thread during a drag.

use super::{card_cache_key, card_image_types};
use crate::app::App;
use crate::app::tests::render_fixtures::make_queue_app;
use mbv_images::CachedImage;
use ratatui_image::picker::{Picker, ProtocolType};

/// Guards 55c37ea71 (landscape key for movies and episodes) and its fix
/// e39b5a8f4 (episode keeps its own still, Movie-only key): a Movie takes the
/// landscape chain even with only a poster tag, while an episode carrying
/// series tags keeps its own poster-first chain so its card never shows
/// series art.
#[rstest::rstest]
#[case::movie_with_primary_tag(
    "Movie",
    Some("primary-tag"),
    None,
    "item:QB",
    &["Backdrop", "Primary"]
)]
#[case::episode_with_series_tags(
    "Episode",
    None,
    Some("series-thumb-tag"),
    "item:P",
    &["Primary", "Thumb", "Backdrop", "Logo"]
)]
fn artwork_ownership_keeps_movies_landscape_and_episodes_episode_owned(
    #[case] item_type: &str,
    #[case] primary_tag: Option<&str>,
    #[case] series_thumb_tag: Option<&str>,
    #[case] expected_key: &str,
    #[case] expected_chain: &[&str],
) {
    let mut item = mbv_emby_model::test_support::make_item("Item", item_type);
    item.id = "item".into();
    if let Some(tag) = primary_tag {
        item.image_tags.primary = tag.into();
    }
    if let Some(tag) = series_thumb_tag {
        item.image_tags.series_thumb = tag.into();
    }
    assert_eq!(card_cache_key(&item), expected_key);
    assert_eq!(card_image_types(&item), expected_chain);
}

fn cached_colour_image(width: u32, height: u32) -> CachedImage {
    CachedImage {
        img: Some(image::DynamicImage::ImageRgba8(
            image::RgbaImage::from_pixel(width, height, image::Rgba([20, 40, 60, 255])),
        )),
        protocols: std::collections::HashMap::new(),
        cover_box: None,
        applied_logo_key: None,
    }
}

/// An active-playing queue app with the card protocol and one base bitmap, so
/// the title-site decision can reach the overlay-variant build.
fn overlay_app() -> App {
    let mut app = make_queue_app(1);
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 0;
        status.queue_len = 1;
    };
    let mut picker = Picker::halfblocks();
    picker.set_protocol_type(ProtocolType::Kitty);
    app.images.configure_protocol(None, true);
    app.images
        .set_image_pickers_for_test(picker, Picker::halfblocks());
    // (height, width): the card box the overlay is composed for.
    app.images.record_card_size(4, 8);
    app
}

/// `overlay_app` with the base bitmap already inserted, so the overlay
/// variant can compose this sync.
fn overlay_app_with_base_image(app: App) -> (App, String) {
    let key = card_cache_key(
        app.playback_queue()
            .emby_item_at(0)
            .expect("the queue holds an Emby item"),
    );
    let mut app = app;
    app.images
        .insert_image(key.clone(), cached_colour_image(4, 2));
    (app, key)
}

/// User-reported regression: every distinct fitted width during a queue-column
/// resize drag composed a Lanczos3 title overlay synchronously on the tick
/// thread. The gate must build no variant while the drag is active and compose
/// exactly one once it ends — and the drag must not resurrect the header:
/// the site carries over through the transient gate (2026-10-03).
#[test]
fn column_resize_drag_builds_no_overlay_variant_until_it_ends() {
    let (mut app, base_key) = overlay_app_with_base_image(overlay_app());
    app.cached_image_protocol_mut(&base_key)
        .expect("the base card protocol is available");

    // Warm sync: eligibility passes and the site is decided before any paint.
    app.refresh_queue_card_image(false);
    assert_eq!(
        app.queue_card_projection.title_site,
        mbv_ui_model::playback::NowPlayingTitleSite::Artwork,
        "the warm sync decides the site before the panel first displays"
    );
    let baseline = app.images.image_protocol_builds();

    app.refresh_queue_card_image(true);
    assert_eq!(
        app.images.image_protocol_builds(),
        baseline,
        "no overlay variant is composed while the column drag is active"
    );
    assert_eq!(
        app.queue_card_projection.cache_key.as_deref(),
        Some(base_key.as_str()),
        "the card paints plain base art during the drag"
    );
    assert_eq!(
        app.queue_card_projection.title_site,
        mbv_ui_model::playback::NowPlayingTitleSite::Artwork,
        "the resize drag must not resurrect the header: the site carries over"
    );

    app.refresh_queue_card_image(false);
    assert_eq!(
        app.images.image_protocol_builds(),
        baseline,
        "the drag composes nothing and the warm sync's variant is reused at the unchanged size"
    );
    assert!(
        app.queue_card_projection
            .cache_key
            .as_deref()
            .is_some_and(|key| key.contains(":t:")),
        "the ended drag projects the overlay variant: {:?}",
        app.queue_card_projection.cache_key
    );
}

/// The flash fix's core: the site is decided while the base-art fetch is
/// still in flight. A missing cache *entry* means the art will arrive and
/// carry the title — deciding only after its arrival is what made the
/// header flash on every playback start (2026-10-03).
#[test]
fn the_site_is_decided_while_the_art_fetch_is_pending() {
    let mut app = overlay_app();
    assert!(
        app.queue_card_projection
            .cache_key
            .as_deref()
            .and_then(|key| app.images.image(key))
            .is_none(),
        "the test fixture starts with the art fetch pending"
    );
    app.refresh_queue_card_image(false);
    assert_eq!(
        app.queue_card_projection.title_site,
        mbv_ui_model::playback::NowPlayingTitleSite::Artwork,
        "a pending art fetch must not hold the header as the title's home"
    );
}

/// The transient/transient split's other half: a mode gate is a stable
/// presentation fact, so it forces the header back as the title's home even
/// from a carried-over `Artwork` site.
#[test]
fn a_mode_gate_forces_the_header_back_from_the_artwork_site() {
    let (mut app, _base_key) = overlay_app_with_base_image(overlay_app());
    app.refresh_queue_card_image(false);
    assert_eq!(
        app.queue_card_projection.title_site,
        mbv_ui_model::playback::NowPlayingTitleSite::Artwork
    );

    // The visualizer replaces the artwork: the artwork can no longer carry
    // the title, so the header is its home again.
    app.visualizer_enabled = true;
    app.refresh_queue_card_image(false);
    assert_eq!(
        app.queue_card_projection.title_site,
        mbv_ui_model::playback::NowPlayingTitleSite::Header,
        "a mode gate forces the header back from the artwork site"
    );
}
