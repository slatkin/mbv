use super::Model;
use crate::app::tests::make_app_stub;
use mbv_images::{CachedImage, SourceImage};
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
        source: None,
        cover_box: None,
        applied_logo_key: None,
    }
}

/// A cache entry that keeps its source, as every fetched entry does: the
/// store re-encodes a key's protocol from that source when it needs one, so a
/// test that inserts a bitmap with `img` alone still paints (invariant 38).
fn fetched(img: image::DynamicImage) -> CachedImage {
    CachedImage {
        source: SourceImage::sniff(mbv_images::QUEUE_CARD_PLACEHOLDER_BYTES.to_vec()),
        img: Some(img),
        cover_box: None,
        applied_logo_key: None,
    }
}

fn app_with_base() -> Model {
    app_with_picker(Picker::halfblocks())
}

/// The shell `Model` a test drives, over the stub `App`: the image protocol
/// store is the shell's own, so the pickers are installed on it and the card
/// box and the base entry are seeded through the core's test seams.
fn app_with_picker(mut picker: Picker) -> Model {
    let mut app = make_app_stub();
    picker.set_protocol_type(ProtocolType::Kitty);
    app.enable_image_protocol_for_test();
    let mut model = Model::new(app);
    crate::tui::install_image_pickers_for_test(&mut model, picker, Picker::halfblocks());
    // (height, width): the card box the overlay is composed for.
    model.app.record_card_size_for_test(4, 8);
    model
        .app
        .insert_cached_image_for_test(BASE_KEY, cached(Some(image(4, 2))));
    model
}

/// How many protocol encodings the shell's image store built so far: the
/// store is the shell `Model`'s, so the count is read from it.
fn build_count(model: &Model) -> u32 {
    model.image_protocols.protocol_builds()
}

fn ensure_title_overlay_protocol(
    model: &mut Model,
    cache_key: &str,
    available: (u16, u16),
    parts: &mbv_queue::PlaybackTitleParts,
    logo_cache_key: Option<&str>,
) -> Option<String> {
    model.ensure_title_overlay_protocol(cache_key, available, parts, logo_cache_key, "Movie")
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
    let mut model = app_with_base();
    model.app.set_dim_backdrop_active_for_test(true);
    model
        .cached_image_protocol_mut(BASE_KEY)
        .expect("base protocol is available under the dimmed suffix");
    model.app.set_dim_backdrop_active_for_test(false);
    model
        .cached_image_protocol_mut(BASE_KEY)
        .expect("base protocol is available under the configured suffix");
    let baseline = build_count(&model);
    let ensure = |model: &mut Model, title: &str| {
        ensure_title_overlay_protocol(model, BASE_KEY, (8, 4), &title_parts(title), Some(LOGO_KEY))
    };

    let text_variant =
        ensure(&mut model, "a title").expect("the text variant builds while the logo is absent");
    assert_eq!(build_count(&model), baseline + 1);
    assert_eq!(ensure(&mut model, "a title"), Some(text_variant.clone()));
    assert_eq!(
        build_count(&model),
        baseline + 1,
        "playback ticks reuse the text variant"
    );

    model
        .app
        .insert_cached_image_for_test(LOGO_KEY, CachedImage::empty());
    assert_eq!(
        ensure(&mut model, "a title"),
        Some(text_variant.clone()),
        "a failed logo reuses the text variant"
    );
    assert_eq!(build_count(&model), baseline + 1);

    model
        .app
        .insert_cached_image_for_test(LOGO_KEY, cached(Some(image(2, 1))));
    let logo_variant = ensure(&mut model, "a title").expect("a ready logo builds the logo variant");
    assert_ne!(logo_variant, text_variant);
    assert_eq!(
        build_count(&model),
        baseline + 2,
        "logo arrival builds one new variant"
    );
    assert_eq!(ensure(&mut model, "a title"), Some(logo_variant.clone()));
    assert_eq!(
        build_count(&model),
        baseline + 2,
        "a ready logo is reused on later ticks"
    );

    let changed = ensure(&mut model, "second").expect("a track change builds a distinct variant");
    assert_ne!(changed, logo_variant);
    assert!(model.app.cached_image_for_test(&logo_variant).is_none());
    assert_eq!(
        build_count(&model),
        baseline + 3,
        "a track change builds one protocol"
    );

    let changed_composed = model.app.cached_image_pixels_for_test(&changed);
    model.app.drop_cached_pixels_for_test(BASE_KEY);
    model.app.set_dim_backdrop_active_for_test(true);
    assert_eq!(ensure(&mut model, "second"), Some(changed.clone()));
    assert_eq!(
        build_count(&model),
        baseline + 3,
        "the dimmed backdrop's sizing keeps the configured variant key and builds nothing"
    );
    // The suffix flip's re-encode lives at the paint path: under the backdrop
    // the overlay is measured on the configured grid (stable key above), and
    // the slot painter lazily re-encodes the variant for the fallback suffix
    // when it resolves the protocol to paint.
    model
        .cached_image_protocol_mut(&changed)
        .expect("the painter resolves the variant under the fallback suffix");
    assert_eq!(
        build_count(&model),
        baseline + 4,
        "suffix flip re-encodes without recomposing"
    );
    assert_eq!(
        model.app.cached_image_pixels_for_test(&changed),
        changed_composed
    );
}

