use mbv::{App, stub_app_with_playing_queue, stub_app_with_queue, stub_app_with_unsaved_queue};
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
        artwork: None,
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

/// A footer pill's text is glyph-free (design D8): no Nerd Font private-use
/// codepoint and no emoji fallback reaches the front end. The stub's Nerd
/// Font setting is off, so the projection's composed text carries the emoji
/// fallbacks the frame must drop, and the unsaved queue's dirty pill carries
/// its word.
#[test]
fn footer_pill_text_carries_no_glyph() {
    let mut app = stub_app_with_unsaved_queue(1);
    let frame = frame_after_tick(&mut app);
    assert!(!frame.footer.owner.text.chars().any(is_glyph));
    assert!(!frame.footer.playlist.text.chars().any(is_glyph));
    assert_eq!(
        frame.footer.dirty.as_ref().map(|pill| pill.text.as_str()),
        Some("UNSAVED")
    );
}

/// A glyph, as design D8 forbids in the GUI: a Nerd Font private-use
/// codepoint (the BMP and the two supplementary private-use planes) or one of
/// the projection's emoji fallbacks.
fn is_glyph(c: char) -> bool {
    matches!(c, '\u{1F5A7}' | '\u{1F5AD}')
        || ('\u{E000}'..='\u{F8FF}').contains(&c)
        || ('\u{F0000}'..='\u{FFFFD}').contains(&c)
        || ('\u{100000}'..='\u{10FFFD}').contains(&c)
}
