//! The queue card's visual slot under the dimmed backdrop's halfblock
//! fallback (#451). User regression: opening a modal or the queue context
//! menu flipped the card's picker to the hardcoded-grid fallback, whose
//! paint (a) moved the geometry checkpoint and pushed the queue rows down,
//! and (b) measured the title overlay into a new, never-painted variant key
//! that flashed the now-playing title back to the header row for a moment.

use mbv_images::CachedImage;
use mbv_ui_model::playback::{NowPlayingTitleSite, QueueCardProjection};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui_image::picker::{Picker, ProtocolType};

use crate::app::App;
use crate::app::tests::make_app_stub;

const CARD_KEY: &str = "card:P";

fn card_image(width: u32, height: u32) -> image::DynamicImage {
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

/// An app with one decoded card image and a projected queue slot for it.
/// The configured picker is kitty on the fallback constructor's default font
/// grid; tests here cannot construct a differently-sized picker (the public
/// API bakes the terminal-detected font size in at query time), so the
/// fallback's grid coincides with the configured one — the assertions pin
/// the decision wiring, not the font mismatch itself.
fn app_with_card_image() -> App {
    let mut app = make_app_stub();
    let mut configured = Picker::halfblocks();
    configured.set_protocol_type(ProtocolType::Kitty);
    app.images.configure_protocol(None, true);
    app.images
        .set_image_pickers_for_test(configured, Picker::halfblocks());
    app.images
        .insert_image(CARD_KEY.to_owned(), cached(Some(card_image(4, 2))));
    app.queue_card_projection = QueueCardProjection {
        cache_key: Some(CARD_KEY.to_owned()),
        plain_cache_key: None,
        images_enabled: true,
        visualizer: false,
        title_site: NowPlayingTitleSite::Header,
    };
    app.terminal_height = 50;
    app
}

fn paint_slot(app: &mut App) {
    let mut terminal = Terminal::new(TestBackend::new(48, 30)).unwrap();
    terminal
        .draw(|frame| {
            app.render_queue_playback_slot(frame, Rect::new(0, 0, 40, 24), true);
        })
        .unwrap();
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

/// A dimming overlay must not move the queue card's geometry checkpoint: a
/// fallback paint measured on the fallback's grid is not geometry-comparable
/// with the configured protocol's, and recording it resized the reservation
/// (`queue_playback_rows` derives the queue list's y from it) every time a
/// modal or context menu opened, and back when it closed. Recording resumes
/// for the configured protocol's paint once the backdrop lifts.
#[test]
fn dimmed_backdrop_paint_does_not_move_the_queue_card_checkpoint() {
    let mut app = app_with_card_image();
    // The configured protocol's last paint before any modal opened.
    app.images.record_card_size(16, 32);

    app.dim_backdrop_active = true;
    paint_slot(&mut app);
    assert_eq!(
        app.images.last_card_size(),
        (16, 32),
        "the dimmed backdrop's fallback paint must not move the checkpoint"
    );

    app.dim_backdrop_active = false;
    paint_slot(&mut app);
    assert_ne!(
        app.images.last_card_size(),
        (16, 32),
        "recording resumes for the configured protocol's paint"
    );
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
    let available = ratatui::layout::Size {
        width: 16,
        height: 32,
    };
    let ensure = |app: &mut App| {
        app.ensure_title_overlay_protocol(
            CARD_KEY,
            available,
            &title_parts("a title"),
            None,
            "Movie",
        )
    };

    let variant =
        ensure(&mut app).expect("the variant builds while the configured protocol is active");
    let builds_after_open = app.images.image_protocol_builds();

    app.dim_backdrop_active = true;
    assert_eq!(
        ensure(&mut app),
        Some(variant.clone()),
        "the dimmed backdrop must not size the overlay into a new variant key"
    );
    assert_eq!(
        app.images.image_protocol_builds(),
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