#[test]
fn building_title_overlay_leaves_shared_plain_card_bitmap_unchanged() {
    // Regression guard for the shared-key flash documented by
    // `audiobookshelf_hero_cover_cache_key`: the `{id}:P` bitmap stays plain.
    let mut model = app_with_base();
    let plain_key = "item:P";
    model
        .app
        .insert_cached_image_for_test(plain_key, fetched(image(4, 2)));
    let plain_before = model.app.cached_image_pixels_for_test(plain_key);
    let parts = mbv_queue::PlaybackTitleParts {
        title: mbv_queue::PlaybackTitlePart {
            role: mbv_queue::PlaybackTitlePartRole::Title,
            text: "a title".to_owned(),
        },
        context: None,
    };

    let variant = ensure_title_overlay_protocol(&mut model, plain_key, (32, 16), &parts, None)
        .expect("measurable plain art builds an overlay variant");

    assert_ne!(variant, plain_key);
    assert_eq!(
        model.app.cached_image_pixels_for_test(plain_key),
        plain_before
    );
}

/// Invariant 20 (user-reported 2026-10-09): a changed box under the resize
/// drag hold reports not ready and builds nothing; the settled box
/// re-encodes once after the hold lifts.
#[test]
fn resize_drag_holds_the_placeholder_and_reencodes_once_after_it_settles() {
    let mut model = app_with_base();
    assert!(model.ensure_hero_cover_protocol(BASE_KEY, (10, 5), None, false));
    let baseline = build_count(&model);

    assert!(
        !model.ensure_hero_cover_protocol(BASE_KEY, (13, 6), None, true),
        "the hold reports not ready so the drag shows the placeholder"
    );
    assert_eq!(build_count(&model), baseline, "the hold builds nothing");

    assert!(model.ensure_hero_cover_protocol(BASE_KEY, (13, 6), None, false));
    assert_eq!(
        build_count(&model),
        baseline + 1,
        "the settled box re-encodes once"
    );
    assert!(model.app.cached_cover_box_is_for_test(BASE_KEY, (13, 6)));
}

/// Invariant 20: a box that matches the kept encoding still paints during
/// the hold, so a drag back to the encoded size shows art.
#[test]
fn resize_drag_hold_paints_a_box_matching_the_kept_encoding() {
    let mut model = app_with_base();
    assert!(model.ensure_hero_cover_protocol(BASE_KEY, (10, 5), None, false));
    let baseline = build_count(&model);

    assert!(model.ensure_hero_cover_protocol(BASE_KEY, (10, 5), None, true));
    assert_eq!(build_count(&model), baseline);
}
