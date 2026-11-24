//! Queue artwork ownership: movies fetch landscape art, episodes never show
//! series art; overlay composition stays off the tick thread during a drag.

use crate::tui::Model;
use mbv_client_core::state::projection::card::refresh_queue_card_image;
use mbv_client_core::tests::render_fixtures::make_queue_app;
use mbv_images::CachedImage;
use ratatui_image::picker::{Picker, ProtocolType};

fn cached_colour_image(width: u32, height: u32) -> CachedImage {
    CachedImage {
        img: Some(image::DynamicImage::ImageRgba8(
            image::RgbaImage::from_pixel(width, height, image::Rgba([20, 40, 60, 255])),
        )),
        source: None,
        cover_box: None,
        applied_logo_key: None,
    }
}

/// An active-playing queue app with the card protocol and one base bitmap, so
/// the title-site decision can reach the overlay-variant build. The protocol
/// store is the shell's (`Model`), so these tests drive the model.
fn overlay_app() -> Model {
    let mut app = make_queue_app(1);
    app.seed_playing_status_for_test(0, 1);
    let mut picker = Picker::halfblocks();
    picker.set_protocol_type(ProtocolType::Kitty);
    app.configure_image_protocol_for_test(None, true);
    let mut model = Model::new(app);
    crate::tui::install_image_pickers_for_test(&mut model, picker, Picker::halfblocks());
    // (height, width): the card box the overlay is composed for.
    model.app.record_card_size_for_test(4, 8);
    model
}

/// `overlay_app` with the base bitmap already inserted, so the overlay
/// variant can compose this sync.
fn overlay_app_with_base_image(model: Model) -> (Model, String) {
    let key =
        crate::queue_card_cache_key_for_test(&model.app, 0).expect("the queue holds an Emby item");
    let mut model = model;
    model
        .app
        .insert_cached_image_for_test(key.as_str(), cached_colour_image(4, 2));
    (model, key)
}

/// Run the card's sync pass and build the overlay variant from the record it
/// leaves, exactly as `Model::sync_queue` does: the decision is the core's,
/// the overlay's protocol is the shell's.
fn sync_card(model: &mut Model, resizing: bool) {
    let record = refresh_queue_card_image(&mut model.app, resizing);
    if let Some(record) = record {
        model.ensure_queue_card_title_overlay(&record, resizing);
    }
}

/// User-reported regression: every distinct fitted width during a queue-column
/// resize drag composed a Lanczos3 title overlay synchronously on the tick
/// thread. The gate must build no variant while the drag is active and compose
/// exactly one once it ends — and the drag must not resurrect the header:
/// the site carries over through the transient gate (2026-10-03).
#[test]
fn column_resize_drag_builds_no_overlay_variant_until_it_ends() {
    let (mut model, base_key) = overlay_app_with_base_image(overlay_app());
    model
        .cached_image_protocol_mut(&base_key)
        .expect("the base card protocol is available");

    // Warm sync: eligibility passes and the site is decided before any paint.
    sync_card(&mut model, false);
    assert_eq!(
        model.app.queue_card_title_site_for_test(),
        mbv_ui_model::playback::NowPlayingTitleSite::Artwork,
        "the warm sync decides the site before the panel first displays"
    );
    let baseline = model.image_protocols.protocol_builds();

    sync_card(&mut model, true);
    assert_eq!(
        model.image_protocols.protocol_builds(),
        baseline,
        "no overlay variant is composed while the column drag is active"
    );
    assert_eq!(
        model.app.queue_card_projection().cache_key.as_deref(),
        Some(base_key.as_str()),
        "the card paints plain base art during the drag"
    );
    assert_eq!(
        model.app.queue_card_title_site_for_test(),
        mbv_ui_model::playback::NowPlayingTitleSite::Artwork,
        "the resize drag must not resurrect the header: the site carries over"
    );

    sync_card(&mut model, false);
    assert_eq!(
        model.image_protocols.protocol_builds(),
        baseline,
        "the drag composes nothing and the warm sync's variant is reused at the unchanged size"
    );
    assert!(
        model
            .app
            .queue_card_projection()
            .cache_key
            .as_deref()
            .is_some_and(|key| key.contains(":t:")),
        "the ended drag projects the overlay variant: {:?}",
        model.app.queue_card_projection().cache_key
    );
}

/// The flash fix's core: the site is decided while the base-art fetch is
/// still in flight. A missing cache *entry* means the art will arrive and
/// carry the title — deciding only after its arrival is what made the
/// header flash on every playback start (2026-10-03).
#[test]
fn the_site_is_decided_while_the_art_fetch_is_pending() {
    let mut model = overlay_app();
    assert!(
        model
            .app
            .queue_card_projection()
            .cache_key
            .as_deref()
            .and_then(|key| model.app.cached_image(key))
            .is_none(),
        "the test fixture starts with the art fetch pending"
    );
    sync_card(&mut model, false);
    assert_eq!(
        model.app.queue_card_title_site_for_test(),
        mbv_ui_model::playback::NowPlayingTitleSite::Artwork,
        "a pending art fetch must not hold the header as the title's home"
    );
}

