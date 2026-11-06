//! The Queue playback band's headless tests (row 6.5): the egui
//! `Context::run_ui` composition with synthetic `RawInput`, asserting the
//! request the shell boundary receives and the band an idle queue reserves,
//! never appearance (design D11).

use eframe::egui;
use mbv_surfaces::queue_playback::QueuePlaybackHitTarget;
use mbv_ui_msg::{Msg, PlaybackRequest};

use crate::headless::{frame_input, run_frame};
use crate::tick_host::GuiTickHost;

/// The centre of the region the last frame painted for `target`.
fn centre_of(host: &GuiTickHost, target: QueuePlaybackHitTarget) -> egui::Pos2 {
    let (region, _) = host
        .playback_test_report()
        .hit_map
        .iter()
        .find(|(_, painted)| **painted == target)
        .expect("the control paints in a playing band");
    egui::pos2(
        region.x + region.width / 2.0,
        region.y + region.height / 2.0,
    )
}

/// One primary press at `at` over the fixed window.
fn press_at(at: egui::Pos2) -> egui::RawInput {
    frame_input(vec![egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    }])
}

/// A press on the play/pause control sends the toggle request across the shell
/// boundary (row 6.5's verification). The press travels the full path: the
/// egui translation edge, the gesture policy, the band's painted hit map, and
/// the Msg the shell boundary receives.
#[test]
fn a_click_on_play_pause_sends_the_toggle_request() {
    let ctx = egui::Context::default();
    let mut host = GuiTickHost::new(mbv::stub_app_with_playing_queue(40), ctx.clone());
    run_frame(&ctx, &mut host, frame_input(Vec::new()));

    let at = centre_of(&host, QueuePlaybackHitTarget::PlayPause);
    run_frame(&ctx, &mut host, press_at(at));

    assert_eq!(
        host.playback_test_request(),
        Some(&Msg::Playback(PlaybackRequest::TogglePlayPause))
    );
}

/// An idle queue collapses the transport section: the panel reports no regions
/// a press could resolve against.
#[test]
fn empty_queue_paints_no_transport_band() {
    let ctx = egui::Context::default();
    let mut host = GuiTickHost::new(mbv::stub_app_with_queue(0), ctx.clone());
    run_frame(&ctx, &mut host, frame_input(Vec::new()));

    assert!(!host.playback_test_shown());
    assert!(host.playback_test_report().hit_map.is_empty());
}

/// An idle queue still paints the band's header row: the status word and the
/// target indicator stay on the band while the transport section collapses
/// (row 6.5's header).
#[test]
fn an_idle_band_paints_its_header_row() {
    let ctx = egui::Context::default();
    let mut host = GuiTickHost::new(mbv::stub_app_with_queue(0), ctx.clone());
    run_frame(&ctx, &mut host, frame_input(Vec::new()));

    assert!(host.playback_test_header_painted());
}
