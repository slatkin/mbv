//! The Queue panel's headless tests (row 6.4): the egui `Context::run`
//! composition with synthetic `RawInput`, asserting the core state the frame
//! drives, never appearance (design D11).

use std::time::Instant;

use eframe::egui;

use crate::tick_host::GuiTickHost;
use mbv::ClientCore;

/// One headless frame, in the production order: the Client core's tick over
/// the frame's raw input (the input phase reads it inside the frame, as the
/// real window's `update` does), then the surface sync, then the paint.
fn run_frame(ctx: &egui::Context, host: &mut GuiTickHost, raw_input: egui::RawInput) {
    // The frame's `FullOutput` is paint bookkeeping the test does not read.
    let _ = ctx.run(raw_input, |ctx| {
        ClientCore::tick(host, Instant::now());
        host.sync_surfaces();
        host.paint(ctx);
    });
}

/// One frame's input over a fixed window: the Queue panel paints the whole
/// window in the first slice.
fn frame_input(events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(480.0, 640.0),
        )),
        events,
        ..egui::RawInput::default()
    }
}

/// One pressed key event over the fixed window.
fn key_input(key: egui::Key) -> egui::RawInput {
    frame_input(vec![egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }])
}

/// Row 6.4's verification: a `PageDown` press moves the queue cursor by the
/// page stride the reported capacity drives — one row short of the whole
/// rows the painted list fits (`client-interaction-core`: the stride source
/// is the painted geometry). The press travels the full path: the egui
/// translation edge, the shared router's `FallThrough`, and the Queue
/// Surface state's key rules.
#[test]
fn page_down_moves_the_cursor_by_the_painted_capacity() {
    let ctx = egui::Context::default();
    let mut host = GuiTickHost::new(mbv::stub_app_with_queue(40), ctx.clone());
    run_frame(&ctx, &mut host, frame_input(Vec::new()));

    let capacity = host.queue_test_capacity();
    assert!(capacity > 1, "the painted list must fit more than one row");
    let before = host.queue_test_cursor();

    run_frame(&ctx, &mut host, key_input(egui::Key::PageDown));

    let stride = capacity.saturating_sub(1).max(1);
    assert_eq!(host.queue_test_cursor(), before + stride);
}

/// The empty-queue state is the Queue panel's own state, not a placeholder:
/// an empty queue paints no row regions — the hit map stays empty so a
/// press resolves against no target — while the capacity still reports the
/// rows the list body fits, as the TUI Client's report does.
#[test]
fn empty_queue_paints_no_row_regions() {
    let ctx = egui::Context::default();
    let mut host = GuiTickHost::new(mbv::stub_app_with_queue(0), ctx.clone());
    run_frame(&ctx, &mut host, frame_input(Vec::new()));

    let report = host.queue_test_report();
    assert_eq!(report.capacity.whole_items(), 26);
    assert!(report.hit_map.is_empty());
}

/// A pointer press on a painted row moves the queue cursor to that slot,
/// exactly as a click does in the TUI Client
/// (`specs/gui-client`, "A queue row is clicked").
#[test]
fn a_click_on_a_painted_row_selects_that_slot() {
    let ctx = egui::Context::default();
    let mut host = GuiTickHost::new(mbv::stub_app_with_queue(40), ctx.clone());
    run_frame(&ctx, &mut host, frame_input(Vec::new()));
    let before = host.queue_test_cursor();

    // The row below the cursor row: the panel padding is 8 points and one
    // row is 24, so the third row's band is the click's target.
    let click = frame_input(vec![egui::Event::PointerButton {
        pos: egui::pos2(240.0, 8.0 + 2.5 * super::ROW_HEIGHT),
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    }]);
    run_frame(&ctx, &mut host, click);

    assert_eq!(host.queue_test_cursor(), before + 2);
}

/// A Visual selection's row-local `Space` claims the chord ahead of the
/// global play/pause toggle: the focused surface's Visual state feeds the
/// router's snapshot, and the deferred candidate cancels when the surface
/// consumes (`media-list-multi-select`). If `Space` reached the global
/// toggle instead, the mark set would stay as Visual mode left it.
#[test]
fn visual_selection_claims_space_ahead_of_the_global_toggle() {
    let ctx = egui::Context::default();
    let mut host = GuiTickHost::new(mbv::stub_app_with_queue(4), ctx.clone());
    run_frame(&ctx, &mut host, frame_input(Vec::new()));

    // Shift+V begins Visual mode at the cursor: the press arrives as egui's
    // caseless key with SHIFT plus its `Text` twin, the shape the
    // translation edge maps to the neutral chord. The `Text` twin reads the
    // frame's modifier state (`RawInput.modifiers`, which winit's
    // ModifiersChanged keeps current), so the fixture sets it.
    let mut shift_v = frame_input(vec![
        egui::Event::Key {
            key: egui::Key::V,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::SHIFT,
        },
        egui::Event::Text("V".to_string()),
    ]);
    shift_v.modifiers = egui::Modifiers::SHIFT;
    run_frame(&ctx, &mut host, shift_v);
    assert_eq!(host.queue_test_mark_count(), 1);

    run_frame(
        &ctx,
        &mut host,
        frame_input(vec![
            egui::Event::Key {
                key: egui::Key::Space,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::Text(" ".to_string()),
        ]),
    );
    assert_eq!(host.queue_test_mark_count(), 0);
}
