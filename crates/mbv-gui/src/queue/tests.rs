//! The Queue panel's headless tests (rows 6.4 and 6.7): the egui `Context::run`
//! composition with synthetic `RawInput`, asserting the core state the frame
//! drives, never appearance (design D11).

use eframe::egui;

use crate::headless::{frame_input, key_input, run_frame};
use crate::tick_host::GuiTickHost;
use mbv_queue::QueueSlotId;
use mbv_ui_model::media_list::{MediaKind, MediaListRow, MediaSemanticState};

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

    // The 480x640 test window less the Status bar row, the idle band's header
    // row with its panel padding (40 points), the list's panel padding, and the
    // footer row leaves 536 points of list: 22 rows.
    let report = host.queue_test_report();
    assert_eq!(report.capacity.whole_items(), 22);
    assert!(report.hit_map.is_empty());
}

/// An unsaved queue carries its dirty pill into the Queue footer: the
/// projection the frame adopts holds the pill the footer paints (row 6.4's
/// footer, design D11).
#[test]
fn an_unsaved_queue_paints_the_dirty_pill_in_the_footer() {
    let ctx = egui::Context::default();
    let mut host = GuiTickHost::new(mbv::stub_app_with_unsaved_queue(4), ctx.clone());
    run_frame(&ctx, &mut host, frame_input(Vec::new()));

    assert!(
        host.queue_test_footer()
            .is_some_and(|footer| footer.dirty.is_some())
    );
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

    // The third row, two below the cursor row, is the click's target.
    let click = frame_input(vec![egui::Event::PointerButton {
        pos: row_centre(2),
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    }]);
    run_frame(&ctx, &mut host, click);

    assert_eq!(host.queue_test_cursor(), before + 2);
}

/// Hovering over a painted row with no button held leaves the queue cursor
/// where it was (`specs/gui-client`, "Hover does not move focus"). The cursor
/// is the only focus the GUI shows yet, so it is what the test reads. The move
/// travels the full path: the translation edge maps it to a hover `Move`, which
/// the gesture policy recognizes as nothing.
#[test]
fn hovering_a_painted_row_without_a_press_leaves_the_cursor() {
    let ctx = egui::Context::default();
    let mut host = GuiTickHost::new(mbv::stub_app_with_queue(40), ctx.clone());
    run_frame(&ctx, &mut host, frame_input(Vec::new()));
    let before = host.queue_test_cursor();

    run_frame(
        &ctx,
        &mut host,
        frame_input(vec![egui::Event::PointerMoved(row_centre(3))]),
    );

    assert_eq!(host.queue_test_cursor(), before);
}

/// A move past the last visible row keeps the cursor row painted
/// (`specs/gui-client`, "Scrolling follows the cursor"). The cursor steps down
/// one frame per press, from the first row to the row below the last fully
/// visible one, and the frame that moves it paints it.
#[test]
fn a_move_past_the_last_visible_row_keeps_the_cursor_row_painted() {
    let ctx = egui::Context::default();
    let mut host = GuiTickHost::new(mbv::stub_app_with_queue(40), ctx.clone());
    run_frame(&ctx, &mut host, frame_input(Vec::new()));
    let capacity = host.queue_test_capacity();

    press_once_per_frame(&ctx, &mut host, egui::Key::ArrowDown, capacity);

    assert_eq!(host.queue_test_cursor(), capacity);
    assert!(host.queue_test_cursor_row_painted());
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

/// Review finding 3 (row 6.4, EDGE): a title wider than its column paints
/// clipped at that column's edge, never over the duration.
#[test]
fn a_long_title_paints_no_further_right_than_its_column() {
    let ctx = egui::Context::default();
    let output = ctx.run(frame_input(Vec::new()), |ctx| {
        let painter = ctx.layer_painter(egui::LayerId::background());
        super::paint_row_text(&painter, row_rect(), &long_title_row(), false);
    });

    assert!(title_clip(&output).right() <= duration_bounds(&output).left());
}

/// The duration text the long-title row paints. The title is every other
/// painted text in that row.
const DURATION: &str = "3:00";

/// One row whose title is far wider than the painted row, with a duration.
fn long_title_row() -> MediaListRow<QueueSlotId> {
    MediaListRow::Item {
        target: QueueSlotId::from_raw(1),
        primary: "x".repeat(200),
        secondary: None,
        trailing: None,
        duration: Some(DURATION.into()),
        kind: MediaKind::Media,
        semantic_state: MediaSemanticState::Ordinary,
    }
}

/// The one painted row: the window's width, one fixed row height.
fn row_rect() -> egui::Rect {
    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(320.0, super::ROW_HEIGHT))
}

/// The point at the vertical middle of list row `row` (the first row is `0`),
/// over the window's middle. The idle band's header row sits above the list:
/// its two panel paddings and one row, then the list's own panel padding, put
/// the first row 48 points down.
fn row_centre(row: u16) -> egui::Pos2 {
    let list_top = 3.0 * super::PANEL_PADDING + super::ROW_HEIGHT;
    egui::pos2(240.0, list_top + (f32::from(row) + 0.5) * super::ROW_HEIGHT)
}

/// Press `key` once per frame, `times` presses in all, so each move reaches
/// the paint that follows it, as in a running window.
fn press_once_per_frame(ctx: &egui::Context, host: &mut GuiTickHost, key: egui::Key, times: usize) {
    for _ in 0..times {
        run_frame(ctx, host, key_input(key));
    }
}

/// The clip rect of the painted title.
fn title_clip(output: &egui::FullOutput) -> egui::Rect {
    painted_text(output)
        .into_iter()
        .find(|(_, _, text)| text != DURATION)
        .map(|(clip, _, _)| clip)
        .expect("the title paints")
}

/// The painted bounds of the duration text.
fn duration_bounds(output: &egui::FullOutput) -> egui::Rect {
    painted_text(output)
        .into_iter()
        .find(|(_, _, text)| text == DURATION)
        .map(|(_, bounds, _)| bounds)
        .expect("the duration paints")
}

/// Every painted text shape of one frame, in paint order: the clip rect,
/// the painted bounds, and the full text.
fn painted_text(output: &egui::FullOutput) -> Vec<(egui::Rect, egui::Rect, String)> {
    output
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::epaint::Shape::Text(text) => Some((
                clipped.clip_rect,
                text.visual_bounding_rect(),
                text.galley.text().to_owned(),
            )),
            _ => None,
        })
        .collect()
}
