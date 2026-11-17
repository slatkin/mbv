//! The Client core's shown entry (gui-named-queues design D7): the local
//! Owner's Named queues, the one resting queue a GUI Client shows, and the link
//! the queue panel and the edits resolve to. Specs: `gui-queue-chooser`
//! ("The player panel follows the shown entry's owner"), `named-queues`
//! ("Queue edits name their queue").

use super::*;
use crate::app::tests::route_state::stub_endpoint;
use mbv_ctrl::{NamedQueueEntry, UnifiedQueueStateData};
use mbv_queue::NamedQueueId;

/// The stub Owner's current queue.
fn current() -> NamedQueueId {
    NamedQueueId::from_raw(1)
}

/// The stub Owner's resting queue.
fn resting() -> NamedQueueId {
    NamedQueueId::from_raw(2)
}

/// The queue list the stub Owner publishes: `current()` holds three slots, the
/// resting queue holds one.
fn named_queue_list() -> Vec<NamedQueueEntry> {
    vec![
        NamedQueueEntry {
            id: current(),
            name: "Default".to_string(),
            len: 3,
        },
        NamedQueueEntry {
            id: resting(),
            name: "Kids".to_string(),
            len: 1,
        },
    ]
}

/// One resting queue's snapshot: `items` as its slots, naming `resting()`.
fn resting_snapshot(items: &[EmbyItem]) -> UnifiedQueueStateData {
    UnifiedQueueStateData {
        queue: Some(resting()),
        start_slot: None,
        slots: items
            .iter()
            .enumerate()
            .map(|(index, item)| mbv_ctrl::UnifiedQueueSlot {
                slot_id: index as u64 + 1,
                item: mbv_queue::QueueItem::Emby(Box::new(item.clone())),
            })
            .collect(),
        ..emby_unified_state(items, 0)
    }
}

/// A local-daemon App holding `local_items` on its current queue, with the
/// Named-queue list adopted.
fn app_with_named_queues(local_items: Vec<EmbyItem>) -> App {
    let mut app = make_local_daemon_app_stub(local_items);
    app.adopt_named_queue_list(named_queue_list(), current());
    app
}

/// The GUI Client shows a queue that rests: its snapshot fills the shown view,
/// and the current queue's view is untouched (design D7).
#[test]
fn a_resting_snapshot_fills_the_shown_view_and_not_the_current_queue() {
    let (mut app, cmd_rx) = make_local_daemon_app_stub_with_cmd_rx(make_items(3));
    while cmd_rx.try_recv().is_ok() {}
    app.enable_gui_client();
    app.adopt_named_queue_list(named_queue_list(), current());

    app.show_named_queue(resting());
    assert!(
        matches!(cmd_rx.try_recv(), Ok(mbv_ctrl::CtrlCmd::NamedQueueShow { queue }) if queue == resting()),
        "showing a resting queue asks the local Owner for its snapshots"
    );
    assert_eq!(
        app.displayed_queue().slots().len(),
        0,
        "the shown resting queue starts empty, never on the current queue's rows"
    );

    app.handle_player_event(PlayerEvent::NamedQueue(
        mbv_ctrl::NamedQueueEvent::Snapshot {
            queue: resting(),
            data: Box::new(resting_snapshot(&make_items(1))),
        },
    ));

    assert_eq!(app.displayed_queue().slots().len(), 1);
    assert_eq!(app.local_view.slots().len(), 3);
}

/// A GUI Client connected to a peer shows the local Named queue it picked
/// (ADR 0041: the shown entry, not the connection, picks the shown queue).
#[test]
fn a_gui_client_connected_to_a_peer_shows_a_local_queue() {
    let mut app = app_with_named_queues(make_items(3));
    let (remote, remote_rx) = mbv_remote_player::RemotePlayer::stub(make_items(2), 0);
    let sess = make_session("peer-host", "mbv");
    app.switch_to_direct_remote(&sess, remote, remote_rx, &stub_endpoint());
    assert!(app.suspended_local.is_some(), "the local link stays held");
    app.enable_gui_client();

    app.show_named_queue(current());

    assert_eq!(app.playing_queue_link(), QueueLink::Local);
    assert_eq!(app.displayed_queue().slots().len(), 3);
}

/// The TUI Client keeps the connection rule (ADR 0034): while a peer is
/// connected it shows the peer's queue, whatever the GUI-only shown entry
/// holds.
#[test]
fn the_tui_link_keeps_the_connection_rule() {
    let mut app = app_with_named_queues(make_items(3));
    let (remote, remote_rx) = mbv_remote_player::RemotePlayer::stub(make_items(2), 0);
    let sess = make_session("peer-host", "mbv");
    app.switch_to_direct_remote(&sess, remote, remote_rx, &stub_endpoint());
    app.gui_shown_entry = Some(crate::app::state::playback::ShownEntry::Local(current()));

    assert_eq!(app.playing_queue_link(), QueueLink::Peer);
    assert_eq!(app.displayed_queue().slots().len(), 2);
}
