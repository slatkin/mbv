use super::super::App;
use crate::app::tests::make_app_stub;
use mbv_images::CachedImage;
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
    let mut projection = mbv_ui_model::playback::QueueCardProjection::default();
    app.ensure_title_overlay_protocol(
        cache_key,
        available,
        parts,
        logo_cache_key,
        &mut projection,
        "Movie",
    )
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

#[test]
fn logo_arrival_builds_one_new_overlay_variant() {
    let mut app = app_with_base();
    app.cached_image_protocol_mut(BASE_KEY)
        .expect("base protocol is available");
    let baseline = build_count(&app);
    let available = ratatui::layout::Size {
        width: 8,
        height: 4,
    };
    let parts = title_parts("a title");

    let text_variant =
        ensure_title_overlay_protocol(&mut app, BASE_KEY, available, &parts, Some(LOGO_KEY))
            .expect("the text variant builds while the logo is absent");
    assert_eq!(build_count(&app), baseline + 1);

    app.images
        .insert_image(LOGO_KEY.to_owned(), cached(Some(image(2, 1))));
    let logo_variant =
        ensure_title_overlay_protocol(&mut app, BASE_KEY, available, &parts, Some(LOGO_KEY))
            .expect("a ready logo builds the logo variant");
    assert_ne!(logo_variant, text_variant);
    assert_eq!(
        build_count(&app),
        baseline + 2,
        "logo arrival builds one new variant"
    );

    assert_eq!(
        ensure_title_overlay_protocol(&mut app, BASE_KEY, available, &parts, Some(LOGO_KEY)),
        Some(logo_variant)
    );
    assert_eq!(
        build_count(&app),
        baseline + 2,
        "a ready logo is reused on later ticks"
    );
}

#[test]
fn absent_or_failed_logo_keeps_the_text_variant_valid() {
    let mut app = app_with_base();
    app.cached_image_protocol_mut(BASE_KEY)
        .expect("base protocol is available");
    let baseline = build_count(&app);
    let available = ratatui::layout::Size {
        width: 8,
        height: 4,
    };
    let parts = title_parts("a title");

    let text_variant = ensure_title_overlay_protocol(&mut app, BASE_KEY, available, &parts, None)
        .expect("the text variant builds");
    assert_eq!(build_count(&app), baseline + 1);

    assert_eq!(
        ensure_title_overlay_protocol(&mut app, BASE_KEY, available, &parts, Some(LOGO_KEY)),
        Some(text_variant.clone()),
        "an absent logo reuses the text variant"
    );
    assert_eq!(build_count(&app), baseline + 1);

    app.images
        .insert_image(LOGO_KEY.to_owned(), CachedImage::empty());
    assert_eq!(
        ensure_title_overlay_protocol(&mut app, BASE_KEY, available, &parts, Some(LOGO_KEY)),
        Some(text_variant),
        "a failed logo reuses the text variant"
    );
    assert_eq!(build_count(&app), baseline + 1);
}

#[test]
fn arriving_logo_rebuilds_base_only_protocol_once() {
    let mut app = app_with_base();

    assert!(app.ensure_hero_cover_protocol(BASE_KEY, BOX, None));
    assert_eq!(build_count(&app), 1);

    app.images
        .insert_image(LOGO_KEY.to_owned(), cached(Some(image(2, 1))));
    assert!(app.ensure_hero_cover_protocol(BASE_KEY, BOX, Some(LOGO_KEY)));
    assert_eq!(build_count(&app), 2);
    assert_eq!(
        app.images
            .image(BASE_KEY)
            .and_then(|entry| entry.applied_logo_key.as_deref()),
        Some(LOGO_KEY)
    );

    assert!(app.ensure_hero_cover_protocol(BASE_KEY, BOX, Some(LOGO_KEY)));
    assert_eq!(build_count(&app), 2);
}

