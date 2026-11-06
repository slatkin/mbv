use mbv::{
    App, stub_app_with_playing_items, stub_app_with_playing_queue, stub_app_with_queue,
    stub_app_with_unsaved_queue,
};
use mbv_emby_model::TICKS_PER_SECOND;
use mbv_emby_model::test_support::make_item;
use mbv_gui::bridge::frame::queue::{GuiQueueRow, GuiRightColumn, GuiSemanticState};
use mbv_gui::bridge::frame::theme::GuiRole;
use mbv_gui::bridge::frame::{FrameSources, GuiFrame};
use mbv_queue::QueueItem;
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

/// An empty queue's frame carries no rows and no active slot, and its
/// summary counts no remaining slot and names no finish clock — no slot
/// carries the playhead, so no end time is anchored.
#[test]
fn empty_queue_frame_carries_no_rows() {
    let mut app = stub_app_with_queue(0);
    let frame = frame_after_tick(&mut app);
    assert_eq!(frame.queue.rows.len(), 0);
    assert_eq!(frame.queue.active, None);
    assert_eq!(frame.summary.remaining, 0);
    assert_eq!(frame.summary.finish_clock, None);
    assert_eq!(frame.summary.right_column, GuiRightColumn::Length);
    assert_eq!(frame.playback.finish_clock, None);
}

/// One Emby movie slot with its own stored facts: the runtime, the resume
/// position, and the played flag a row's state and clock read.
fn slot_item(name: &str, runtime_ticks: i64, position_ticks: i64, played: bool) -> QueueItem {
    let mut item = make_item(name, "Movie");
    item.runtime_ticks = runtime_ticks;
    item.playback_position_ticks = position_ticks;
    item.played = played;
    QueueItem::Emby(Box::new(item))
}

/// A queue's rows carry their state and both right-column values (design
/// D4): the active row marks now-playing, a slot with a stored resume
/// position marks its bounded progress, a played slot marks watched — and
/// every pending row carries its "Ends at" clock, the queue's finish clock
/// is the last pending row's, and the scrubber's finish clock is the active
/// row's. The stub numbers its slots from 1 and the first one plays.
#[test]
fn playing_queue_frame_carries_row_states_and_finish_clocks() {
    let minute = 60 * TICKS_PER_SECOND;
    let mut app = stub_app_with_playing_items(vec![
        slot_item("Playing", 20 * minute, 0, false),
        slot_item("Partial", 10 * minute, 3 * minute, false),
        slot_item("Watched", 10 * minute, 0, true),
    ]);
    let frame = frame_after_tick(&mut app);

    let states: Vec<&GuiSemanticState> = frame
        .queue
        .rows
        .iter()
        .map(|row| match row {
            GuiQueueRow::Item { state, .. } => state,
            GuiQueueRow::Heading { .. } | GuiQueueRow::Spacer => {
                panic!("the stub queue has no structural rows")
            }
        })
        .collect();
    assert_eq!(
        states,
        [
            &GuiSemanticState::NowPlaying { progress: None },
            &GuiSemanticState::Active { progress: Some(30) },
            &GuiSemanticState::Played,
        ]
    );

    let ends_at: Vec<Option<&str>> = frame
        .queue
        .rows
        .iter()
        .map(|row| match row {
            GuiQueueRow::Item { ends_at, .. } => ends_at.as_deref(),
            GuiQueueRow::Heading { .. } | GuiQueueRow::Spacer => None,
        })
        .collect();
    let (active_clock, partial_clock, watched_clock) = match ends_at.as_slice() {
        [Some(active), Some(partial), Some(watched)] => (*active, *partial, *watched),
        _ => panic!("every pending row carries its finish clock"),
    };
    // The queue's finish clock is the last pending row's; the scrubber's is
    // the active row's.
    assert_eq!(frame.summary.finish_clock.as_deref(), Some(watched_clock));
    assert_eq!(frame.playback.finish_clock.as_deref(), Some(active_clock));
    // The clocks chain: each pending row ends one runtime after the row
    // before it, so the partial row's clock differs from the active row's.
    assert_ne!(partial_clock, active_clock);

    assert_eq!(frame.summary.remaining, 2);
    assert_eq!(frame.summary.right_column, GuiRightColumn::Length);
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

/// A playing queue's frame alternates its item rows through the zebra walk
/// the TUI's wide list paints (`media_list/wide.rs`): an ungrouped list opens
/// on the surface fill and flips per item row, so the parity rides each row
/// and stays stable when the viewport moves.
#[test]
fn playing_queue_frame_carries_the_zebra_parity() {
    let mut app = stub_app_with_playing_queue(3);
    let frame = frame_after_tick(&mut app);

    let striped: Vec<bool> = frame
        .queue
        .rows
        .iter()
        .map(|row| match row {
            GuiQueueRow::Item { striped, .. } => *striped,
            GuiQueueRow::Heading { .. } | GuiQueueRow::Spacer => {
                panic!("the stub queue has no structural rows")
            }
        })
        .collect();
    assert_eq!(striped, [false, true, false]);
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

/// The stub player is audio-only with no codec, so the frame's cluster
/// collapses to the unresolved resolution label in the muted role: the
/// frame carries the cluster as the queue band paints it, audio and
/// subtitles dropped.
#[test]
fn playing_queue_frame_carries_the_status_indicator_cluster() {
    let mut app = stub_app_with_playing_queue(3);
    let frame = frame_after_tick(&mut app);

    let items = frame
        .playback
        .status_indicators
        .expect("the playing frame carries the cluster");
    assert_eq!(items.len(), 1, "audio-only playback collapses the cluster");
    assert_eq!(items[0].label, "--");
    assert_eq!(items[0].role, GuiRole::TextMuted);
}
