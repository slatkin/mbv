use mbv::{
    App, PlaybackChapter, PlaybackDecorations, SegmentMark, TrickplayGrid,
    stub_app_with_open_row_menu, stub_app_with_playing_items, stub_app_with_playing_queue,
    stub_app_with_queue, stub_app_with_track_popover, stub_app_with_unsaved_queue,
};
use mbv_emby_model::TICKS_PER_SECOND;
use mbv_emby_model::test_support::make_item;
use mbv_gui::bridge::frame::queue::{GuiQueue, GuiQueueRow, GuiRightColumn, GuiSemanticState};
use mbv_gui::bridge::frame::theme::GuiRole;
use mbv_gui::bridge::frame::{FrameSources, GuiFrame};
use mbv_queue::{QueueItem, QueueSlotId};
use mbv_surfaces::FocusStack;
use mbv_surfaces::queue::{QueueCursorUpdate, QueueSurface};
use mbv_surfaces::queue_playback::QueuePlaybackSurface;
use mbv_ui_model::queue_rows::queue_media_rows;

/// The Queue viewport height, in rows, that the frame tests paint with.
const VIEWPORT_ROWS: usize = 10;

/// The frame of `app` after one tick, with `playback` as the Queue
/// playback Surface state — the variant `frame_after_tick` uses when the
/// scrubber hover matters.
fn frame_with_playback_surface(app: &mut App, playback: &QueuePlaybackSurface) -> GuiFrame {
    let projection = app.queue_surface_projection();
    let mut queue = QueueSurface::new();
    queue.list_mut().set_content(queue_media_rows(
        projection.slots.as_deref().unwrap_or_default(),
        projection.playback,
        projection.pending_slot,
    ));
    queue.set_cursor(&projection.cursor);
    let playback_projection = app.queue_playback_projection();
    GuiFrame::build(&FrameSources {
        app,
        focus: &FocusStack::new(),
        queue: &queue,
        playback,
        playback_projection: &playback_projection,
        hover_tile: None,
        viewport_rows: VIEWPORT_ROWS,
        theme_changed: false,
        artwork: None,
        library: None,
    })
}

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
    let playback_projection = app.queue_playback_projection();
    GuiFrame::build(&FrameSources {
        app,
        focus: &FocusStack::new(),
        queue: &queue,
        playback: &QueuePlaybackSurface::new(),
        playback_projection: &playback_projection,
        hover_tile: None,
        viewport_rows: VIEWPORT_ROWS,
        theme_changed: false,
        artwork: None,
        library: None,
    })
}

