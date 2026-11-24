mod settings_screen;

use mbv_client_core::{
    App, stub_app_with_playing_items, stub_app_with_playing_queue, stub_app_with_queue,
    stub_app_with_unsaved_queue, transport_client_capabilities,
};
use mbv_emby_model::TICKS_PER_SECOND;
use mbv_emby_model::test_support::make_item;
use mbv_gui::bridge::frame::GuiPanelToggles;
use mbv_gui::bridge::frame::chrome::GuiQueueFooter;
use mbv_gui::bridge::frame::overlay::GuiOverlayFrame;
use mbv_gui::bridge::frame::playback::{GuiQueuePlayback, GuiScrubberHover};
use mbv_gui::bridge::frame::queue::{
    GuiQueue, GuiQueueRow, GuiQueueScroll, GuiQueueSummary, GuiSemanticState,
};
use mbv_gui::bridge::frame::settings::GuiSettingsFrame;
use mbv_gui::bridge::frame::theme::GuiRole;
use mbv_gui::bridge::sections::{GuiSection, GuiSectionBatch};
use mbv_gui::gui_core::GuiCore;
use mbv_gui::gui_core::test_support::{stub_app_with_open_row_menu, stub_app_with_track_popover};
use mbv_gui::sections::{SectionLedger, SectionSources};
use mbv_queue::QueueItem;
use mbv_surfaces::queue::{QueueCursorUpdate, QueueSurface};
use mbv_surfaces::queue_playback::QueuePlaybackSurface;
use mbv_ui_model::queue_rows::queue_media_rows;

/// The Queue viewport height, in rows, that the frame tests paint with.
const VIEWPORT_ROWS: usize = 10;

/// The sections one ledger collect built (design D1). Each accessor reads the
/// section that owns the fact a test asserts.
struct Built(GuiSectionBatch);

/// One section's value. The first collect builds every section, so a missing
/// one is a ledger fault.
fn value<T>(section: Option<&GuiSection<T>>) -> &T {
    &section.expect("the ledger built the section").value
}

/// One optional section's value: a `null` section reads as `None`.
fn optional<T>(section: Option<&GuiSection<Option<T>>>) -> Option<&T> {
    section.and_then(|section| section.value.as_ref())
}

impl Built {
    fn queue(&self) -> &GuiQueue {
        value(self.0.queue.as_ref())
    }

    fn summary(&self) -> &GuiQueueSummary {
        value(self.0.summary.as_ref())
    }

    fn panel(&self) -> &GuiPanelToggles {
        value(self.0.panel.as_ref())
    }

    fn overlay(&self) -> Option<&GuiOverlayFrame> {
        optional(self.0.overlay.as_ref())
    }

    fn playback(&self) -> &GuiQueuePlayback {
        value(self.0.playback.as_ref())
    }

    fn hover(&self) -> Option<&GuiScrubberHover> {
        optional(self.0.hover.as_ref())
    }

    fn footer(&self) -> &GuiQueueFooter {
        value(self.0.footer.as_ref())
    }

    fn settings(&self) -> Option<&GuiSettingsFrame> {
        optional(self.0.settings.as_ref())
    }
}

/// The Queue Surface state after one tick: it adopts the projection's rows and
/// cursor, as the TUI's Queue component does.
fn adopted_queue(app: &mut App) -> QueueSurface {
    let projection = app.queue_surface_projection();
    let mut queue = QueueSurface::new();
    queue.list_mut().set_content(queue_media_rows(
        projection.slots.as_deref().unwrap_or_default(),
        projection.playback,
        projection.pending_slot,
    ));
    queue.set_cursor(&projection.cursor);
    queue
}

