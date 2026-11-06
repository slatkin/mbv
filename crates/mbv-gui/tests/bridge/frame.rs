use mbv::{App, stub_app_with_playing_queue, stub_app_with_queue};
use mbv_gui::bridge::frame::{FrameSources, GuiFrame};
use mbv_surfaces::FocusStack;
use mbv_surfaces::breakpoint::BreakpointClass;
use mbv_surfaces::queue::QueueSurface;
use mbv_ui_model::queue_rows::queue_media_rows;

/// The Queue viewport height, in rows, that the frame tests paint with.
const VIEWPORT_ROWS: usize = 10;

/// The frame of `app` after one tick. The Queue Surface state adopts the
/// projection's rows and cursor, as the TUI's Queue component does.
fn frame_after_tick(app: &mut App) -> GuiFrame {
    let projection = app.queue_surface_projection();
    let mut queue = QueueSurface::new();
    queue.list_mut().set_content(queue_media_rows(
        projection.slots.as_deref().unwrap_or_default(),
        projection.playback,
        projection.pending_slot,
    ));
    queue.set_cursor(&projection.cursor);
    GuiFrame::build(&FrameSources {
        app,
        focus: &FocusStack::new(),
        queue: &queue,
        viewport_rows: VIEWPORT_ROWS,
        breakpoint: BreakpointClass::Wide,
        theme_changed: false,
    })
}

/// A playing queue's frame carries its rows and the slot under the now-playing
/// marker. The stub numbers its slots from 1, and the first one plays.
#[test]
fn playing_queue_frame_carries_rows_and_active_slot() {
    let mut app = stub_app_with_playing_queue(3);
    let frame = frame_after_tick(&mut app);
    assert_eq!(frame.queue.rows.len(), 3);
    assert_eq!(frame.queue.active, Some(1));
}

/// An empty queue's frame carries no rows and no active slot.
#[test]
fn empty_queue_frame_carries_no_rows() {
    let mut app = stub_app_with_queue(0);
    let frame = frame_after_tick(&mut app);
    assert_eq!(frame.queue.rows.len(), 0);
    assert_eq!(frame.queue.active, None);
}
