//! The section batch's contracts (`gui-change-driven-sections` design D1, D2):
//! a tick sends the sections it changed, each under its revision, and nothing
//! else.

use mbv_gui::bridge::sections::{GuiSection, GuiSectionBatch};
use mbv_gui::core_thread::CoreInput;
use mbv_gui::sections::SentRevisions;
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers};

use crate::test_core;

/// The attached core's first batch carries `attach: null` (design D4): the
/// window's pre-attach state clears.
#[test]
fn the_first_batch_clears_the_attach_state() {
    let mut test = test_core(2);
    assert!(test.core.step());
    assert_eq!(
        test.next_batch().attach,
        Some(GuiSection {
            revision: 1,
            value: None,
        })
    );
}

/// The attached core's attach and theme sections mint past the revisions the
/// campaign sent (design D4), so the window never skips the `null` for a
/// revision it already holds.
#[test]
fn the_attached_core_continues_the_campaign_revisions() {
    let mut test = test_core(2);
    test.core.resume_after_attach(SentRevisions {
        attach: 4,
        theme: 2,
    });
    assert!(test.core.step());
    let batch = test.next_batch();
    assert_eq!(batch.attach.map(|section| section.revision), Some(5));
    assert_eq!(batch.theme.map(|section| section.revision), Some(3));
}

/// After a batch, `sync` re-sends every present section at the revision it
/// held (design D4): the second batch equals the first one.
#[test]
fn sync_re_sends_every_present_section_at_its_revision() {
    let mut test = test_core(2);
    assert!(test.core.step());
    let first = test.next_batch();

    test.tx.send(CoreInput::Sync).unwrap();
    assert!(test.core.step());
    assert_eq!(test.next_batch(), first);
}

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
