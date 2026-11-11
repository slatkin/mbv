//! add-gui-library-panel row 3.5 (design D8): play next admits a library item
//! directly after the active slot, and at the tail when the active slot is the
//! last one or the owner reports none. The Owner is the answered-op stub and
//! every answer is injected ahead of the call, so nothing waits on a clock.

use crate::app::tests::queue::queue_op::{answered_local_daemon_app, inject_player_rx};
use crate::app::tests::{QueueViewTestExt, emby_unified_state, make_items};
use mbv_ctrl::player::PlayerEvent;
use mbv_ctrl::{CtrlCmd, QueueOpId, QueueOpOutcome};
use mbv_queue::QueueItem;
use std::sync::mpsc::Receiver;

/// `count` queue entries for the adopted view: slot ids 1 to `count`.
fn queue_items(count: usize) -> Vec<QueueItem> {
    make_items(count)
        .into_iter()
        .map(|item| QueueItem::Emby(Box::new(item)))
        .collect()
}

/// The library item the test asks to play next, with an id no queue entry has.
fn next_item() -> mbv_emby_model::EmbyItem {
    let mut item = make_items(1)[0].clone();
    item.id = "next-item".to_string();
    item
}

/// Inject the answer the answered-op Owner sends for the first op: the queue
/// as the Owner keeps it after the append.
fn answer_the_append(tx: &std::sync::mpsc::Sender<PlayerEvent>) {
    tx.send(PlayerEvent::QueueOpResult {
        op: QueueOpId(1),
        outcome: QueueOpOutcome::Applied(Box::new(emby_unified_state(&make_items(4), 0))),
    })
    .unwrap();
}

/// The append the Owner received: its insertion anchor, whether it is an
/// answered op, and the ids of the items it carries.
fn received_append(cmd_rx: &Receiver<CtrlCmd>) -> (Option<u64>, bool, Vec<String>) {
    cmd_rx
        .try_iter()
        .find_map(|cmd| match cmd {
            CtrlCmd::UnifiedQueueAppend { items, before, op } => Some((
                before,
                op.is_some(),
                items.iter().map(|item| item.id().to_string()).collect(),
            )),
            _ => None,
        })
        .expect("the Owner received a queue append")
}

#[test]
fn play_next_admits_the_item_before_the_slot_after_the_active_one() {
    let (mut app, cmd_rx) = answered_local_daemon_app();
    let tx = inject_player_rx(&mut app);
    // The active slot is the first of three, so the anchor is slot 2.
    app.local_view
        .adopt_queue_items_with_active(queue_items(3), 0, 0);
    answer_the_append(&tx);

    app.play_lib_item_next(0, next_item());

    let (before, answered, ids) = received_append(&cmd_rx);
    assert_eq!(
        before,
        Some(2),
        "the item enters directly after the active slot"
    );
    assert!(answered, "play next is an answered owner operation");
    assert_eq!(ids, ["next-item"], "the item itself is admitted once");
}

#[test]
fn play_next_appends_at_the_tail_when_the_active_slot_is_the_last_one() {
    let (mut app, cmd_rx) = answered_local_daemon_app();
    let tx = inject_player_rx(&mut app);
    app.local_view
        .adopt_queue_items_with_active(queue_items(3), 0, 2);
    answer_the_append(&tx);

    app.play_lib_item_next(0, next_item());

    let (before, _, ids) = received_append(&cmd_rx);
    assert_eq!(before, None, "nothing follows the active slot: the tail");
    assert_eq!(ids, ["next-item"]);
}

#[test]
fn play_next_appends_at_the_tail_when_the_owner_reports_no_active_slot() {
    let (mut app, cmd_rx) = answered_local_daemon_app();
    let tx = inject_player_rx(&mut app);
    // An adopted snapshot whose active slot is unset.
    app.local_view.adopt_queue_items(queue_items(3), 0);
    answer_the_append(&tx);

    app.play_lib_item_next(0, next_item());

    let (before, _, ids) = received_append(&cmd_rx);
    assert_eq!(before, None, "with no active slot the item goes last");
    assert_eq!(ids, ["next-item"]);
}