/// The sections one ledger collect builds for `app` (design D2). The first
/// collect holds nothing, and the gated sections carry their marks, so the
/// batch carries every section.
fn built(app: &mut App, queue: &QueueSurface, playback: &QueuePlaybackSurface) -> Built {
    let mut ledger = SectionLedger::default();
    ledger.mark_all_gated();
    let mut core = GuiCore::new(std::mem::replace(app, stub_app_with_queue(0)));
    // The wrapper owns its own Client state, and the test seeded and reads the
    // state beside the `App` (row 8.1 of `decouple-client-core`), so the
    // collect runs over the seeded one and publishes back what it left.
    core.take_seeded_gui_state_for_test();
    let batch = ledger.collect(SectionSources {
        core: &mut core,
        queue,
        playback_surface: playback,
        viewport_rows: VIEWPORT_ROWS,
        now: std::time::Instant::now(),
    });
    core.publish_seeded_gui_state_for_test();
    *app = core.into_app();
    Built(batch.expect("the first collect builds every section"))
}

/// The sections of `app` with `playback` as the Queue playback Surface state:
/// the variant the scrubber hover tests use.
fn frame_with_playback_surface(app: &mut App, playback: &QueuePlaybackSurface) -> Built {
    let queue = adopted_queue(app);
    built(app, &queue, playback)
}

/// The sections of `app` after one tick. The Queue Surface state adopts the
/// projection's rows and cursor, as the TUI's Queue component does.
fn frame_after_tick(app: &mut App) -> Built {
    let queue = adopted_queue(app);
    built(app, &queue, &QueuePlaybackSurface::new())
}

/// The sections of `app` after routing `keys` through the GUI Client's routing
/// entry with no surface leaf, as the Client core's core thread routes a
/// chord whose action the `App` applies itself.
fn frame_after_keys(app: &mut App, keys: &[mbv_ui_msg::Key]) -> Built {
    for code in keys {
        let key = mbv_ui_msg::KeyEvent::new(*code, mbv_ui_msg::KeyModifiers::NONE);
        let mut core = GuiCore::new(std::mem::replace(app, stub_app_with_queue(0)));
        core.take_seeded_gui_state_for_test();
        let _ = core.route_gui_input(mbv_gui::gui_core::GuiInput::Key(key), None);
        core.publish_seeded_gui_state_for_test();
        *app = core.into_app();
    }
    frame_after_tick(app)
}

/// A playing queue's frame carries its rows and the slot under the now-playing
/// marker. The stub numbers its slots from 1, and the first one plays.
#[test]
fn playing_queue_frame_carries_rows_and_active_slot() {
    let mut app = stub_app_with_playing_queue(3);
    let frame = frame_after_tick(&mut app);
    assert_eq!(frame.queue().rows.len(), 3);
    assert_eq!(frame.queue().active, Some(1));
}

/// The row ordinal is the slot's 1-based position in the canonical queue, not
/// its index in the painted viewport (audit/main.md 19): a viewport scrolled
/// to its second row still numbers the visible rows by queue position.
#[test]
fn a_scrolled_viewport_numbers_rows_by_queue_position() {
    let items: Vec<QueueItem> = (0..4)
        .map(|i| slot_item(&format!("Item {i}"), 10 * TICKS_PER_SECOND, 0, false))
        .collect();
    let mut app = stub_app_with_playing_items(items);
    let projection = app.queue_surface_projection();
    let mut queue = QueueSurface::new();
    queue.list_mut().set_content(queue_media_rows(
        projection.slots.as_deref().unwrap_or_default(),
        projection.playback,
        projection.pending_slot,
    ));
    queue.set_cursor(&QueueCursorUpdate::Set(2));

    let frame = GuiQueue::from_surface(&queue, 2, &app.queue_summary_projection(&[]));

    let painted: Vec<(&str, usize)> = frame
        .rows
        .iter()
        .filter_map(|row| match row {
            GuiQueueRow::Item {
                ordinal, primary, ..
            } => Some((primary.as_str(), *ordinal)),
            GuiQueueRow::Heading { .. } | GuiQueueRow::Spacer => None,
        })
        .collect();
    assert_eq!(painted, [("Item 1", 2), ("Item 2", 3)]);
}

/// The queue frame carries the list's scroll position while the rows outrun
/// the viewport (user request 2026-11-17): the window draws its scroll bar
/// from it. A viewport scrolled to its second row of four reads offset 1.
#[test]
fn a_long_queue_frame_carries_its_scroll_position() {
    let frame = scrolled_queue_frame(2);

    assert_eq!(
        frame.scroll,
        Some(GuiQueueScroll {
            offset: 1,
            visible: 2,
            total: 4,
        })
    );
}

