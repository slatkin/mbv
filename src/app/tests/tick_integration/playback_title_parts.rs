//! Task 5.2: one real `Application::tick()` through the shell sync pass
//! proves both playback panels receive the typed now-playing title parts
//! from the one shared transport projection — a two-part item (audio track
//! with an artist context) and a single-part item (a movie) at the same
//! playhead, in the queue-column panel and in the right-column strip.
//!
//! Row 4.1: the same harness pins the no-flash header rule end to end — with
//! overlay-capable art the header stays hidden while the overlay composes
//! (playback start and track change alike) and reappears only in the
//! unreachable fallbacks (visualizer, images off, visual slot hidden).

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui_image::picker::{Picker, ProtocolType};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

use crate::app::tests::tick_integration::harness::TickHarness;
use crate::app::tests::{QueueViewTestExt, make_app_stub};
use crate::app::{App, PanelFocus, PanelMode};
use mbv_components::{LibraryPlaybackPanel, QueuePlaybackPanel};
use mbv_emby_model::test_support::make_item;
use mbv_images::CachedImage;
use mbv_queue::{PlaybackTitlePart, PlaybackTitlePartRole, PlaybackTitleParts};
use mbv_render::arrangements::chrome::QUEUE_PLAYBACK_HEADER_ROWS;
use mbv_ui_msg::ComponentId;

/// An unbound chord: no policy arm claims it and the focused component
/// ignores it, so the tick is real while mutating nothing.
fn inert_key() -> Event<mbv_ui_msg::UserEvent> {
    Event::Keyboard(KeyEvent {
        code: Key::Function(24),
        modifiers: KeyModifiers::NONE,
    })
}

/// An active, locally-owned playhead over a two-item queue: slot 0 is an
/// audio track with an artist (two parts), slot 1 a movie (one part).
fn title_parts_app(active_idx: usize) -> App {
    let mut app = make_app_stub();
    app.panel_mode = PanelMode::Both;
    app.panel_focus = PanelFocus::Queue;
    let mut track = make_item("Track Two", "Audio");
    track.id = "track-two".into();
    track.media_type = "Audio".into();
    track.artist = "Artist B".into();
    let mut movie = make_item("Movie One", "Movie");
    movie.id = "movie-one".into();
    app.local_view.adopt_items(vec![track, movie], 0);
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = active_idx;
        status.queue_len = 2;
    };
    app
}

fn expected_parts(title: &str, context: Option<&str>) -> PlaybackTitleParts {
    PlaybackTitleParts {
        title: PlaybackTitlePart {
            role: PlaybackTitlePartRole::Title,
            text: title.into(),
        },
        context: context.map(|text| PlaybackTitlePart {
            role: PlaybackTitlePartRole::Context,
            text: text.into(),
        }),
    }
}

fn step_tick(harness: &mut TickHarness) {
    harness.inject(inert_key());
    harness.step();
}

fn queue_panel(harness: &TickHarness) -> &QueuePlaybackPanel {
    harness
        .model()
        .application
        .get_component(&ComponentId::QueuePlaybackPanel)
        .and_then(|component| component.as_any().downcast_ref::<QueuePlaybackPanel>())
        .expect("Queue playback panel mounted in a queue-visible layout")
}

fn strip(harness: &TickHarness) -> &LibraryPlaybackPanel {
    harness
        .model()
        .application
        .get_component(&ComponentId::LibraryPlaybackPanel)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPlaybackPanel>())
        .expect("the strip mounts when the queue column is hidden")
}

