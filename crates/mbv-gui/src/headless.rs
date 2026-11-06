//! The GUI Client's headless frame harness (design D11 of `add-gui-front-end`):
//! one egui `Context::run_ui` pass over synthetic `RawInput`, with no window
//! and no GPU. The tests built on it assert core state and delivery, never
//! appearance.

use std::time::Instant;

use eframe::egui;
use mbv::ClientCore;

use crate::tick_host::GuiTickHost;

/// One headless frame, in the production order: the Client core's tick over
/// the frame's raw input (the input phase reads it inside the frame, as the
/// real window's `logic` does), then the surface sync, then the paint.
pub(crate) fn run_frame(ctx: &egui::Context, host: &mut GuiTickHost, raw_input: egui::RawInput) {
    // The frame's `FullOutput` is paint bookkeeping the test does not read,
    // and no GPU applies its texture deltas, so they are dropped unapplied.
    ctx.run_ui(raw_input, |ui| {
        ClientCore::tick(host, Instant::now());
        host.sync_surfaces();
        host.paint(ui);
    })
    .drop_without_applying_deltas();
}

/// One frame's input over a fixed window.
pub(crate) fn frame_input(events: Vec<egui::Event>) -> egui::RawInput {
    frame_input_at_width(480.0, events)
}

/// One frame's input over a window `width` logical points wide, at the test
/// window's fixed height.
pub(crate) fn frame_input_at_width(width: f32, events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(width, 640.0),
        )),
        events,
        ..egui::RawInput::default()
    }
}

/// One pressed key event over the fixed window.
pub(crate) fn key_input(key: egui::Key) -> egui::RawInput {
    frame_input(vec![egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }])
}
