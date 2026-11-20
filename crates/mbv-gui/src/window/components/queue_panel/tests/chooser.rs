//! The Queue chooser's dropdown contracts (gui-named-queues task 8.3, restyled
//! by design D8 of `gui-queue-panel-redesign`, spec `gui-queue-chooser` "The
//! chooser lists local queues and connected peers"): the local queues in queue
//! order then the peer, each entry its own pointer region under the identity it
//! paints; the shown, current, and peer marks; every entry's slot count; and the
//! inline rename entry's draft text with its caret.

use blitz_dom::LocalName;

use super::harness::*;
use crate::bridge::frame::overlay::GuiQueueChooserRename;
use crate::bridge::layout::{QueuePlaybackTargetId, QueueTargetId, RegionTarget};

/// Contract (spec `gui-queue-chooser` "The chooser lists local queues and
/// connected peers"): the dropdown paints the local Owner's queues in queue
/// order and then the connected peer, and each entry reports its own region
/// last — the decision of `Default` and `Kids` lands on the queue the entry
/// painted, and the peer entry names the peer rather than a queue.
#[test]
fn the_chooser_lists_the_local_queues_then_the_peer_and_reports_each_entry() {
    let fixture = built_with_frame(
        queue_panel_app,
        queue(Vec::new()),
        chooser_summary(),
        Some(queue_chooser(None)),
    );

    assert_eq!(
        fixture.text_of("[data-chooser-entry=\"1\"] [data-chooser-label]"),
        "Default"
    );
    assert_eq!(
        fixture.text_of("[data-chooser-entry=\"2\"] [data-chooser-label]"),
        "Kids"
    );
    assert_eq!(
        fixture.text_of("[data-chooser-entry=\"peer\"] [data-chooser-label]"),
        "living-room"
    );
    assert_eq!(
        fixture.last_reported_targets(),
        vec![
            RegionTarget::Queue(QueueTargetId::QueueChooser),
            RegionTarget::Queue(QueueTargetId::NewQueue),
            RegionTarget::Queue(QueueTargetId::CloseQueue),
            RegionTarget::Queue(QueueTargetId::Filter),
            RegionTarget::QueuePlayback(QueuePlaybackTargetId::PlaybackTarget),
            RegionTarget::Queue(QueueTargetId::QueueChooserEntry(1)),
            RegionTarget::Queue(QueueTargetId::QueueChooserEntry(2)),
            RegionTarget::Queue(QueueTargetId::QueueChooserPeer),
        ]
    );
}

/// Contract (user request on PR 1019, spec `gui-queue-chooser`): while the
/// local Owner plays its current queue, that entry carries the playing mark in
/// place of the current dot, and no other entry carries it.
#[test]
fn the_chooser_marks_the_queue_that_plays_in_place_of_the_current_dot() {
    let fixture = built_with_frame(
        queue_panel_app,
        queue(Vec::new()),
        chooser_summary(),
        Some(queue_chooser_playing()),
    );

    assert!(
        fixture.presents("[data-chooser-entry=\"1\"] [data-chooser-playing]"),
        "the playing queue carries the playing mark"
    );
    assert!(
        !fixture.presents("[data-chooser-entry=\"1\"] [data-chooser-current]"),
        "the playing mark replaces the current dot"
    );
    assert!(
        !fixture.presents("[data-chooser-entry=\"2\"] [data-chooser-playing]"),
        "a resting queue is not marked as playing"
    );
}

/// Contract (spec `gui-queue-chooser` "The chooser lists local queues and
/// connected peers"): the list marks the shown entry, marks the current queue,
/// and marks the peer entry as a peer.
#[test]
fn the_chooser_marks_the_shown_and_current_queues_and_the_peer() {
    let fixture = built_with_frame(
        queue_panel_app,
        queue(Vec::new()),
        chooser_summary(),
        Some(queue_chooser(None)),
    );

    assert_eq!(
        fixture.attr_of(
            "[data-chooser-entry=\"2\"] [data-chooser-shown]",
            LocalName::from("data-chooser-shown"),
        ),
        Some("true".to_owned()),
        "the shown queue carries the check"
    );
    assert!(
        fixture.presents("[data-chooser-entry=\"1\"] [data-chooser-current]"),
        "the queue that holds the playhead carries the current mark"
    );
    assert!(
        !fixture.presents("[data-chooser-entry=\"2\"] [data-chooser-current]"),
        "the shown resting queue is not current"
    );
    assert!(
        fixture.presents("[data-chooser-entry=\"peer\"] [data-brand=\"mbvd\"]"),
        "the peer entry carries its own mark"
    );
}

/// Contract (spec `gui-queue-chooser` "Each entry shows its slot count",
/// design D8 of `gui-queue-panel-redesign`): every entry paints its queue's
/// slot count at its trailing end, after the marks — the current queue's 26,
/// the resting queue's 4, and the peer's 9.
#[test]
fn the_chooser_paints_each_entrys_slot_count_after_its_mark() {
    let fixture = built_with_frame(
        queue_panel_app,
        queue(Vec::new()),
        chooser_summary(),
        Some(queue_chooser(None)),
    );

    assert_eq!(
        fixture.text_of("[data-chooser-entry=\"1\"] [data-chooser-count]"),
        "26"
    );
    assert_eq!(
        fixture.text_of("[data-chooser-entry=\"2\"] [data-chooser-count]"),
        "4"
    );
    assert_eq!(
        fixture.text_of("[data-chooser-entry=\"peer\"] [data-chooser-count]"),
        "9"
    );
    assert!(
        fixture
            .presents("[data-chooser-entry=\"1\"] [data-chooser-current] + [data-chooser-count]"),
        "the count follows the current queue's mark"
    );
}

/// Contract (spec `gui-queue-chooser` "The new-queue control makes and shows a
/// queue"): the entry whose name is open for edit paints the core-held draft
/// text with the Queue filter's block caret, and the peer entry opens no draft.
#[test]
fn the_chooser_paints_the_rename_draft_with_a_caret() {
    let fixture = built_with_frame(
        queue_panel_app,
        queue(Vec::new()),
        chooser_summary(),
        Some(queue_chooser(Some(GuiQueueChooserRename {
            queue: 2,
            text: "Kid".to_owned(),
            selected: false,
        }))),
    );

    assert_eq!(
        fixture.text_of("[data-chooser-entry=\"2\"] [data-chooser-label]"),
        "Kid"
    );
    assert!(
        fixture.presents("[data-chooser-entry=\"2\"] [data-caret]"),
        "the draft carries the block caret"
    );
    assert!(!fixture.presents("[data-chooser-entry=\"peer\"] [data-caret]"));
}

/// Contract (user request 2026-11-17): a draft whose whole name is selected
/// paints the name highlighted and no caret, and a long draft is never cut to
/// an ellipsis as it is typed.
#[test]
fn a_selected_draft_paints_highlighted_and_is_never_cut() {
    let name = "A long queue name typed in";
    let fixture = built_with_frame(
        queue_panel_app,
        queue(Vec::new()),
        chooser_summary(),
        Some(queue_chooser(Some(GuiQueueChooserRename {
            queue: 2,
            text: name.to_owned(),
            selected: true,
        }))),
    );

    assert_eq!(
        fixture.text_of("[data-chooser-entry=\"2\"] [data-chooser-label]"),
        name
    );
    assert!(
        fixture
            .style_of("[data-chooser-entry=\"2\"] [data-chooser-label]")
            .contains("background"),
        "the selected name paints highlighted"
    );
    assert!(!fixture.presents("[data-chooser-entry=\"2\"] [data-caret]"));
}
