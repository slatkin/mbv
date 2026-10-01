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
    let key = card_cache_key(
        app.playback_queue()
            .emby_item_at(0)
            .expect("the queue holds an Emby item"),
    );
    app.images.insert_image(key, cached_colour_image(4, 2));
    app
}

/// User-reported regression: every distinct fitted width during a queue-column
/// resize drag composed a Lanczos3 title overlay synchronously on the tick
/// thread. The gate must build no variant while the drag is active and compose
/// exactly one once it ends.
#[test]
fn column_resize_drag_builds_no_overlay_variant_until_it_ends() {
    let mut app = overlay_app();
    let base_key = card_cache_key(
        app.playback_queue()
            .emby_item_at(0)
            .expect("the queue holds an Emby item"),
    );
    app.cached_image_protocol_mut(&base_key)
        .expect("the base card protocol is available");
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

    app.refresh_queue_card_image(false);
    assert_eq!(
        app.images.image_protocol_builds(),
        baseline + 1,
        "the drag's final width composes the overlay once"
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
