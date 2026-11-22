//! The queue card's title overlay under the dimmed backdrop (#451): the overlay
//! stays measured on the configured protocol's grid, so opening a dimming
//! overlay neither re-encodes nor re-keys the variant.

use crate::app::App;
use crate::app::tests::make_app_stub;
use mbv_images::CachedImage;
use mbv_ui_model::playback::{NowPlayingTitleSite, QueueCardProjection};
use ratatui_image::picker::{Picker, ProtocolType};

const CARD_KEY: &str = "card:P";

fn cached(img: Option<image::DynamicImage>) -> CachedImage {
    CachedImage {
        img,
        source: None,
        cover_box: None,
        applied_logo_key: None,
    }
}

/// An app with one decoded card image and a projected queue slot for it,
/// composed as the queue slot's paint leaves it.
fn app_with_card_image() -> App {
    let mut app = make_app_stub();
    let mut configured = Picker::halfblocks();
    configured.set_protocol_type(ProtocolType::Kitty);
    app.images.configure_protocol(None, true);
    app.image_protocols
        .set_image_pickers_for_test(configured, Picker::halfblocks());
    app.images.insert_image(
        CARD_KEY.to_owned(),
        cached(Some(image::DynamicImage::ImageRgba8(
            image::RgbaImage::from_pixel(4, 2, image::Rgba([20, 40, 60, 255])),
        ))),
    );
    app.queue_card_projection = QueueCardProjection {
        cache_key: Some(CARD_KEY.to_owned()),
        plain_cache_key: None,
        images_enabled: true,
        visualizer: false,
        title_site: NowPlayingTitleSite::Header,
    };
    app
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

/// Opening a dimming overlay must not disturb the title overlay pipeline:
/// the overlay stays sized on the configured protocol's grid, so the same
/// variant key survives the backdrop opening and closing — no header flash,
/// and the sizing path re-encodes nothing. With the fallback sizing the
/// measurements, each `ensure` call would lazily build a fallback protocol
/// for the base and the variant (two extra builds) even though the key
/// happens to coincide on a shared font grid.
#[test]
fn dimmed_backdrop_keeps_the_title_overlay_variant_and_pipeline_stable() {
    let mut app = app_with_card_image();
    let ensure = |app: &mut App| {
        app.ensure_title_overlay_protocol(
            CARD_KEY,
            (16, 32),
            &title_parts("a title"),
            None,
            "Movie",
        )
    };

    let variant =
        ensure(&mut app).expect("the variant builds while the configured protocol is active");
    let builds_after_open = app.image_protocols.protocol_builds();

    app.dim_backdrop_active = true;
    assert_eq!(
        ensure(&mut app),
        Some(variant.clone()),
        "the dimmed backdrop must not size the overlay into a new variant key"
    );
    assert_eq!(
        app.image_protocols.protocol_builds(),
        builds_after_open,
        "the sizing path must measure the configured protocol, not build fallback re-encodes"
    );

    app.dim_backdrop_active = false;
    assert_eq!(
        ensure(&mut app),
        Some(variant),
        "closing the backdrop returns to the same variant"
    );
}
