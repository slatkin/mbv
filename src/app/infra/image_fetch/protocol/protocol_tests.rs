use super::super::App;
use crate::app::tests::make_app_stub;
use mbv_images::CachedImage;
use ratatui_image::picker::{Picker, ProtocolType};

const BASE_KEY: &str = "hero-base";
const LOGO_KEY: &str = "hero-logo";

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
    let mut picker = Picker::halfblocks();
    picker.set_protocol_type(ProtocolType::Kitty);
    app.images.configure_protocol(None, true);
    app.images
        .set_image_pickers_for_test(picker, Picker::halfblocks());
    app.images
        .insert_image(BASE_KEY.to_owned(), cached(Some(image(4, 2))));
    app
}

fn build_count(app: &App) -> u32 {
    app.images.image_protocol_builds()
}

fn ensure_title_overlay_protocol(
    app: &mut App,
    cache_key: &str,
    available: ratatui::layout::Size,
    parts: &mbv_queue::PlaybackTitleParts,
    logo_cache_key: Option<&str>,
) -> Option<String> {
    app.ensure_title_overlay_protocol(cache_key, available, parts, logo_cache_key, "Movie")
}

fn title_parts(title: &str) -> mbv_queue::PlaybackTitleParts {
    mbv_queue::PlaybackTitleParts {
        title: mbv_queue::PlaybackTitlePart {
            role: mbv_queue::PlaybackTitlePartRole::Title,
            text: title.to_owned(),
        },
        context: None,
    }
}

/// Overlay variant identity over one lifecycle: the text variant builds once
/// while the logo is absent or failed and is reused on ticks, logo arrival
/// builds exactly one new variant, a track change builds one more and evicts
/// the old one, and a suffix flip re-encodes the variant without recomposing
/// it. Catches stale titles after track changes and recompose storms.
#[test]
fn title_overlay_variant_identity_rebuilds_once_per_change_and_reuses_on_ticks() {
    let mut app = app_with_base();
    app.dim_backdrop_active = true;
    app.cached_image_protocol_mut(BASE_KEY)
        .expect("base protocol is available under the dimmed suffix");
    app.dim_backdrop_active = false;
    app.cached_image_protocol_mut(BASE_KEY)
        .expect("base protocol is available under the configured suffix");
    let baseline = build_count(&app);
    let available = ratatui::layout::Size {
        width: 8,
        height: 4,
    };
    let ensure = |app: &mut App, title: &str| {
        ensure_title_overlay_protocol(
            app,
            BASE_KEY,
            available,
            &title_parts(title),
            Some(LOGO_KEY),
        )
    };

    let text_variant =
        ensure(&mut app, "a title").expect("the text variant builds while the logo is absent");
    assert_eq!(build_count(&app), baseline + 1);
    assert_eq!(ensure(&mut app, "a title"), Some(text_variant.clone()));
    assert_eq!(
        build_count(&app),
        baseline + 1,
        "playback ticks reuse the text variant"
    );

    app.images
        .insert_image(LOGO_KEY.to_owned(), CachedImage::empty());
    assert_eq!(
        ensure(&mut app, "a title"),
        Some(text_variant.clone()),
        "a failed logo reuses the text variant"
    );
    assert_eq!(build_count(&app), baseline + 1);

    app.images
        .insert_image(LOGO_KEY.to_owned(), cached(Some(image(2, 1))));
    let logo_variant = ensure(&mut app, "a title").expect("a ready logo builds the logo variant");
    assert_ne!(logo_variant, text_variant);
    assert_eq!(
        build_count(&app),
        baseline + 2,
        "logo arrival builds one new variant"
    );
    assert_eq!(ensure(&mut app, "a title"), Some(logo_variant.clone()));
    assert_eq!(
        build_count(&app),
        baseline + 2,
        "a ready logo is reused on later ticks"
    );

    let changed = ensure(&mut app, "second").expect("a track change builds a distinct variant");
    assert_ne!(changed, logo_variant);
    assert!(!app.images.is_cached(&logo_variant));
    assert_eq!(
        build_count(&app),
        baseline + 3,
        "a track change builds one protocol"
    );

    let changed_composed = app.images.image(&changed).unwrap().img.clone();
    app.images.image_mut(BASE_KEY).unwrap().img = None;
    app.dim_backdrop_active = true;
    assert_eq!(ensure(&mut app, "second"), Some(changed.clone()));
    assert_eq!(
        build_count(&app),
        baseline + 3,
        "the dimmed backdrop's sizing keeps the configured variant key and builds nothing"
    );
    // The suffix flip's re-encode lives at the paint path: under the backdrop
    // the overlay is measured on the configured grid (stable key above), and
    // the slot painter lazily re-encodes the variant for the fallback suffix
    // when it resolves the protocol to paint.
    app.cached_image_protocol_mut(&changed)
        .expect("the painter resolves the variant under the fallback suffix");
    assert_eq!(
        build_count(&app),
        baseline + 4,
        "suffix flip re-encodes without recomposing"
    );
    assert_eq!(app.images.image(&changed).unwrap().img, changed_composed);
}

#[test]
fn building_title_overlay_leaves_shared_plain_card_bitmap_unchanged() {
    // Regression guard for the shared-key flash documented by
    // `audiobookshelf_hero_cover_cache_key`: the `{id}:P` bitmap stays plain.
    let mut app = app_with_base();
    let plain_key = "item:P";
    app.images
        .insert_image(plain_key.to_owned(), cached(Some(image(4, 2))));
    let plain_before = app.images.image(plain_key).unwrap().img.clone();
    let parts = mbv_queue::PlaybackTitleParts {
        title: mbv_queue::PlaybackTitlePart {
            role: mbv_queue::PlaybackTitlePartRole::Title,
            text: "a title".to_owned(),
        },
        context: None,
    };

    let variant = ensure_title_overlay_protocol(
        &mut app,
        plain_key,
        ratatui::layout::Size {
            width: 8,
            height: 4,
        },
        &parts,
        None,
    )
    .expect("measurable plain art builds an overlay variant");

    assert_ne!(variant, plain_key);
    assert_eq!(app.images.image(plain_key).unwrap().img, plain_before);
}
