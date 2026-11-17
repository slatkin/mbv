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

/// The local Owner's own position, distinct from the peer's.
const HOME_POSITION: i64 = 7;

/// The connected peer's own position, distinct from the local Owner's.
const PEER_POSITION: i64 = 42;

/// A GUI Client attached to the local Owner whose link plays at
/// [`HOME_POSITION`], then connected to a peer that plays at
/// [`PEER_POSITION`]: each link's commands land on its own channel. The
/// connect shows the peer entry (design D7), as the playback-target chooser
/// does.
fn gui_app_with_peer() -> (
    App,
    std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>,
    std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>,
) {
    let (mut app, home_cmd_rx) = make_local_daemon_app_stub_with_cmd_rx(make_items(3));
    while home_cmd_rx.try_recv().is_ok() {}
    app.player
        .update_status(|status| status.position_ticks = HOME_POSITION);
    app.enable_gui_client();
    app.adopt_named_queue_list(named_queue_list(), current());

    let (peer, peer_rx, peer_cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_with_command_rx(make_items(2), 1);
    let sess = make_session("peer-host", "mbv");
    app.switch_to_direct_remote(&sess, peer, peer_rx, &stub_endpoint());
    app.player
        .update_status(|status| status.position_ticks = PEER_POSITION);
    while peer_cmd_rx.try_recv().is_ok() {}
    (app, home_cmd_rx, peer_cmd_rx)
}

/// The pause intent one transport command sent to `link`'s owner.
fn pause_intent(paused: bool) -> impl Fn(&mbv_ctrl::CtrlCmd) -> bool {
    move |cmd| {
        matches!(
            cmd,
            mbv_ctrl::CtrlCmd::PlaybackIntent(mbv_ctrl::PlaybackIntent {
                action: mbv_ctrl::PlaybackIntentAction::SetPaused { paused: sent },
                ..
            }) if *sent == paused
        )
    }
}

/// A resting local queue shown while the local Owner plays: the panel and the
/// transport follow the local Owner, so pause pauses its playback and the
/// connected peer's playback is untouched (design D7, spec
/// `gui-queue-chooser` "A resting queue is shown while another plays").
#[test]
fn a_shown_resting_queue_pauses_the_local_owner_not_the_peer() {
    let (mut app, home_cmd_rx, peer_cmd_rx) = gui_app_with_peer();
    app.show_named_queue(resting());
    assert!(
        matches!(
            home_cmd_rx.try_recv(),
            Ok(mbv_ctrl::CtrlCmd::NamedQueueShow { .. })
        ),
        "the show reaches the local Owner's link"
    );

    assert_eq!(app.effective_playback_state().position_ticks, HOME_POSITION);
    LocalPlaybackTarget::toggle_play_pause(&mut app);

    assert!(
        pause_intent(true)(&home_cmd_rx.try_recv().unwrap()),
        "pause reaches the local Owner's playback"
    );
    assert!(
        peer_cmd_rx.try_recv().is_err(),
        "the connect's peer keeps playing"
    );
}

/// The peer entry shown: the panel and the transport follow the peer, so the
/// panel reports the peer's position and pause reaches it (design D7, spec
/// `gui-queue-chooser` "A peer entry is shown").
#[test]
fn a_shown_peer_entry_follows_the_peer_playback() {
    let (mut app, home_cmd_rx, peer_cmd_rx) = gui_app_with_peer();
    assert_eq!(app.playing_queue_link(), QueueLink::Peer);

    assert_eq!(app.effective_playback_state().position_ticks, PEER_POSITION);
    LocalPlaybackTarget::toggle_play_pause(&mut app);

    assert!(
        pause_intent(true)(&peer_cmd_rx.try_recv().unwrap()),
        "pause reaches the shown peer's playback"
    );
    assert!(
        home_cmd_rx.try_recv().is_err(),
        "the local Owner is not the shown entry"
    );
}

/// The TUI Client keeps the connection rule for the panel and the transport
/// too: with a peer connected, its status is the panel's and its playback is
/// what the transport controls, and the connect records no shown entry.
#[test]
fn the_tui_panel_and_transport_keep_the_connection_rule() {
    let (mut app, home_cmd_rx) = make_local_daemon_app_stub_with_cmd_rx(make_items(3));
    while home_cmd_rx.try_recv().is_ok() {}
    let (peer, peer_rx, peer_cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_with_command_rx(make_items(2), 1);
    let sess = make_session("peer-host", "mbv");
    app.switch_to_direct_remote(&sess, peer, peer_rx, &stub_endpoint());
    app.player
        .update_status(|status| status.position_ticks = PEER_POSITION);
    while peer_cmd_rx.try_recv().is_ok() {}

    assert!(
        app.gui_shown_entry.is_none(),
        "a TUI Client records no entry"
    );
    assert_eq!(app.effective_playback_state().position_ticks, PEER_POSITION);
    LocalPlaybackTarget::toggle_play_pause(&mut app);

    assert!(pause_intent(true)(&peer_cmd_rx.try_recv().unwrap()));
    assert!(
        home_cmd_rx.try_recv().is_err(),
        "the local Owner is not the connection's playback target"
    );
}
