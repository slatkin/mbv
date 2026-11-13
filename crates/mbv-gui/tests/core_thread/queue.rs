//! The canvas queue's row contracts (add-tauri-gui-client row 6.5, design
//! D5): the hover that paints the row chrome, the grip's drag (the drop index
//! from a pointer move, the ghost and insertion line, one move request on
//! release, none on a cancel), and the row menu (its actions button, and an
//! entry press executing through the Client core).

use mbv::stub_app_with_queue_and_cmd_rx;
use mbv_gui::bridge::frame::overlay::GuiOverlayFrame;
use mbv_gui::bridge::frame::queue::{GuiQueueInsertion, GuiQueueRow};
use mbv_gui::bridge::layout::{QueueTargetId, RegionTarget};
use mbv_gui::core_thread::CoreInput;
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers, PointerEvent, PointerKind, Position};

use crate::{measured_layout, move_at, press_at, region_at, test_core, test_core_with};

/// A pointer move with the primary button held (the neutral drag), as the
/// translated `pointer` command carries it.
fn drag_to(x: f32, y: f32) -> CoreInput {
    CoreInput::Pointer(PointerEvent::new(
        PointerKind::Drag,
        Position { x, y },
        KeyModifiers::NONE,
    ))
}

/// A cancelled gesture, as the front end sends it when the webview loses the
/// pointer (add-tauri-gui-client row 6.5, finding P1-2). The position is the
/// neutral placeholder the bridge sends for a window blur.
fn cancel_pointer() -> CoreInput {
    CoreInput::Pointer(PointerEvent::new(
        PointerKind::Cancel,
        Position { x: 0.0, y: 0.0 },
        KeyModifiers::NONE,
    ))
}

/// The primary button's release, as the translated `pointer` command carries
/// it.
fn release_at(x: f32, y: f32) -> CoreInput {
    CoreInput::Pointer(PointerEvent::new(
        PointerKind::Release,
        Position { x, y },
        KeyModifiers::NONE,
    ))
}

/// A layout report with four queue rows, the first row's grip painted over
/// it: the canvas's hovered row reports its chrome as its own regions.
fn rows_with_first_grip() -> CoreInput {
    measured_layout(
        4,
        vec![
            region_at(0.0, 0.0, RegionTarget::Queue(QueueTargetId::Slot(1))),
            region_at(0.0, 24.0, RegionTarget::Queue(QueueTargetId::Slot(2))),
            region_at(0.0, 48.0, RegionTarget::Queue(QueueTargetId::Slot(3))),
            region_at(0.0, 72.0, RegionTarget::Queue(QueueTargetId::Slot(4))),
            region_at(0.0, 0.0, RegionTarget::Queue(QueueTargetId::RowGrip(1))),
        ],
    )
}

/// The slot moves the Owner received, as `(slot_id, to_index)`.
fn slot_moves(cmd_rx: &std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) -> Vec<(u64, usize)> {
    cmd_rx
        .try_iter()
        .filter_map(|cmd| match cmd {
            mbv_ctrl::CtrlCmd::UnifiedQueueMoveSlot {
                slot_id, to_index, ..
            } => Some((slot_id, to_index)),
            _ => None,
        })
        .collect()
}

/// The row hover the frame carries: the row under the pointer, its chrome
/// region included, and `None` when the pointer leaves the rows (add-tauri-
/// gui-client row 6.5: the frame paints the hovered row's grip and actions
/// button).
#[test]
fn the_frame_carries_the_hovered_row() {
    let mut test = test_core(3);
    test.tx.send(rows_with_first_grip()).unwrap();
    assert!(test.core.step());
    test.held();

    // A move over the second row's region.
    test.tx.send(move_at(10.0, 30.0)).unwrap();
    assert!(test.core.step());
    assert_eq!(test.held().queue().hover, Some(2));

    // The report the hovered row's own chrome regions cause keeps the hover:
    // clearing it dropped the chrome the report measured, so it flickered on
    // every move.
    test.tx.send(rows_with_first_grip()).unwrap();
    assert!(test.core.step());
    assert_eq!(test.held().queue().hover, Some(2));

    // A move onto the first row's grip keeps that row hovered.
    test.tx.send(move_at(10.0, 6.0)).unwrap();
    assert!(test.core.step());
    assert_eq!(test.held().queue().hover, Some(1));

    // A move away from every row clears it.
    test.tx.send(move_at(500.0, 500.0)).unwrap();
    assert!(test.core.step());
    assert_eq!(test.held().queue().hover, None);
}

