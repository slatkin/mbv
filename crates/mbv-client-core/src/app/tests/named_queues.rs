//! The Client core's shown entry (gui-named-queues design D7): the local
//! Owner's Named queues, the one resting queue a GUI Client shows, and the link
//! the queue panel and the edits resolve to. Specs: `gui-queue-chooser`
//! ("The player panel follows the shown entry's owner"), `named-queues`
//! ("Queue edits name their queue").

use super::*;
use crate::app::state::playback::ShownEntry;
use mbv_ctrl::{NamedQueueEntry, UnifiedQueueStateData};
use mbv_queue::NamedQueueId;
use mbv_surfaces::queue::QueueCursorUpdate;
use rstest::rstest;

pub(super) mod peer_link;

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
    app.set_client_capabilities(gui_client_capabilities());
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

/// A TUI Client attached to a local Owner that advertises answered queue ops.
fn answered_local_daemon_app() -> (App, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    let (remote, player_rx, cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_answered_queue_ops_with_command_rx(make_items(3), 0);
    let config = crate::app::config::Config {
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

/// Replace the suspended home link's event receiver with an injected one, so a
/// test can deliver the local Owner's events while a peer holds the active
/// link. Keep the sender alive to deliver them.
pub(crate) fn inject_home_events(app: &mut App) -> std::sync::mpsc::Sender<PlayerEvent> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.suspended_local
        .as_mut()
        .expect("a suspended home link")
        .player_rx = rx;
    tx
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
        app.shown_entry,
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
/// shown queue carries no playhead of its own, so its cursor takes its own
/// snapshot's start slot and never the playing queue's index (design D7, D9).
#[test]
fn a_resting_queue_never_focuses_the_playing_queue_s_index() {
    let (mut app, _cmd_rx) = gui_app_with_named_queues();
    app.show_named_queue(resting());
    app.adopt_named_queue_snapshot(resting(), &resting_state(&make_items(3), Some(2)));

    let playback = app.displayed_queue_playback_state();
    assert!(
        !playback.active,
        "the shown resting queue carries no playhead"
    );
    assert_eq!(playback.active_idx, None);
    app.focus_queue_initial_item();
    assert_eq!(
        app.displayed_queue().cursor(),
        2,
        "the playing queue's active index never moves the resting queue's row"
    );
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

/// One replacement payload: `items` played from the front, as a Library Play
/// resolves it before the queue-replacement gates run.
fn replacement_of(items: &[EmbyItem]) -> PendingQueueAction {
    PendingQueueAction::PlayItems {
        items: items.to_vec(),
        start_idx: 0,
        source: mbv_queue::QueueSource::Unknown,
    }
}

/// The GUI Client shows a populated resting `Kids` while the current `Default`
/// queue is empty, so a replacement asks before it discards the shown rows
/// (spec `gui-queue-chooser` "The confirmation before a populated queue is
/// replaced SHALL apply to the shown queue").
#[test]
fn a_populated_resting_queue_asks_before_the_replacement() {
    let (mut app, _cmd_rx) = gui_app_with_named_queues();
    app.show_named_queue(resting());
    app.adopt_named_queue_snapshot(resting(), &resting_snapshot(&make_items(2)));
    adopt_current_queue(&mut app, &emby_unified_state(&[], 0));

    app.request_queue_replacement(replacement_of(&make_items(1)), ReplacementExecutor::Pending);

    assert!(
        matches!(
            &app.pending_overlay,
            Some(mbv_ui_model::overlay::OverlayRequest::Confirm(modal))
                if modal.on_confirm == crate::app::ConfirmAction::ReplacePopulatedQueue
        ),
        "the shown resting queue's rows are about to go, so the replacement asks first"
    );
    assert!(app.queue_deferrals.has_gated_replacement());
}

/// The GUI Client shows an empty resting `Kids` while a populated `Default`
/// holds the playhead, so the replacement runs without asking: the current
/// queue's rows are not the ones it discards (spec `gui-queue-chooser`, the
/// same requirement).
#[test]
fn an_empty_resting_queue_takes_the_replacement_without_asking() {
    let (mut app, cmd_rx) = gui_app_with_named_queues();
    app.show_named_queue(resting());
    app.adopt_named_queue_snapshot(resting(), &resting_snapshot(&[]));
    adopt_current_queue(&mut app, &emby_unified_state(&make_items(3), 0));
    drain_commands(&cmd_rx);

    app.request_queue_replacement(replacement_of(&make_items(1)), ReplacementExecutor::Pending);

    assert!(
        app.pending_overlay.is_none(),
        "an empty shown queue needs no confirmation"
    );
    assert!(!app.queue_deferrals.has_gated_replacement());
    let sent = cmd_rx
        .try_recv()
        .expect("the confirmed-empty replacement reaches the shown queue");
    assert_eq!(
        sent_replace_queue(&sent),
        Some(resting()),
        "the replacement names the shown resting queue, not the populated current one"
    );
}

/// A dirty saved playlist holding the playhead does not raise its save prompt
/// for a replacement of another queue: the save would take the current queue's
/// edits and the replacement discards the shown resting queue instead.
#[test]
fn a_resting_queue_replacement_skips_the_current_queue_save_prompt() {
    let (mut app, cmd_rx) = gui_app_with_named_queues();
    app.show_named_queue(resting());
    app.adopt_named_queue_snapshot(resting(), &resting_snapshot(&[]));
    adopt_current_queue(&mut app, &emby_unified_state(&make_items(3), 0));
    app.local_view
        .adopt_source(mbv_queue::QueueSource::Playlist {
            id: Some("pl-mix".to_string()),
            name: "Mix".to_string(),
        });
    app.queue_dirty = true;
    drain_commands(&cmd_rx);

    app.request_queue_replacement(replacement_of(&make_items(1)), ReplacementExecutor::Pending);

    assert!(
        !app.queue_deferrals.is_save_deferred(),
        "the current queue's unsaved edits are not the ones the replacement discards"
    );
    assert!(app.pending_overlay.is_none());
    let sent = cmd_rx
        .try_recv()
        .expect("the replacement runs instead of prompting to save");
    assert_eq!(sent_replace_queue(&sent), Some(resting()));
}

/// The Named queue one sent queue replacement carries.
fn sent_replace_queue(cmd: &mbv_ctrl::CtrlCmd) -> Option<NamedQueueId> {
    match cmd {
        mbv_ctrl::CtrlCmd::UnifiedQueueReplace { queue, .. } => *queue,
        _ => None,
    }
}