#[test]
fn tick_projects_title_parts_to_both_playback_panels_from_the_one_projection() {
    let mut harness = TickHarness::new(title_parts_app(0));

    // Queue-column panel (queue visible): the two-part slot projects both
    // parts, the single-part slot one.
    step_tick(&mut harness);
    assert_eq!(
        queue_panel(&harness)
            .transport_title_parts_for_test()
            .as_ref(),
        Some(&expected_parts("Track Two", Some("Artist B"))),
        "the two-part slot projects a title part and a context part"
    );

    harness
        .model_mut()
        .app
        .player
        .status
        .lock()
        .unwrap()
        .current_idx = 1;
    step_tick(&mut harness);
    assert_eq!(
        queue_panel(&harness)
            .transport_title_parts_for_test()
            .as_ref(),
        Some(&expected_parts("Movie One", None)),
        "the single-part slot projects the title part alone"
    );

    // Right-column strip (queue column hidden): the same playheads project
    // through the same shared projection into the other panel.
    harness.model_mut().app.panel_mode = PanelMode::LibraryOnly;
    harness.model_mut().app.panel_focus = PanelFocus::Library;
    step_tick(&mut harness);
    assert_eq!(
        strip(&harness).title_parts_for_test().as_ref(),
        Some(&expected_parts("Movie One", None)),
        "the strip receives the single-part projection"
    );

    harness
        .model_mut()
        .app
        .player
        .status
        .lock()
        .unwrap()
        .current_idx = 0;
    step_tick(&mut harness);
    assert_eq!(
        strip(&harness).title_parts_for_test().as_ref(),
        Some(&expected_parts("Track Two", Some("Artist B"))),
        "the strip receives the two-part projection"
    );
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

/// A playhead whose card art is overlay-capable: the protocol is enabled,
/// the pickers are configured, the card box is measured and both slots carry
/// a cached base bitmap, so the title-site classification resolves to the
/// artwork site (pending composition) rather than an unreachable fallback.
fn overlay_capable_app(active_idx: usize) -> App {
    let mut app = title_parts_app(active_idx);
    app.terminal_width = 100;
    app.terminal_height = 40;
    app.images.configure_protocol(None, true);
    let mut picker = Picker::halfblocks();
    picker.set_protocol_type(ProtocolType::Kitty);
    app.images
        .set_image_pickers_for_test(picker, Picker::halfblocks());
    app.images.record_card_size(8, 16);
    app.images
        .insert_image("track-two:P".into(), cached_colour_image(8, 4));
    app.images
        .insert_image("movie-one:QB".into(), cached_colour_image(8, 4));
    app
}

/// Inject the overlay landing as an outcome: mark the variant key the last
/// sync composed as painted, so a later sync observes `painted == true`
/// without driving the image pipeline or waiting on a fetch.
fn land_painted_overlay(harness: &mut TickHarness) {
    let key = harness
        .model()
        .app
        .queue_card_projection
        .cache_key
        .clone()
        .expect("a composed overlay projects its variant key");
    assert!(
        key.contains(mbv_images::title_overlay::DERIVED_SEP),
        "the composed key is the overlay variant: {key:?}"
    );
    harness
        .model_mut()
        .app
        .images
        .record_painted_title_overlay(Some(key));
}

/// Draw one real frame and return the Queue playback panel's header row as
/// text: the placement's recessed row between the two-column insets
/// (`queue_panel_inset`), where the panel paints the now-playing title while
/// the header is reserved and nothing while the artwork carries the title.
fn painted_header_row(harness: &mut TickHarness) -> String {
    let (width, height) = (
        harness.model().app.terminal_width,
        harness.model().app.terminal_height,
    );
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    let placement = harness
        .model()
        .app
        .layout
        .root_frame
        .queue_playback
        .expect("queue playback placed in a queue-visible layout");
    let row = placement.y + 1;
    (placement.x + 2..placement.x + placement.width.saturating_sub(2))
        .map(|x| terminal.backend().buffer()[(x, row)].symbol().to_string())
        .collect()
}

/// The behavioral seam row 4.1 pins: the classification the shell projects,
/// the `queue_header_rows()` policy geometry and paint share, and the panel's
/// painted header row must all say the header is absent.
fn assert_header_hidden(harness: &mut TickHarness, title: &str) {
    assert!(
        !harness.model().app.queue_card_projection.header_visible,
        "the artwork site keeps the header hidden"
    );
    assert_eq!(
        harness.model().app.queue_header_rows(),
        0,
        "the artwork site reserves zero header rows"
    );
    assert!(
        !painted_header_row(harness).contains(title),
        "the panel paints no now-playing header while the artwork carries the title"
    );
}

/// The same seam in an unreachable fallback: the header carries the title and
/// the panel paints it on its reserved row.
fn assert_header_shown(harness: &mut TickHarness, title: &str) {
    assert!(
        harness.model().app.queue_card_projection.header_visible,
        "an unreachable artwork site shows the header"
    );
    assert_eq!(
        harness.model().app.queue_header_rows(),
        QUEUE_PLAYBACK_HEADER_ROWS,
        "an unreachable artwork site reserves the header band"
    );
    assert!(
        painted_header_row(harness).contains(title),
        "the panel paints the now-playing title on the header row"
    );
}

/// Row 4.1 / spec "Overlay not yet painted": with overlay-capable art the
/// header stays hidden from playback start's first frame (no transient header
/// while the overlay composes), and a landed overlay does not bring it back.
#[test]
fn playback_start_keeps_the_header_hidden_across_the_overlay_compose_window() {
    let mut harness = TickHarness::new(overlay_capable_app(0));

    step_tick(&mut harness);
    assert_header_hidden(&mut harness, "Track Two");

    land_painted_overlay(&mut harness);
    step_tick(&mut harness);
    assert_header_hidden(&mut harness, "Track Two");
}

/// Row 4.1 / spec "Track change does not re-introduce the header": once the
/// artwork carries the title, switching tracks keeps the header hidden while
/// the new overlay composes and after it lands.
#[test]
fn track_change_keeps_the_header_hidden_for_the_new_overlay() {
    let mut harness = TickHarness::new(overlay_capable_app(0));
    step_tick(&mut harness);
    land_painted_overlay(&mut harness);
    step_tick(&mut harness);
    assert_header_hidden(&mut harness, "Track Two");

    harness
        .model_mut()
        .app
        .player
        .status
        .lock()
        .unwrap()
        .current_idx = 1;
    step_tick(&mut harness);
    assert_header_hidden(&mut harness, "Movie One");

    land_painted_overlay(&mut harness);
    step_tick(&mut harness);
    assert_header_hidden(&mut harness, "Movie One");
}

/// Row 4.1 / spec "Visualizer replaces artwork": switching the visual slot to
/// the visualizer moves the title back to the header, and switching away
/// returns it to the artwork.
#[test]
fn visualizer_toggle_shows_and_hides_the_header() {
    let mut harness = TickHarness::new(overlay_capable_app(0));
    step_tick(&mut harness);
    land_painted_overlay(&mut harness);
    step_tick(&mut harness);
    assert_header_hidden(&mut harness, "Track Two");

    harness.model_mut().app.visualizer_enabled = true;
    step_tick(&mut harness);
    assert_header_shown(&mut harness, "Track Two");

    harness.model_mut().app.visualizer_enabled = false;
    step_tick(&mut harness);
    assert_header_hidden(&mut harness, "Track Two");
}

/// Row 4.1: disabling images (or the image protocol) is an unreachable
/// fallback, so the header carries the title.
#[test]
fn disabling_images_shows_the_header() {
    let mut harness = TickHarness::new(overlay_capable_app(0));
    step_tick(&mut harness);
    land_painted_overlay(&mut harness);
    step_tick(&mut harness);
    assert_header_hidden(&mut harness, "Track Two");

    harness
        .model_mut()
        .app
        .images
        .configure_protocol(Some("kitty".into()), false);
    step_tick(&mut harness);
    assert_header_shown(&mut harness, "Track Two");
}

/// Row 4.1 / spec "Header returns in a fallback state": hiding the visual
/// slot moves the title back to the header; showing it again returns the
/// title to the artwork.
#[test]
fn hiding_the_visual_slot_shows_the_header_and_restoring_hides_it() {
    let mut harness = TickHarness::new(overlay_capable_app(0));
    step_tick(&mut harness);
    land_painted_overlay(&mut harness);
    step_tick(&mut harness);
    assert_header_hidden(&mut harness, "Track Two");

    harness.model_mut().app.visual_slot_hidden = true;
    step_tick(&mut harness);
    assert_header_shown(&mut harness, "Track Two");

    harness.model_mut().app.visual_slot_hidden = false;
    step_tick(&mut harness);
    assert_header_hidden(&mut harness, "Track Two");
}