/// A drag's motion previews the drop index and the frame's ghost and
/// insertion line: the grip's press starts the drag at the row's own place,
/// and a move over the third row's lower half inserts the row after it, so
/// the slot's final index is the row it would become (the list with the
/// dragged row removed).
#[test]
fn a_drag_move_previews_the_drop_index_and_the_ghost() {
    let mut test = test_core(4);
    test.tx.send(rows_with_first_grip()).unwrap();
    assert!(test.core.step());
    test.held();

    test.tx.send(press_at(10.0, 6.0)).unwrap();
    assert!(test.core.step());
    let started = test.held();
    let drag = started
        .queue()
        .drag
        .clone()
        .expect("the grip's press starts the drag");
    assert_eq!(drag.slot, 1);
    assert_eq!(drag.drop_index, 0);
    assert_eq!(drag.ghost_primary(), "Item 0");

    // The third row runs from y=48 to y=72; its lower half is the "after"
    // side.
    test.tx.send(drag_to(10.0, 66.0)).unwrap();
    assert!(test.core.step());
    let frame = test.held();
    let drag = frame.queue().drag.clone().expect("the drag keeps running");
    assert_eq!(drag.slot, 1);
    assert_eq!(drag.drop_index, 2);
    assert_eq!(drag.insertion, GuiQueueInsertion::After { slot: 3 });
    assert_eq!(drag.ghost_primary(), "Item 0");
}

/// A release after a drag sends exactly one move request, addressed by the
/// dragged slot's stable identity and the drop index; the press and the
/// motion send none (add-tauri-gui-client row 6.5, design D5).
#[test]
fn a_drag_release_sends_one_move_request() {
    let (app, cmd_rx) = stub_app_with_queue_and_cmd_rx(4);
    let mut test = test_core_with(app);
    test.tx.send(rows_with_first_grip()).unwrap();
    assert!(test.core.step());
    test.held();

    test.tx.send(press_at(10.0, 6.0)).unwrap();
    assert!(test.core.step());
    test.held();
    assert_eq!(slot_moves(&cmd_rx), vec![], "a press sends no request");

    test.tx.send(drag_to(10.0, 66.0)).unwrap();
    assert!(test.core.step());
    test.held();
    assert_eq!(slot_moves(&cmd_rx), vec![], "motion sends no request");

    test.tx.send(release_at(10.0, 66.0)).unwrap();
    assert!(test.core.step());
    assert!(test.held().queue().drag.is_none(), "the drag ended");
    assert_eq!(slot_moves(&cmd_rx), vec![(1, 2)]);
}

/// Esc cancels a drag in progress: the release sends no move request and the
/// frame paints no drag (add-tauri-gui-client row 6.5's "cancel sends none").
#[test]
fn a_cancelled_drag_sends_no_move_request() {
    let (app, cmd_rx) = stub_app_with_queue_and_cmd_rx(4);
    let mut test = test_core_with(app);
    test.tx.send(rows_with_first_grip()).unwrap();
    assert!(test.core.step());
    test.held();

    test.tx.send(press_at(10.0, 6.0)).unwrap();
    assert!(test.core.step());
    test.held();
    test.tx.send(drag_to(10.0, 66.0)).unwrap();
    assert!(test.core.step());
    test.held();

    test.tx
        .send(CoreInput::Key(KeyEvent::new(Key::Esc, KeyModifiers::NONE)))
        .unwrap();
    assert!(test.core.step());
    assert!(test.held().queue().drag.is_none());

    test.tx.send(release_at(10.0, 66.0)).unwrap();
    assert!(test.core.step());
    test.held();
    assert_eq!(slot_moves(&cmd_rx), vec![]);
}

/// A row-menu entry's press executes the painted entry through the Client
/// core: the menu the actions button opened, Remove from queue, sends the
/// Owner the pressed row's slot removal (add-tauri-gui-client row 6.5's row
/// menu). The menu's entries and its anchor are the frame test's contract.
#[test]
fn a_row_menu_entry_press_removes_its_row() {
    let (app, cmd_rx) = stub_app_with_queue_and_cmd_rx(4);
    let mut test = test_core_with(app);
    test.tx
        .send(measured_layout(
            4,
            vec![region_at(
                80.0,
                24.0,
                RegionTarget::Queue(QueueTargetId::RowActions(2)),
            )],
        ))
        .unwrap();
    assert!(test.core.step());
    test.held();

    test.tx.send(press_at(90.0, 30.0)).unwrap();
    assert!(test.core.step());
    assert!(test.held().overlay().is_some());

    // The menu's paint reports its rows; the remove entry is the fifth.
    test.tx
        .send(measured_layout(
            4,
            vec![region_at(
                0.0,
                200.0,
                RegionTarget::Queue(QueueTargetId::RowMenuEntry(4)),
            )],
        ))
        .unwrap();
    assert!(test.core.step());
    test.held();

    test.tx.send(press_at(10.0, 210.0)).unwrap();
    assert!(test.core.step());
    let frame = test.held();
    assert!(
        frame.overlay().is_none(),
        "the executed entry closed the menu"
    );
    let removed: Vec<u64> = cmd_rx
        .try_iter()
        .filter_map(|cmd| match cmd {
            mbv_ctrl::CtrlCmd::UnifiedQueueRemoveSlot { slot_id, .. } => Some(slot_id),
            _ => None,
        })
        .collect();
    assert_eq!(removed, vec![2]);
}

