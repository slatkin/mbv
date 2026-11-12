//! The section batch's contracts (`gui-change-driven-sections` design D1, D2):
//! a tick sends the sections it changed, each under its revision, and nothing
//! else.

use mbv_gui::bridge::sections::GuiSectionBatch;
use mbv_gui::core_thread::CoreInput;
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers};

use crate::test_core;

/// A queue cursor move sends `queue` with a new revision and no other section
/// (design D2): the cursor is the only fact the key changed, and the compare
/// finds no other section moved.
#[test]
fn a_queue_cursor_move_sends_only_the_queue_section() {
    let mut test = test_core(3);
    assert!(test.core.step());
    let first_revision = test
        .next_batch()
        .queue
        .map(|section| section.revision)
        .expect("the first batch carries the queue");

    test.tx
        .send(CoreInput::Key(KeyEvent::new(Key::Down, KeyModifiers::NONE)))
        .unwrap();
    assert!(test.core.step());
    let batch = test.next_batch();

    let queue = batch
        .queue
        .clone()
        .expect("the cursor move sends the queue");
    assert!(queue.revision > first_revision);
    assert_eq!(
        batch,
        GuiSectionBatch {
            queue: Some(queue),
            ..GuiSectionBatch::default()
        }
    );
}
