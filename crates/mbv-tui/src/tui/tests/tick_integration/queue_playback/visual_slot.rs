//! The visual slot and artwork half of the Queue playback panel's tick
//! tests (`split-tui-gui-clients` row 2.4): the title's home over a hidden
//! slot, and the queue card's paint checkpoint. The transport and layout
//! tests stay in the parent module, whose fixtures these reuse.

use super::*;

/// Hiding the visual slot while playing (the user's hide toggle) takes the
/// title's home back from the artwork: the sync pass resets the carried-over
/// site to `Header`, so the header reappears over a slot that paints nothing
/// instead of the title living nowhere.
#[test]
fn hiding_the_visual_slot_while_playing_returns_the_title_to_the_header() {
    let mut app = active_app(PanelMode::Both);
    app.seed_queue_card_title_site_for_test(NowPlayingTitleSite::Artwork);
    let mut harness = TickHarness::new(app);
    crate::tui::test_support::set_terminal_size(harness.model_mut(), 100, 40);

    harness.model_mut().sync_mounted_surfaces();
    harness
        .model_mut()
        .app
        .set_visual_slot_hidden_for_test(true);
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        harness.model().app.queue_card_title_site_for_test(),
        NowPlayingTitleSite::Header,
        "the hidden slot's header is the title's home again"
    );
}

/// Install a configured kitty picker and the halfblock fallback on a model the
/// way `init_image_protocols` does, so the slot's paint resolves a protocol (a
/// bare `App` carries none).
fn set_image_protocols(mut model: crate::tui::Model) -> crate::tui::Model {
    let mut configured = ratatui_image::picker::Picker::halfblocks();
    configured.set_protocol_type(ratatui_image::picker::ProtocolType::Kitty);
    crate::tui::install_image_pickers_for_test(
        &mut model,
        configured,
        ratatui_image::picker::Picker::halfblocks(),
    );
    model
}

/// The queue card's geometry checkpoint must reach its target and hold: the
/// recorded size settles on the same target a feedback-free paint of the slot
/// produces, never drifting or stalling below it. The feedback loop behind
/// this (the 2026-10-04 user report) is documented at
/// `queue_card_reserved_rect`.
#[test]
fn queue_card_checkpoint_reaches_its_target_and_holds_across_frames() {
    fn card_app() -> mbv_client_core::App {
        let mut app = active_app(PanelMode::Both);
        app.configure_image_protocol_for_test(None, true);
        // A cached square artwork for the active item (the fixture's shared
        // "id", a Movie: its landscape card key) so the slot paints instead
        // of reserving a fetch no test client would complete.
        app.insert_cached_image_for_test(
            &mbv_images::emby_queue_landscape_cache_key("id"),
            mbv_images::CachedImage {
                img: Some(image::DynamicImage::ImageRgba8(
                    image::RgbaImage::from_pixel(200, 200, image::Rgba([20, 40, 60, 255])),
                )),
                source: None,
                cover_box: None,
                applied_logo_key: None,
            },
        );
        app
    }

    fn paint_until_recorded(
        app: &mut crate::tui::Model,
        area: ratatui::layout::Rect,
    ) -> (u16, u16) {
        let mut size = (0, 0);
        // Bounded spins, no sleeps: each paint pumps the protocol's encode
        // channel; the fitted size lands within a few.
        for _ in 0..50 {
            let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
            terminal
                .draw(|frame| {
                    crate::tui::queue_card::render_queue_playback_slot(app, 40, frame, area, true);
                })
                .unwrap();
            size = app.app.last_card_size();
            if size != (0, 0) {
                break;
            }
        }
        size
    }

    // The slot paints through the configured picker the shell installs before
    // the listener starts (`Model::init_image_pickers`), which encodes the
    // seeded artwork's protocol from its retained source. The picker lives on
    // the shell, so the seeded model itself runs the loop.
    let (_, listener) = TickHarness::port();
    let mut harness = TickHarness::from_model(set_image_protocols(
        crate::tui::Model::new_with_listener(card_app(), listener),
    ));
    let mut sizes = Vec::new();
    crate::tui::test_support::set_terminal_size(harness.model_mut(), 120, 40);
    for _ in 0..6 {
        harness.model_mut().sync_mounted_surfaces();
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        terminal
            .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
            .unwrap();
        sizes.push(harness.model().app.last_card_size());
    }

    // The feedback-free reference: the same slot painted with its height
    // raised to the tier cap — the rows the band offers once it is sized by
    // the target itself — with the slot projected directly (no sync pass).
    let mut reference = set_image_protocols(crate::tui::Model::new(card_app()));
    reference.app.seed_queue_card_projection_for_test(
        mbv_ui_model::playback::QueueCardProjection {
            cache_key: Some(mbv_images::emby_queue_landscape_cache_key("id")),
            plain_cache_key: None,
            images_enabled: true,
            visualizer: false,
            title_site: NowPlayingTitleSite::Header,
        },
    );
    let placement =
        crate::tui::draw::compute_chrome_geometry(&reference.app, 0, 120, Rect::new(0, 0, 120, 40))
            .root
            .queue_playback
            .expect("queue playback placed while playing");
    let slot = mbv_render::arrangements::chrome::queue_playback_slot_region(placement);
    let target_area = ratatui::layout::Rect {
        height: mbv_render::components::card::queue_card_height_cap(40),
        ..slot
    };
    let expected = paint_until_recorded(&mut reference, target_area);
    assert_ne!(expected, (0, 0), "the reference paint recorded a size");

    assert_eq!(
        sizes[4], sizes[5],
        "the checkpoint must hold steady once reached"
    );
    assert_eq!(
        sizes[5], expected,
        "the closed loop must settle on the feedback-free target, not drift or stall below it"
    );
}