/// The dragged row's primary line, for the ghost assertions.
trait GhostPrimary {
    fn ghost_primary(&self) -> String;
}

impl GhostPrimary for mbv_gui::bridge::frame::queue::GuiQueueDrag {
    fn ghost_primary(&self) -> String {
        match &self.ghost {
            GuiQueueRow::Item { primary, .. } => primary.clone(),
            GuiQueueRow::Heading { .. } | GuiQueueRow::Spacer => {
                panic!("a drag holds an item row")
            }
        }
    }
}

/// A press on the header's Length / Ends-at switch runs the `t` chord's
/// toggle (add-tauri-gui-client row 6.5): the next frame's right column is
/// the end clock.
#[test]
fn a_right_column_press_switches_the_rows_right_column() {
    let mut test = test_core(3);
    test.tx
        .send(measured_layout(
            3,
            vec![region_at(
                0.0,
                0.0,
                RegionTarget::Queue(QueueTargetId::RightColumn),
            )],
        ))
        .unwrap();
    assert!(test.core.step());
    test.held();

    test.tx.send(press_at(10.0, 10.0)).unwrap();
    assert!(test.core.step());
    assert_eq!(
        test.held().summary().right_column,
        mbv_gui::bridge::frame::queue::GuiRightColumn::EndsAt
    );
}

/// A press on the header's filter control runs the `/` chord's text entry
/// (add-tauri-gui-client row 6.5): the next frame reports the caret fact.
#[test]
fn a_filter_press_focuses_the_filter_text_entry() {
    let mut test = test_core(3);
    test.tx
        .send(measured_layout(
            3,
            vec![region_at(
                0.0,
                0.0,
                RegionTarget::Queue(QueueTargetId::Filter),
            )],
        ))
        .unwrap();
    assert!(test.core.step());
    test.held();

    test.tx.send(press_at(10.0, 10.0)).unwrap();
    assert!(test.core.step());
    assert!(test.held().summary().filter_active);
}

/// The row menu's frame names the row it opened on (add-tauri-gui-client row
/// 6.5): the context-menu chord opens it over the cursor row, and the
/// hovered row's actions button opens it over that row, so the window never
/// guesses the anchor.
#[test]
fn the_row_menu_frame_carries_the_row_it_opened_on() {
    let mut test = test_core(4);
    test.tx
        .send(measured_layout(
            4,
            vec![region_at(
                80.0,
                48.0,
                RegionTarget::Queue(QueueTargetId::RowActions(3)),
            )],
        ))
        .unwrap();
    assert!(test.core.step());
    test.held();

    // The context-menu chord opens the menu over the cursor row, slot 1.
    test.tx
        .send(CoreInput::Key(KeyEvent::new(
            Key::Char('.'),
            KeyModifiers::NONE,
        )))
        .unwrap();
    assert!(test.core.step());
    let frame = test.held();
    let Some(GuiOverlayFrame::RowMenu { anchor_slot, .. }) = frame.overlay() else {
        panic!("the chord opened the row menu: {:?}", frame.overlay());
    };
    assert_eq!(*anchor_slot, 1);

    // Esc dismisses it; the actions button then opens it over its own row.
    test.tx
        .send(CoreInput::Key(KeyEvent::new(Key::Esc, KeyModifiers::NONE)))
        .unwrap();
    assert!(test.core.step());
    assert!(test.held().overlay().is_none());

    test.tx.send(press_at(90.0, 60.0)).unwrap();
    assert!(test.core.step());
    let frame = test.held();
    let Some(GuiOverlayFrame::RowMenu { anchor_slot, .. }) = frame.overlay() else {
        panic!(
            "the actions button opened the row menu: {:?}",
            frame.overlay()
        );
    };
    assert_eq!(*anchor_slot, 3);
}

