//! The Client core's shown entry (gui-named-queues design D7): the local
//! Owner's Named queues, the one resting queue a GUI Client shows, and the link
//! the queue panel and the edits resolve to. Specs: `gui-queue-chooser`
//! ("The player panel follows the shown entry's owner"), `named-queues`
//! ("Queue edits name their queue").

use super::*;
use crate::app::state::playback::ShownEntry;
use crate::app::tests::route_state::stub_endpoint;
use mbv_ctrl::{NamedQueueEntry, UnifiedQueueStateData};
use mbv_queue::NamedQueueId;
use mbv_surfaces::queue::QueueCursorUpdate;
use rstest::rstest;

/// The stub Owner's current queue.
pub(crate) fn current() -> NamedQueueId {
    NamedQueueId::from_raw(1)
}

/// The stub Owner's resting queue.
pub(crate) fn resting() -> NamedQueueId {
    NamedQueueId::from_raw(2)
}

/// The queue list the stub Owner publishes: `current()` holds three slots, the
/// resting queue holds one.
pub(crate) fn named_queue_list() -> Vec<NamedQueueEntry> {
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
pub(crate) fn resting_snapshot(items: &[EmbyItem]) -> UnifiedQueueStateData {
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
pub(crate) fn gui_app_with_peer() -> (
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

/// Adopt `state` as the current queue's view: the snapshot the Owner would
/// have published for the queue that holds the playhead.
fn adopt_current_queue(app: &mut App, state: &UnifiedQueueStateData) {
    app.local_view.adopt(
        state,
        crate::app::state::queue_view::AdoptCause::Replacement,
    );
}

/// The current queue's snapshot: `items` as its slots, with its start slot at
/// `start_index` (design D4).
fn queue_with_start(items: &[EmbyItem], start_index: usize) -> UnifiedQueueStateData {
    let mut state = emby_unified_state(items, 0);
    state.start_slot = state.slots.get(start_index).map(|slot| slot.slot_id);
    state
}

/// A GUI Client attached to a local Owner that advertises answered queue ops,
/// with the Named-queue list adopted and its constructor traffic drained.
/// Every named-queue edit is an answered op, so a test answers the Client's
/// own op through [`inject_local_events`].
pub(crate) fn gui_app_with_named_queues() -> (App, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    let (remote, player_rx, cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_answered_queue_ops_with_command_rx(make_items(3), 0);
    let config = crate::config::Config {
        stay_alive: true,
        ..remote_stub_config()
    };
    let mut app = App::new_remote_with_config(
        mbv_emby::EmbyClient::new(config.clone()),
        remote,
        player_rx,
        &mbv_remote_player::DaemonEndpoint::Local,
        config,
    );
    close_initial_services(&mut app);
    while cmd_rx.try_recv().is_ok() {}
    app.enable_gui_client();
    app.adopt_named_queue_list(named_queue_list(), current());
    (app, cmd_rx)
}

/// A TUI Client attached to a local Owner that advertises answered queue ops.
fn answered_local_daemon_app() -> (App, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    let (remote, player_rx, cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_answered_queue_ops_with_command_rx(make_items(3), 0);
    let config = crate::config::Config {
        stay_alive: true,
        ..remote_stub_config()
    };
    let mut app = App::new_remote_with_config(
        mbv_emby::EmbyClient::new(config.clone()),
        remote,
        player_rx,
        &mbv_remote_player::DaemonEndpoint::Local,
        config,
    );
    close_initial_services(&mut app);
    while cmd_rx.try_recv().is_ok() {}
    (app, cmd_rx)
}

/// Replace the Local link's event receiver with an injected one, so a test can
/// answer the Client's own queue op; keep the sender alive to deliver it.
pub(crate) fn inject_local_events(app: &mut App) -> std::sync::mpsc::Sender<PlayerEvent> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.player_rx = rx;
    tx
}

/// The answer to the Client's own queue op `op`, carrying `data`.
pub(crate) fn op_answer(op: u64, data: UnifiedQueueStateData) -> PlayerEvent {
    PlayerEvent::QueueOpResult {
        op: mbv_ctrl::QueueOpId(op),
        outcome: mbv_ctrl::QueueOpOutcome::Applied(Box::new(data)),
    }
}

/// The `queue` field one sent command carries, for the commands that address a
/// Named queue.
fn sent_queue(cmd: &mbv_ctrl::CtrlCmd) -> Option<NamedQueueId> {
    match cmd {
        mbv_ctrl::CtrlCmd::UnifiedQueueRemoveSlot { queue, .. }
        | mbv_ctrl::CtrlCmd::UnifiedQueueAppend { queue, .. } => *queue,
        other => panic!("not a quantity-carrying queue command: {other:?}"),
    }
}

/// An enqueue from the Library panel reaches the shown resting queue, and the
/// playing current queue keeps its slots (spec `gui-queue-chooser` "Enqueue to
/// a resting queue").
#[test]
fn an_enqueue_to_a_resting_queue_names_it() {
    let (mut app, cmd_rx) = gui_app_with_named_queues();
    let tx = inject_local_events(&mut app);
    app.show_named_queue(resting());
    while cmd_rx.try_recv().is_ok() {}
    let kids_after_append = resting_snapshot(&make_items(2));
    tx.send(op_answer(1, kids_after_append)).unwrap();

    let appended = app.append_on_owner(
        QueueLink::Local,
        make_items(1)
            .into_iter()
            .map(|item| mbv_queue::QueueItem::Emby(Box::new(item)))
            .collect(),
        None,
    );

    assert!(appended);
    assert_eq!(
        sent_queue(&cmd_rx.try_recv().unwrap()),
        Some(resting()),
        "the append names the shown resting queue"
    );
    assert_eq!(
        app.displayed_queue().slots().len(),
        2,
        "the answer fills the shown resting queue"
    );
    assert_eq!(
        app.local_view.slots().len(),
        3,
        "the playing current queue does not change"
    );
}

/// Play while the local Owner plays nothing and a resting queue is shown asks
/// the Owner to play that queue, so it starts at its own start slot (spec
/// `gui-queue-chooser` "Play a stopped queue").
#[test]
fn play_with_nothing_playing_starts_the_shown_queue() {
    let (mut app, cmd_rx) = gui_app_with_named_queues();
    let tx = inject_local_events(&mut app);
    app.show_named_queue(resting());
    while cmd_rx.try_recv().is_ok() {}
    app.player.update_status(|status| status.active = false);
    tx.send(op_answer(1, queue_with_start(&make_items(3), 2)))
        .unwrap();

    LocalPlaybackTarget::toggle_play_pause(&mut app);

    assert!(
        matches!(
            cmd_rx.try_recv().unwrap(),
            mbv_ctrl::CtrlCmd::NamedQueuePlay {
                queue,
                slot: None,
                op: mbv_ctrl::QueueOpId(1),
            } if queue == resting()
        ),
        "Play names the shown queue and lets it pick its own start slot"
    );
}

/// Undo sends its inverse edit to the queue the undone edit reached, whichever
/// entry is shown when the user undoes (spec `gui-queue-chooser` "Undo reaches
/// the queue it edited").
#[test]
fn undo_reaches_the_queue_it_edited() {
    let (mut app, cmd_rx) = gui_app_with_named_queues();
    let tx = inject_local_events(&mut app);
    app.show_named_queue(resting());
    while cmd_rx.try_recv().is_ok() {}
    app.adopt_named_queue_snapshot(resting(), &resting_snapshot(&make_items(2)));
    tx.send(op_answer(1, resting_snapshot(&make_items(1))))
        .unwrap();
    app.remove_from_queue(1);
    assert_eq!(
        sent_queue(&cmd_rx.try_recv().unwrap()),
        Some(resting()),
        "the removal names the shown resting queue"
    );

    app.show_local_current_entry();
    tx.send(op_answer(2, resting_snapshot(&make_items(2))))
        .unwrap();
    app.undo_last_queue_edit(QueueLink::Local);

    match cmd_rx.try_recv().unwrap() {
        mbv_ctrl::CtrlCmd::UnifiedQueueAppend {
            queue,
            items,
            before,
            ..
        } => {
            assert_eq!(queue, Some(resting()), "the undo names the edited queue");
            assert_eq!(items.len(), 1);
            assert_eq!(
                before, None,
                "a resting queue the panel no longer shows holds no client view, so the item returns at its end"
            );
        }
        other => panic!("expected the inverse append: {other:?}"),
    }
    assert_eq!(
        app.local_view.slots().len(),
        3,
        "the queue the panel shows now does not change"
    );
}

/// The TUI Client shows the current queue by the connection rule, so its edits
/// name no Named queue (spec `named-queues` "Queue edits name their queue": an
/// edit that names no queue applies to the current queue).
#[test]
fn a_tui_edit_names_no_queue() {
    let (mut app, cmd_rx) = answered_local_daemon_app();
    let tx = inject_local_events(&mut app);
    tx.send(op_answer(1, emby_unified_state(&make_items(2), 0)))
        .unwrap();

    app.remove_from_queue(1);

    assert_eq!(
        sent_queue(&cmd_rx.try_recv().unwrap()),
        None,
        "a TUI Client's edit carries no Named queue"
    );
}

/// A lost connection removes the peer entry and shows the current local queue,
/// with the entry-change reset: the filter clears and the active row returns to
/// the shown queue's start slot (spec `gui-queue-chooser` "A connect adds a peer
/// entry" — "The connection is lost").
#[test]
fn the_connection_is_lost_shows_the_current_queue() {
    let (mut app, home_cmd_rx, _peer_cmd_rx) = gui_app_with_peer();
    while home_cmd_rx.try_recv().is_ok() {}
    adopt_current_queue(&mut app, &queue_with_start(&make_items(3), 1));
    app.queue_filter = "kid".to_string();
    app.queue_filter_active = true;

    app.restore_local_mode("lost");

    assert_eq!(
        app.gui_shown_entry,
        Some(ShownEntry::Local(current())),
        "the queue panel returns to the current local queue"
    );
    assert!(
        app.queue_filter.is_empty() && !app.queue_filter_active,
        "the Queue filter closes and empties"
    );
    let projection = app.queue_surface_projection();
    assert!(
        projection.presentation_reset,
        "the Queue Surface drops its marks"
    );
    assert!(
        matches!(projection.cursor, QueueCursorUpdate::Set(1)),
        "the active row rests on the shown queue's start slot"
    );
}

/// Showing another entry clears the filter and the marks and puts the active
/// row on the shown queue's start slot (spec `gui-queue-chooser` "Showing an
/// entry resets the Queue active row and marks").
#[test]
fn showing_another_entry_resets_the_queue_presentation() {
    let (mut app, home_cmd_rx) = gui_app_with_named_queues();
    adopt_current_queue(&mut app, &queue_with_start(&make_items(3), 1));
    while home_cmd_rx.try_recv().is_ok() {}

    app.show_named_queue(resting());
    app.adopt_named_queue_snapshot(resting(), &queue_with_start(&make_items(3), 2));
    assert!(
        matches!(
            app.queue_surface_projection().cursor,
            QueueCursorUpdate::Set(2)
        ),
        "the resting queue's own start slot is its first active row"
    );
    app.queue_filter = "kid".to_string();
    app.queue_filter_active = true;

    app.show_local_current_entry();

    assert!(
        app.queue_filter.is_empty() && !app.queue_filter_active,
        "the Queue filter closes and empties"
    );
    let projection = app.queue_surface_projection();
    assert!(
        projection.presentation_reset,
        "the Queue Surface drops its marks"
    );
    assert!(matches!(projection.cursor, QueueCursorUpdate::Set(1)));
}

/// A close of the shown queue that another Client asked for removes its entry
/// and shows the current local queue, with the same reset (spec
/// `gui-queue-chooser` "The close control closes the shown entry": after a
/// close the Queue panel shows the current local queue).
#[test]
fn a_closed_shown_queue_returns_to_the_current_queue() {
    let (mut app, _cmd_rx) = gui_app_with_named_queues();
    app.adopt_named_queue_snapshot(resting(), &resting_snapshot(&make_items(2)));
    app.show_named_queue(resting());
    app.queue_filter = "kid".to_string();
    app.queue_filter_active = true;

    app.adopt_named_queue_list(
        vec![NamedQueueEntry {
            id: current(),
            name: "Default".to_string(),
            len: 3,
        }],
        current(),
    );

    assert_eq!(
        app.gui_shown_entry,
        Some(ShownEntry::Local(current())),
        "the queue the panel shows is the one that holds the playhead"
    );
    assert!(
        app.queue_filter.is_empty() && !app.queue_filter_active,
        "the Queue filter closes and empties"
    );
    assert!(
        app.queue_surface_projection().presentation_reset,
        "the Queue Surface drops its marks"
    );
}

/// One resting queue's snapshot: its start slot at `start_index` — `None` for a
/// queue without one — and no playhead, because a resting queue holds no run
/// (design D4, D7).
fn resting_state(items: &[EmbyItem], start_index: Option<usize>) -> UnifiedQueueStateData {
    let mut state = emby_unified_state(items, 0);
    state.active_slot = None;
    state.start_slot =
        start_index.and_then(|index| state.slots.get(index).map(|slot| slot.slot_id));
    state
}

/// A resting queue is shown while the current queue plays its first slot. The
/// shown queue carries no playhead of its own, so its rows take the start mark
/// from its own snapshot and never the playing queue's index (design D7, D9;
/// spec `gui-client` "A stopped queue marks its start slot").
#[test]
fn a_resting_queue_never_marks_the_playing_queue_s_index() {
    let (mut app, _cmd_rx) = gui_app_with_named_queues();
    app.show_named_queue(resting());
    app.adopt_named_queue_snapshot(resting(), &resting_state(&make_items(3), Some(2)));

    let playback = app.displayed_queue_playback_state();
    assert!(
        !playback.active,
        "the shown resting queue carries no playhead"
    );
    assert_eq!(playback.active_idx, None);
    let start = app.gui_queue_start_projection();
    assert!(!start.playing, "a resting queue does not play");
    assert_eq!(
        start.start,
        app.displayed_queue()
            .slots()
            .get(2)
            .map(|slot| slot.slot_id.raw()),
        "the start mark is the resting queue's own start slot"
    );
    app.focus_queue_initial_item();
    assert_eq!(
        app.displayed_queue().cursor(),
        2,
        "the playing queue's active index never moves the resting queue's row"
    );
}

/// A peer that reports no start slot marks its playing row: `start` falls back
/// to the observed active slot, so the row keeps the playing mark and no row
/// carries an outlined start mark (design D9, spec `gui-client` "A peer queue
/// marks its playing row").
#[test]
fn a_peer_queue_marks_its_playing_row() {
    let (app, _home_cmd_rx, _peer_cmd_rx) = gui_app_with_peer();
    let queue = app.displayed_queue();
    assert!(
        queue.start_slot_id().is_none(),
        "the peer reports no start slot"
    );

    let start = app.gui_queue_start_projection();

    assert_eq!(
        start.start,
        queue.active_slot_id().map(mbv_queue::QueueSlotId::raw),
        "the observed active slot stands in for the missing start slot"
    );
    assert!(start.playing, "the peer's queue plays");
}

/// A Play-next move inside a shown resting queue reaches that queue: the row
/// lands directly after the queue's start slot, which stands where a playhead
/// stands without one, and first when the queue has no start slot (spec
/// `gui-queue-chooser` "Edits and enqueue reach the shown entry", design D7,
/// D9).
#[rstest]
#[case::after_its_start_slot(Some(0), 2, 1)]
#[case::at_the_front_without_a_start_slot(None, 2, 0)]
fn play_next_on_a_resting_queue_reaches_it(
    #[case] start_index: Option<usize>,
    #[case] moved: usize,
    #[case] expected_to: usize,
) {
    let (mut app, cmd_rx) = gui_app_with_named_queues();
    app.show_named_queue(resting());
    app.adopt_named_queue_snapshot(resting(), &resting_state(&make_items(3), start_index));
    drain_commands(&cmd_rx);

    app.play_queue_item_next(moved);

    let sent = cmd_rx
        .try_recv()
        .expect("the shown queue receives the move");
    assert_eq!(
        sent_move_target(&sent),
        Some((expected_to, Some(resting()))),
        "the move lands after the resting queue's own anchor, naming it"
    );
}

/// Drain every command the Client sent so far.
fn drain_commands(cmd_rx: &std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    while cmd_rx.try_recv().is_ok() {}
}

/// The target index and Named queue one sent queue move carries.
fn sent_move_target(cmd: &mbv_ctrl::CtrlCmd) -> Option<(usize, Option<NamedQueueId>)> {
    match cmd {
        mbv_ctrl::CtrlCmd::UnifiedQueueMoveSlot {
            to_index, queue, ..
        } => Some((*to_index, *queue)),
        _ => None,
    }
}