/// A queue whose rows all fit the viewport carries no scroll position, so the
/// window paints no bar.
#[test]
fn a_queue_frame_that_fits_carries_no_scroll_position() {
    assert_eq!(scrolled_queue_frame(10).scroll, None);
}

/// The frame of a four-item queue with the cursor on its third item, painted
/// through a viewport `viewport_rows` tall.
fn scrolled_queue_frame(viewport_rows: usize) -> GuiQueue {
    let items: Vec<QueueItem> = (0..4)
        .map(|i| slot_item(&format!("Item {i}"), 10 * TICKS_PER_SECOND, 0, false))
        .collect();
    let mut app = stub_app_with_playing_items(items);
    let projection = app.queue_surface_projection();
    let mut queue = QueueSurface::new();
    queue.list_mut().set_content(queue_media_rows(
        projection.slots.as_deref().unwrap_or_default(),
        projection.playback,
        projection.pending_slot,
    ));
    queue.set_cursor(&QueueCursorUpdate::Set(2));
    GuiQueue::from_surface(&queue, viewport_rows, &app.queue_summary_projection(&[]))
}

/// An empty queue's frame carries no rows and no active slot, and its
/// summary counts no remaining slot and names no tail — no slot
/// carries the playhead, so no end time is anchored — and the Client-core
/// state the row 6.2 dispatches introduced rests at its defaults: the panel
/// expanded and unfolded, no filter, and no overlay.
#[test]
fn empty_queue_frame_carries_no_rows() {
    let mut app = stub_app_with_queue(0);
    let frame = frame_after_tick(&mut app);
    assert_eq!(frame.queue().rows.len(), 0);
    assert_eq!(frame.queue().active, None);
    assert_eq!(frame.summary().remaining, 0);
    assert_eq!(frame.summary().next_title, None);
    assert_eq!(frame.summary().tail_secs, None);
    assert_eq!(frame.summary().filter, "");
    assert!(!frame.summary().filter_active);
    assert!(!frame.panel().collapsed);
    assert!(!frame.panel().queue_folded);
    assert_eq!(frame.overlay(), None);
}

/// The summary names the slot after the playhead's title (review of
/// add-tauri-gui-client row 6.6, finding P2): the compact strip paints no
/// Queue list, so its next-item line cannot read `GuiQueue::rows` — the
/// datum comes from the queue itself. A playhead on the last slot has no
/// successor and names none. The stub numbers its slots from 1 and names
/// each item `Item <index>`, the first one playing.
#[test]
fn the_summary_names_the_slot_after_the_playhead_review_u66_p2() {
    let mut app = stub_app_with_playing_queue(3);
    let frame = frame_after_tick(&mut app);
    assert_eq!(frame.summary().next_title.as_deref(), Some("Item 1"));

    let mut app = stub_app_with_playing_queue(1);
    let frame = frame_after_tick(&mut app);
    assert_eq!(frame.summary().next_title, None);
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

/// A queue's rows carry their state (design D4): the active row marks
/// now-playing, a slot with a stored resume position marks its bounded
/// progress, and a played slot marks watched — while the queue's tail is the
/// last pending row's runtime past the playhead, and the scrubber's finish
/// clock is the active row's. The stub numbers its slots from 1 and the first
/// one plays.
///
/// The same stub queue drives the open row menu (the task row's "open
/// popover"), whose frame section carries the canvas's entries with their
/// key hints while the rows keep their state.
#[test]
fn playing_queue_frame_carries_row_states_finish_clocks_and_the_open_menu() {
    let minute = 60 * TICKS_PER_SECOND;
    let mut app = stub_app_with_open_row_menu(vec![
        slot_item("Playing", 20 * minute, 0, false),
        slot_item("Partial", 10 * minute, 3 * minute, false),
        slot_item("Watched", 10 * minute, 0, true),
    ]);
    let frame = frame_after_tick(&mut app);

    let states: Vec<&GuiSemanticState> = frame
        .queue()
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
            // The stub's playing row has no position yet, so no percentage.
            &GuiSemanticState::NowPlaying {
                tracks_progress: false
            },
            &GuiSemanticState::Active { progress: Some(30) },
            &GuiSemanticState::Played,
        ]
    );

    // The queue's tail is the sum of the pending runtimes after the playhead:
    // the partial row's 10 minutes and the watched row's 10 minutes.
    assert_eq!(frame.summary().tail_secs, Some(20 * 60));

    assert_eq!(frame.summary().remaining, 2);

    // The open row menu rides the same frame: the row it opened on, and the
    // canvas's four entries, each with the key hint that selects it
    // directly.
    let mbv_gui::bridge::frame::overlay::GuiOverlayFrame::RowMenu {
        anchor_slot,
        cursor,
        entries,
    } = frame.overlay().expect("the row menu opened")
    else {
        panic!("the overlay is the row menu");
    };
    // The stub's queue is numbered from 1 and the menu opened over its first
    // row.
    assert_eq!(*anchor_slot, 1);
    assert_eq!(*cursor, 0);
    let labels: Vec<&str> = entries.iter().map(|entry| entry.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Play now",
            "Mark watched",
            "Go to episode",
            "Remove from queue",
            "Clear queue"
        ]
    );
    let hints: Vec<Option<&str>> = entries
        .iter()
        .map(|entry| entry.key_hint.as_deref())
        .collect();
    assert_eq!(
        hints,
        [Some("Enter"), Some("w"), Some("g"), Some("dd"), Some("c")]
    );
}