/// The GUI-only ``Alt+J``/``Alt+K`` row move addresses the row the window
/// paints selected, not the App's follow cursor (review of row 6.5, finding
/// P1-1): a keyboard move of the surface selection, then the chord, names
/// the moved selection's stable slot.
#[test]
fn a_row_move_chord_moves_the_keyboard_selected_row_review_u65_p1_1() {
    let (app, cmd_rx) = stub_app_with_queue_and_cmd_rx(3);
    let mut test = test_core_with(app);
    test.tx.send(measured_layout(3, vec![])).unwrap();
    assert!(test.core.step());
    test.held();

    test.tx
        .send(CoreInput::Key(KeyEvent::new(Key::Down, KeyModifiers::NONE)))
        .unwrap();
    assert!(test.core.step());
    assert_eq!(
        test.held().queue().cursor,
        Some(2),
        "the key moved the painted cursor to the second slot"
    );

    test.tx
        .send(CoreInput::Key(KeyEvent::new(
            Key::Char('j'),
            KeyModifiers::ALT,
        )))
        .unwrap();
    assert!(test.core.step());
    test.held();

    assert_eq!(
        slot_moves(&cmd_rx),
        vec![(2, 2)],
        "the move names the painted selection's slot"
    );
}

/// A lost release leaves no live drag (review of add-tauri-gui-client row
/// 6.5, finding P1-2): the cancel clears the armed drag, and the click that
/// follows sends no move for the stale slot.
#[test]
fn a_cancelled_drag_then_click_sends_no_move_review_u65_p1_2() {
    let (app, cmd_rx) = stub_app_with_queue_and_cmd_rx(4);
    let mut test = test_core_with(app);
    test.tx.send(rows_with_first_grip()).unwrap();
    assert!(test.core.step());
    test.held();

    test.tx.send(press_at(10.0, 6.0)).unwrap();
    assert!(test.core.step());
    test.held();
    test.tx.send(drag_to(10.0, 66.0)).unwrap();
    assert!(test.core.step());
    assert_eq!(
        test.held()
            .queue()
            .drag
            .clone()
            .expect("the drag holds")
            .drop_index,
        2
    );

    test.tx.send(cancel_pointer()).unwrap();
    assert!(test.core.step());
    assert!(
        test.held().queue().drag.is_none(),
        "the cancel clears the drag"
    );

    // The next click is a fresh gesture and moves nothing.
    test.tx.send(press_at(10.0, 30.0)).unwrap();
    assert!(test.core.step());
    test.held();
    test.tx.send(release_at(10.0, 30.0)).unwrap();
    assert!(test.core.step());
    test.held();
    assert_eq!(slot_moves(&cmd_rx), vec![]);
}

/// A cancel sends no move (review of add-tauri-gui-client row 6.5, finding
/// P1-2): the release that follows the cancel has no live press, so it
/// reaches no move request.
#[test]
fn a_pointer_cancel_sends_no_move_review_u65_p1_2() {
    let (app, cmd_rx) = stub_app_with_queue_and_cmd_rx(4);
    let mut test = test_core_with(app);
    test.tx.send(rows_with_first_grip()).unwrap();
    assert!(test.core.step());
    test.held();

    test.tx.send(press_at(10.0, 6.0)).unwrap();
    assert!(test.core.step());
    test.held();
    test.tx.send(drag_to(10.0, 66.0)).unwrap();
    assert!(test.core.step());
    test.held();

    test.tx.send(cancel_pointer()).unwrap();
    assert!(test.core.step());
    test.held();
    test.tx.send(release_at(10.0, 66.0)).unwrap();
    assert!(test.core.step());
    test.held();
    assert_eq!(slot_moves(&cmd_rx), vec![]);
}

/// A new press discards a stale armed drag (review of add-tauri-gui-client
/// row 6.5, finding P1-2): after a lost release, the next click's `DragEnd`
/// sends no move for the slot the old drag grabbed.
#[test]
fn a_new_press_discards_the_stale_drag_review_u65_p1_2() {
    let (app, cmd_rx) = stub_app_with_queue_and_cmd_rx(4);
    let mut test = test_core_with(app);
    test.tx.send(rows_with_first_grip()).unwrap();
    assert!(test.core.step());
    test.held();

    test.tx.send(press_at(10.0, 6.0)).unwrap();
    assert!(test.core.step());
    test.held();
    test.tx.send(drag_to(10.0, 66.0)).unwrap();
    assert!(test.core.step());
    test.held();

    // The lost release never arrives: the next press starts a new gesture
    // and the stale drag must be gone before its release resolves.
    test.tx.send(press_at(10.0, 30.0)).unwrap();
    assert!(test.core.step());
    assert!(
        test.held().queue().drag.is_none(),
        "the new press discarded the stale drag"
    );
    test.tx.send(release_at(10.0, 30.0)).unwrap();
    assert!(test.core.step());
    test.held();
    assert_eq!(slot_moves(&cmd_rx), vec![]);
}