#[test]
fn title_overlay_builds_only_for_track_or_suffix_changes() {
    let mut app = app_with_base();
    app.dim_backdrop_active = true;
    app.cached_image_protocol_mut(BASE_KEY)
        .expect("base protocol is available under the dimmed suffix");
    app.dim_backdrop_active = false;
    app.cached_image_protocol_mut(BASE_KEY)
        .expect("base protocol is available under the configured suffix");
    let baseline = build_count(&app);
    let parts = |title: &str| mbv_queue::PlaybackTitleParts {
        title: mbv_queue::PlaybackTitlePart {
            role: mbv_queue::PlaybackTitlePartRole::Title,
            text: title.to_owned(),
        },
        context: None,
    };
    let available = ratatui::layout::Size {
        width: 8,
        height: 4,
    };

    let base_image = app.images.image(BASE_KEY).unwrap().img.clone();
    let first = ensure_title_overlay_protocol(&mut app, BASE_KEY, available, &parts("first"), None)
        .expect("base protocol has a measurable size");
    assert_eq!(build_count(&app), baseline + 1);

    assert_eq!(
        ensure_title_overlay_protocol(&mut app, BASE_KEY, available, &parts("first"), None),
        Some(first.clone())
    );
    assert_eq!(
        build_count(&app),
        baseline + 1,
        "playback ticks reuse both protocols"
    );

    let changed =
        ensure_title_overlay_protocol(&mut app, BASE_KEY, available, &parts("second"), None)
            .expect("changed title builds a distinct variant");
    assert_ne!(first, changed);
    assert!(!app.images.is_cached(&first));
    assert_eq!(
        build_count(&app),
        baseline + 2,
        "a track change builds one protocol"
    );
    assert_eq!(app.images.image(BASE_KEY).unwrap().img, base_image);
    let changed_composed = app.images.image(&changed).unwrap().img.clone();

    app.images.image_mut(BASE_KEY).unwrap().img = None;
    app.dim_backdrop_active = true;
    assert_eq!(
        ensure_title_overlay_protocol(&mut app, BASE_KEY, available, &parts("second"), None),
        Some(changed.clone())
    );
    assert_eq!(
        build_count(&app),
        baseline + 3,
        "suffix flip re-encodes without recomposing"
    );
    assert_eq!(app.images.image(&changed).unwrap().img, changed_composed);
    assert_eq!(app.images.image(BASE_KEY).unwrap().img, None);
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

#[test]
fn failed_or_absent_logo_keeps_base_only_protocol_valid() {
    let mut absent = app_with_base();
    assert!(absent.ensure_hero_cover_protocol(BASE_KEY, BOX, None));
    assert!(absent.ensure_hero_cover_protocol(BASE_KEY, BOX, Some(LOGO_KEY)));
    assert_eq!(build_count(&absent), 1);
    assert!(
        absent
            .images
            .image(BASE_KEY)
            .is_some_and(|entry| entry.applied_logo_key.is_none())
    );

    let mut failed = app_with_base();
    failed
        .images
        .insert_image(LOGO_KEY.to_owned(), CachedImage::empty());
    assert!(failed.ensure_hero_cover_protocol(BASE_KEY, BOX, Some(LOGO_KEY)));
    assert!(failed.ensure_hero_cover_protocol(BASE_KEY, BOX, Some(LOGO_KEY)));
    assert_eq!(build_count(&failed), 1);
    assert!(
        failed
            .images
            .image(BASE_KEY)
            .is_some_and(|entry| entry.applied_logo_key.is_none())
    );
}

#[test]
fn title_overlay_title_row_resolves_to_text_emphasis() {
    let colours = super::title_overlay_colours();

    assert_eq!(colours.title, super::color_rgb(mbv_theme::TEXT_EMPHASIS));
    assert_eq!(
        colours.context,
        super::color_rgb(mbv_theme::PLAYBACK_CONTEXT_FG)
    );
}