/// A footer pill's text is glyph-free (design D8): no Nerd Font private-use
/// codepoint and no emoji fallback reaches the window. The stub's Nerd
/// Font setting is off, so the projection's composed text carries the emoji
/// fallbacks the frame must drop, and the unsaved queue's dirty pill carries
/// its word. The footer carries no Player-owner pill (gui-named-queues task
/// 8.5, design D9).
#[test]
fn footer_pill_text_carries_no_glyph() {
    let mut app = stub_app_with_unsaved_queue(1);
    let frame = frame_after_tick(&mut app);
    assert!(!frame.footer().playlist.text.chars().any(is_glyph));
    assert_eq!(
        frame.footer().dirty.as_ref().map(|pill| pill.text.as_str()),
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

/// The stub player is audio-only with no codec, so the frame's cluster
/// collapses to the unresolved resolution label in the muted role: the
/// frame carries the cluster as the queue band paints it, audio and
/// subtitles dropped.
#[test]
fn playing_queue_frame_carries_the_status_indicator_cluster() {
    let mut app = stub_app_with_playing_queue(3);
    let frame = frame_after_tick(&mut app);

    let items = frame
        .playback()
        .status_indicators
        .clone()
        .expect("the playing frame carries the cluster");
    assert_eq!(items.len(), 1, "audio-only playback collapses the cluster");
    assert_eq!(items[0].label, "--");
    assert_eq!(items[0].role, GuiRole::TextMuted);
}

/// The queue filter narrows the projected rows only (add-tauri-gui-client
/// row 6.2): the frame's rows are the title matches under the library's one
/// acceptance rule, the summary carries the typed text and its caret fact,
/// and the canonical queue keeps every slot — the filter never edits it.
#[test]
fn queue_filter_narrows_projected_rows_only() {
    let mut app = stub_app_with_playing_queue(3);
    app.set_client_capabilities_for_test(transport_client_capabilities());
    // The stub titles are "Item 0", "Item 1", "Item 2": the query matches
    // one row.
    let frame = frame_after_keys(
        &mut app,
        &[mbv_ui_msg::Key::Char('/'), mbv_ui_msg::Key::Char('1')],
    );

    assert_eq!(frame.summary().filter, "1");
    assert!(frame.summary().filter_active);
    let slots: Vec<u64> = frame
        .queue()
        .rows
        .iter()
        .filter_map(|row| match row {
            GuiQueueRow::Item { slot, .. } => Some(*slot),
            GuiQueueRow::Heading { .. } | GuiQueueRow::Spacer => None,
        })
        .collect();
    assert_eq!(slots, [2]);
    // The filter's narrowing is the projection's: the summary still counts
    // both slots after the playhead, so no Owner edit rode the typed
    // characters — the canonical queue keeps its three slots.
    assert_eq!(frame.summary().remaining, 2);
}

/// An open track popover's frame section carries the chip's choices with
/// their chosen marks (add-tauri-gui-client row 6.2): the subtitle popover
/// lists Off first and marks it while subtitles are off.
#[test]
fn track_popover_frame_carries_choices_with_the_chosen_mark() {
    let mut app = stub_app_with_track_popover(mbv_ui_msg::TrackChipKind::Subtitles);
    let frame = frame_after_tick(&mut app);

    let mbv_gui::bridge::frame::overlay::GuiOverlayFrame::TrackPopover {
        chip,
        cursor,
        choices,
    } = frame.overlay().expect("the popover is open")
    else {
        panic!("the overlay is the track popover");
    };
    assert_eq!(
        *chip,
        mbv_gui::bridge::frame::overlay::GuiTrackChipKind::Subtitles
    );
    assert_eq!(*cursor, 0, "the cursor rests on the chosen entry");
    assert_eq!(
        choices.clone(),
        vec![
            mbv_gui::bridge::frame::overlay::GuiTrackPopoverChoice {
                id: 0,
                label: "Off".to_string(),
                forced: false,
                chosen: true,
            },
            mbv_gui::bridge::frame::overlay::GuiTrackPopoverChoice {
                id: 2,
                label: "Forced".to_string(),
                forced: true,
                chosen: false,
            },
        ]
    );
}

/// The same popover over the audio chip lists the audio tracks with the
/// playing one marked, and no Off entry — the audio chip has no off state.
#[test]
fn track_popover_frame_marks_the_playing_audio_track() {
    let mut app = stub_app_with_track_popover(mbv_ui_msg::TrackChipKind::Audio);
    let frame = frame_after_tick(&mut app);

    let mbv_gui::bridge::frame::overlay::GuiOverlayFrame::TrackPopover {
        choices, cursor, ..
    } = frame.overlay().expect("the popover is open")
    else {
        panic!("the overlay is the track popover");
    };
    assert_eq!(*cursor, 0);
    assert_eq!(
        choices.clone(),
        vec![mbv_gui::bridge::frame::overlay::GuiTrackPopoverChoice {
            id: 1,
            label: "Stereo".to_string(),
            forced: false,
            chosen: true,
        }]
    );
}

/// The scrubber's hover preview rides the frame (add-tauri-gui-client row
/// 6.4, design D4): the Surface state's hover fraction projects with the
/// time it names at the position.
#[test]
fn the_scrubber_hover_projects_its_time() {
    let item = slot_item("Playing", 20 * 60 * TICKS_PER_SECOND, 0, false);
    let mut app = stub_app_with_playing_items(vec![item]);
    let mut playback = QueuePlaybackSurface::new();
    playback.set_scrubber_hover(Some(0.75));

    let frame = frame_with_playback_surface(&mut app, &playback);

    let hover = frame.hover().expect("the frame carries the hover");
    assert_eq!(hover.fraction, 0.75);
    // The frame carries the hover's time formatted by the TUI's shared
    // duration formatter, not the raw ticks.
    assert_eq!(hover.time_label, "15:00");
}

/// The hover preview names the time under the pointer, and it carries no tile
/// while the App holds no cached sheet for that position (task 6.7). A pointer
/// off the bar carries no hover at all.
#[test]
fn the_scrubber_hover_names_its_time_without_a_cached_sheet() {
    let item = slot_item("Playing", 20 * 60 * TICKS_PER_SECOND, 0, false);
    let mut app = stub_app_with_playing_items(vec![item]);
    let mut playback = QueuePlaybackSurface::new();
    playback.set_scrubber_hover(Some(0.25));
    let queue = adopted_queue(&mut app);
    let hover = built(&mut app, &queue, &playback)
        .hover()
        .cloned()
        .expect("the hover rides the section");
    assert_eq!(hover.fraction, 0.25);

    let away = QueuePlaybackSurface::new();
    assert!(built(&mut app, &queue, &away).hover().is_none());
}
