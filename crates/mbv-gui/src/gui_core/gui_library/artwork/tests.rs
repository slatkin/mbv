//! The Hero backdrop's source choice (gui-hero-design-polish design D4): the
//! tiny Backdrop image wins when it landed, the Hero's own art stands in when
//! the item has no Backdrop image, and nothing is chosen while neither is
//! cached.

use mbv_client_core::make_app_stub;
use mbv_images::CachedImage;

use super::backdrop_source;

const ART_KEY: &str = "item-1:P";

fn pixels() -> image::DynamicImage {
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        8,
        8,
        image::Rgba([20, 40, 60, 255]),
    ))
}

#[test]
fn a_landed_backdrop_image_is_the_source() {
    let mut app = make_app_stub();
    let backdrop_key = mbv_images::backdrop_cache_key(ART_KEY);
    app.seed_card_image_for_test(backdrop_key.clone(), pixels());
    app.seed_card_image_for_test(ART_KEY.to_owned(), pixels());

    let (key, _source) = backdrop_source(&app, ART_KEY).expect("a source");

    assert_eq!(key, backdrop_key);
}

#[test]
fn an_item_with_no_backdrop_image_falls_back_to_the_hero_art() {
    let mut app = make_app_stub();
    app.insert_cached_image_for_test(
        &mbv_images::backdrop_cache_key(ART_KEY),
        CachedImage::empty(),
    );
    app.seed_card_image_for_test(ART_KEY.to_owned(), pixels());

    let (key, _source) = backdrop_source(&app, ART_KEY).expect("a source");

    assert_eq!(key, ART_KEY);
}

#[test]
fn an_uncached_item_has_no_source() {
    let app = make_app_stub();

    assert!(backdrop_source(&app, ART_KEY).is_none());
}