/// The frame of `app` after routing `keys` through the GUI Client's routing
/// entry with no surface leaf, as the Client core's core thread routes a
/// chord whose action the `App` applies itself.
fn frame_after_keys(app: &mut App, keys: &[mbv_ui_msg::Key]) -> GuiFrame {
    for code in keys {
        let key = mbv_ui_msg::KeyEvent::new(*code, mbv_ui_msg::KeyModifiers::NONE);
        let _ = app.route_gui_input(mbv::GuiInput::Key(key), None);
    }
    frame_after_tick(app)
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

    let frame = GuiQueue::from_surface(&queue, 2, &app.queue_summary_projection());

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

/// An empty queue's frame carries no rows and no active slot, and its
/// summary counts no remaining slot and names no finish clock — no slot
/// carries the playhead, so no end time is anchored — and the Client-core
/// state the row 6.2 dispatches introduced rests at its defaults: the panel
/// expanded and unfolded, the length column, no filter, and no overlay.
#[test]
fn empty_queue_frame_carries_no_rows() {
    let mut app = stub_app_with_queue(0);
    let frame = frame_after_tick(&mut app);
    assert_eq!(frame.queue.rows.len(), 0);
    assert_eq!(frame.queue.active, None);
    assert_eq!(frame.summary.remaining, 0);
    assert_eq!(frame.summary.next_title, None);
    assert_eq!(frame.summary.finish_clock, None);
    assert_eq!(frame.summary.right_column, GuiRightColumn::Length);
    assert_eq!(frame.summary.filter, "");
    assert!(!frame.summary.filter_active);
    assert!(!frame.panel.collapsed);
    assert!(!frame.panel.queue_folded);
    assert_eq!(frame.overlay, None);
    assert_eq!(frame.playback.finish_clock, None);
    assert_eq!(frame.playback.chapters, Vec::new());
    assert_eq!(frame.playback.intro_mark, None);
    assert_eq!(frame.playback.credits_mark, None);
    assert_eq!(frame.playback.trickplay, None);
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
    assert_eq!(frame.summary.next_title.as_deref(), Some("Item 1"));

    let mut app = stub_app_with_playing_queue(1);
    let frame = frame_after_tick(&mut app);
    assert_eq!(frame.summary.next_title, None);
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

    // The open row menu rides the same frame: the row it opened on, and the
    // canvas's five entries, each with the key hint that selects it
    // directly.
    let mbv_gui::bridge::frame::overlay::GuiOverlayFrame::RowMenu {
        anchor_slot,
        cursor,
        entries,
    } = frame.overlay.expect("the row menu opened")
    else {
        panic!("the overlay is the row menu");
    };
    // The stub's queue is numbered from 1 and the menu opened over its first
    // row.
    assert_eq!(anchor_slot, 1);
    assert_eq!(cursor, 0);
    let labels: Vec<&str> = entries.iter().map(|entry| entry.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Play now",
            "Play next",
            "Mark watched",
            "Go to episode",
            "Remove from queue"
        ]
    );
    let hints: Vec<Option<&str>> = entries
        .iter()
        .map(|entry| entry.key_hint.as_deref())
        .collect();
    assert_eq!(
        hints,
        [Some("Enter"), Some("n"), Some("w"), Some("g"), Some("dd")]
    );
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

/// The queue filter narrows the projected rows only (add-tauri-gui-client
/// row 6.2): the frame's rows are the title matches under the library's one
/// acceptance rule, the summary carries the typed text and its caret fact,
/// and the canonical queue keeps every slot — the filter never edits it.
#[test]
fn queue_filter_narrows_projected_rows_only() {
    let mut app = stub_app_with_playing_queue(3);
    // The stub titles are "Item 0", "Item 1", "Item 2": the query matches
    // one row.
    let frame = frame_after_keys(
        &mut app,
        &[mbv_ui_msg::Key::Char('/'), mbv_ui_msg::Key::Char('1')],
    );

    assert_eq!(frame.summary.filter, "1");
    assert!(frame.summary.filter_active);
    let slots: Vec<u64> = frame
        .queue
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
    assert_eq!(frame.summary.remaining, 2);
}

/// An open track popover's frame section carries the chip's choices with
/// their chosen marks (add-tauri-gui-client row 6.2): the subtitle popover
/// lists Off first and marks it while subtitles are off.
#[test]
fn track_popover_frame_carries_choices_with_the_chosen_mark() {
    let mut app = stub_app_with_track_popover(mbv::TrackChipKind::Subtitles);
    let frame = frame_after_tick(&mut app);

    let mbv_gui::bridge::frame::overlay::GuiOverlayFrame::TrackPopover {
        chip,
        cursor,
        choices,
    } = frame.overlay.expect("the popover is open")
    else {
        panic!("the overlay is the track popover");
    };
    assert_eq!(
        chip,
        mbv_gui::bridge::frame::overlay::GuiTrackChipKind::Subtitles
    );
    assert_eq!(cursor, 0, "the cursor rests on the chosen entry");
    assert_eq!(
        choices,
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
    let mut app = stub_app_with_track_popover(mbv::TrackChipKind::Audio);
    let frame = frame_after_tick(&mut app);

    let mbv_gui::bridge::frame::overlay::GuiOverlayFrame::TrackPopover {
        choices, cursor, ..
    } = frame.overlay.expect("the popover is open")
    else {
        panic!("the overlay is the track popover");
    };
    assert_eq!(cursor, 0);
    assert_eq!(
        choices,
        vec![mbv_gui::bridge::frame::overlay::GuiTrackPopoverChoice {
            id: 1,
            label: "Stereo".to_string(),
            forced: false,
            chosen: true,
        }]
    );
}

/// The active item's metadata decoration reaches the frame (design D4a): a
/// seeded cache entry's chapters, intro and credits marks, and trickplay
/// source land in the playback section the scrubber reads.
#[test]
fn playing_queue_frame_carries_the_items_decorations() {
    let item = slot_item("Playing", 20 * 60 * TICKS_PER_SECOND, 0, false);
    let key = item.content_id();
    let mut app = stub_app_with_playing_items(vec![item]);
    app.seed_playback_metadata_for_test(
        key,
        PlaybackDecorations {
            chapters: vec![PlaybackChapter {
                name: "Opening".to_string(),
                start_ticks: 0,
            }],
            intro: Some(SegmentMark {
                start_ticks: 100,
                end_ticks: 200,
            }),
            credits: Some(SegmentMark {
                start_ticks: 300,
                end_ticks: 400,
            }),
            trickplay: Some(vec![TrickplayGrid {
                width: 854,
                height: 480,
                tile_width: 10,
                tile_height: 10,
                thumbnail_count: 100,
                interval_ms: 10_000,
                media_source_id: "ms1".to_string(),
            }]),
        },
    );

    let frame = frame_after_tick(&mut app);

    assert_eq!(
        frame.playback.chapters,
        vec![mbv_gui::bridge::frame::playback::GuiChapter {
            name: "Opening".to_string(),
            start_ticks: 0,
        }]
    );
    assert_eq!(
        frame.playback.intro_mark,
        Some(mbv_gui::bridge::frame::playback::GuiSegmentMark {
            start_ticks: 100,
            end_ticks: 200,
        })
    );
    assert_eq!(
        frame.playback.credits_mark,
        Some(mbv_gui::bridge::frame::playback::GuiSegmentMark {
            start_ticks: 300,
            end_ticks: 400,
        })
    );
    assert_eq!(
        frame.playback.trickplay,
        Some(vec![mbv_gui::bridge::frame::playback::GuiTrickplayGrid {
            width: 854,
            height: 480,
            tile_width: 10,
            tile_height: 10,
            thumbnail_count: 100,
            interval_ms: 10_000,
            media_source_id: "ms1".to_string(),
        }])
    );
}

/// The scrubber's hover preview rides the frame (add-tauri-gui-client row
/// 6.4, design D4): the Surface state's hover fraction projects with the
/// time and the covering chapter's name and ordinal at the position — the
/// canvas preview label's `Ch. 2` form (`main` audit 12).
#[test]
fn the_scrubber_hover_projects_its_time_and_chapter() {
    let item = slot_item("Playing", 20 * 60 * TICKS_PER_SECOND, 0, false);
    let key = item.content_id();
    let mut app = stub_app_with_playing_items(vec![item]);
    app.seed_playback_metadata_for_test(
        key,
        PlaybackDecorations {
            chapters: vec![
                PlaybackChapter {
                    name: "Opening".to_string(),
                    start_ticks: 0,
                },
                PlaybackChapter {
                    name: "Middle".to_string(),
                    start_ticks: 10 * 60 * TICKS_PER_SECOND,
                },
            ],
            ..PlaybackDecorations::default()
        },
    );
    let mut playback = QueuePlaybackSurface::new();
    playback.set_scrubber_hover(Some(0.75));

    let frame = frame_with_playback_surface(&mut app, &playback);

    let hover = frame.playback.hover.expect("the frame carries the hover");
    assert_eq!(hover.fraction, 0.75);
    // The frame carries the hover's time formatted by the TUI's shared
    // duration formatter, not the raw ticks.
    assert_eq!(hover.time_label, "15:00");
    assert_eq!(hover.chapter.as_deref(), Some("Middle"));
    assert_eq!(hover.chapter_number, Some(2));
}

/// The drag ghost carries the dragged row's Ends-at value (review of
/// add-tauri-gui-client row 6.5, finding P2-3): in Ends-at mode the ghost
/// paints the same right column as the row it lifts.
#[test]
fn a_drag_ghost_carries_its_rows_ends_at_clock_review_u65_p2_3() {
    let minute = 60 * TICKS_PER_SECOND;
    let mut app = stub_app_with_playing_items(vec![
        slot_item("Playing", 20 * minute, 0, false),
        slot_item("Pending", 10 * minute, 0, false),
        slot_item("Later", 10 * minute, 0, false),
    ]);
    let _ = app.route_gui_input(
        mbv::GuiInput::Key(mbv_ui_msg::KeyEvent::new(
            mbv_ui_msg::Key::Char('t'),
            mbv_ui_msg::KeyModifiers::NONE,
        )),
        None,
    );

    let projection = app.queue_surface_projection();
    let mut queue = QueueSurface::new();
    queue.list_mut().set_content(queue_media_rows(
        projection.slots.as_deref().unwrap_or_default(),
        projection.playback,
        projection.pending_slot,
    ));
    queue.set_cursor(&projection.cursor);
    assert!(queue.begin_row_drag(QueueSlotId::from_raw(2)));

    let summary = app.queue_summary_projection();
    assert_eq!(
        GuiRightColumn::from(summary.right_column),
        GuiRightColumn::EndsAt,
        "the frame paints the Ends-at column"
    );
    let frame = GuiQueue::from_surface(&queue, VIEWPORT_ROWS, &summary);
    let row_ends = frame.rows.iter().find_map(|row| match row {
        GuiQueueRow::Item {
            slot: 2, ends_at, ..
        } => ends_at.clone(),
        _ => None,
    });
    assert!(row_ends.is_some(), "the pending row carries its end clock");
    let drag = frame.drag.expect("the frame carries the drag");
    let ghost_ends = match drag.ghost {
        GuiQueueRow::Item { ends_at, .. } => ends_at,
        GuiQueueRow::Heading { .. } | GuiQueueRow::Spacer => panic!("a drag holds an item row"),
    };
    assert_eq!(ghost_ends, row_ends);
}

/// The hover preview carries the trickplay tile the core resolved (task 6.7):
/// the tile travels inside the hover datum, so a frame whose pointer leaves the
/// bar carries no tile at all — no second "now empty" flag is needed.
#[test]
fn the_scrubber_hover_carries_the_cores_trickplay_tile() {
    use mbv_gui::bridge::frame::playback::GuiScrubberTile;

    let item = slot_item("Playing", 20 * 60 * TICKS_PER_SECOND, 0, false);
    let mut app = stub_app_with_playing_items(vec![item]);
    let mut playback = QueuePlaybackSurface::new();
    playback.set_scrubber_hover(Some(0.25));
    let tile = GuiScrubberTile {
        sheet_key: "item:320:0".to_string(),
        width: 320,
        height: 180,
        columns: 10,
        column: 2,
        row: 1,
        sheet: Some("data:image/jpeg;base64,anBlZw==".to_string()),
    };
    let queue_projection = app.queue_surface_projection();
    let mut queue = QueueSurface::new();
    queue.list_mut().set_content(queue_media_rows(
        queue_projection.slots.as_deref().unwrap_or_default(),
        queue_projection.playback,
        queue_projection.pending_slot,
    ));
    queue.set_cursor(&queue_projection.cursor);
    let playback_projection = app.queue_playback_projection();
    let frame = GuiFrame::build(&FrameSources {
        app: &app,
        focus: &FocusStack::new(),
        queue: &queue,
        playback: &playback,
        playback_projection: &playback_projection,
        hover_tile: Some(tile.clone()),
        viewport_rows: VIEWPORT_ROWS,
        theme_changed: false,
        artwork: None,
        library: None,
    });

    let hover = frame.playback.hover.expect("the frame carries the hover");
    assert_eq!(hover.tile, Some(tile));

    // The pointer leaves the bar: the hover, and its tile, go together.
    let mut away = QueuePlaybackSurface::new();
    away.set_scrubber_hover(None);
    let frame = GuiFrame::build(&FrameSources {
        app: &app,
        focus: &FocusStack::new(),
        queue: &queue,
        playback: &away,
        playback_projection: &playback_projection,
        hover_tile: None,
        viewport_rows: VIEWPORT_ROWS,
        theme_changed: false,
        artwork: None,
        library: None,
    });
    assert!(frame.playback.hover.is_none());
}