/// The transient/transient split's other half: a mode gate is a stable
/// presentation fact, so it forces the header back as the title's home even
/// from a carried-over `Artwork` site.
#[test]
fn a_mode_gate_forces_the_header_back_from_the_artwork_site() {
    let (mut model, _base_key) = overlay_app_with_base_image(overlay_app());
    sync_card(&mut model, false);
    assert_eq!(
        model.app.queue_card_title_site_for_test(),
        mbv_ui_model::playback::NowPlayingTitleSite::Artwork
    );

    // The visualizer replaces the artwork: the artwork can no longer carry
    // the title, so the header is its home again.
    model.app.set_visualizer_enabled_for_test(true);
    sync_card(&mut model, false);
    assert_eq!(
        model.app.queue_card_title_site_for_test(),
        mbv_ui_model::playback::NowPlayingTitleSite::Header,
        "a mode gate forces the header back from the artwork site"
    );
}

/// Content the owner's Emby Session output presents but did not load holds
/// the slotless path: playback follows the `OutputState` `now_playing`
/// payload and no queue slot addresses the item (design D12).
fn session_output(series_id: Option<&str>, series_name: Option<&str>) -> mbv_ctrl::OutputState {
    mbv_ctrl::OutputState::EmbySession {
        device_id: "device-1".to_string(),
        name: "Living Room TV".to_string(),
        connection: mbv_ctrl::OutputConnection::Connected,
        audio_only: false,
        now_playing: Some(mbv_ctrl::SessionNowPlaying {
            item_id: "remote-ep".to_string(),
            item_type: "Episode".to_string(),
            name: "Episode name".to_string(),
            series_id: series_id.map(str::to_string),
            series_name: series_name.map(str::to_string),
            logo_tag: None,
        }),
    }
}

fn slotless_session_app() -> Model {
    let mut model = overlay_app();
    model
        .app
        .seed_playback_output_for_test(Some(session_output(Some("remote-series"), Some("Show"))));
    model
}

/// The slotless remote path must resolve the same logo owner the local queue
/// item does: the episode's show logo, fetched under the series-id key
/// (remote-session-overlay-parity, task 3.2; spec scenario "Remote episode
/// uses its show's logo").
#[test]
fn slotless_session_episode_fetches_its_series_logo() {
    let mut model = slotless_session_app();
    let fetches_before = model.app.card_image_fetch_count_for_test();

    sync_card(&mut model, false);

    assert!(
        model
            .app
            .queue_card_fetch_loading_for_test("remote-series:Logo"),
        "the slotless episode must start the series-logo fetch"
    );
    assert_eq!(
        model.app.card_image_fetch_count_for_test(),
        fetches_before + 2,
        "the card fetch and the logo fetch are separate reservations"
    );
}

/// Once the base art and the series logo are both ready, the slotless overlay
/// composes the same logo variant the local path composes: the ready logo key
/// joins the variant key (design D7).
#[test]
fn slotless_session_episode_composes_the_logo_overlay_variant() {
    let mut model = slotless_session_app();
    let card_key = crate::card_cache_key_for_id_for_test("remote-ep", Some("Episode"));
    model
        .app
        .insert_cached_image_for_test(&card_key, cached_colour_image(4, 2));
    model
        .app
        .insert_cached_image_for_test("remote-series:Logo", cached_colour_image(4, 2));

    sync_card(&mut model, false);

    assert_eq!(
        model.app.queue_card_title_site_for_test(),
        mbv_ui_model::playback::NowPlayingTitleSite::Artwork
    );
    let expected = mbv_images::title_overlay::title_overlay_cache_key(
        &card_key,
        8,
        2,
        mbv_images::title_overlay::TitleOverlayText {
            context: Some("Show"),
            title: "Episode name",
        },
        Some("remote-series:Logo"),
    );
    assert_eq!(
        model.app.queue_card_projection().cache_key.as_deref(),
        Some(expected.as_str()),
        "the overlay variant must carry the ready series logo"
    );
}

/// A session payload without a resolvable logo reference — an episode with no
/// series id, a movie with no advertised logo image — keeps the text overlay
/// and starts no logo fetch (spec scenario "Remote item without a resolvable
/// logo reference").
#[test]
fn slotless_session_without_logo_reference_keeps_the_text_overlay() {
    let mut model = overlay_app();
    model
        .app
        .seed_playback_output_for_test(Some(session_output(None, None)));
    let fetches_before = model.app.card_image_fetch_count_for_test();

    sync_card(&mut model, false);

    assert!(
        !model
            .app
            .queue_card_fetch_loading_for_test("remote-series:Logo")
    );
    assert_eq!(
        model.app.card_image_fetch_count_for_test(),
        fetches_before + 1,
        "only the card fetch is reserved; no logo fetch without a series id"
    );
}
