//! Row 5.1 (design D6): the Client's answered queue-op pump.
//!
//! Every test owns `unified-playback-queue` "Queue edits are answered before
//! the next input". Receivers are injected, and the timeout is forced with a
//! zeroed bound — no real sleeps.

use crate::app::dispatch::notify::ToastSeverity;
use crate::app::tests::tick_integration::harness::TickHarness;
use crate::app::tests::{
    QueueViewTestExt, close_initial_services, emby_unified_state, make_items,
    make_local_daemon_app_stub, make_local_daemon_app_stub_with_cmd_rx, remote_stub_config,
};
use crate::app::{App, QueueLink};
use crate::pin::{PINNED_REQUEST_OVERRIDE, PINNED_REQUEST_TEST_LOCK, PINNED_REQUESTS};
use mbv_ctrl::player::PlayerEvent;
use mbv_ctrl::{CtrlCmd, QueueOpId, QueueOpOutcome};
use mbv_queue::QueueItem;
use mbv_remote_player::QueueOp;
use mbv_ui_model::settings::PinnedWidth;
use std::sync::mpsc;
use std::time::Duration;

/// Local-daemon stub whose owner advertises `answered-queue-ops`.
pub(super) fn answered_local_daemon_app() -> (App, mpsc::Receiver<CtrlCmd>) {
    let (remote, player_rx, cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_answered_queue_ops_with_command_rx(make_items(2), 0);
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
    // Drain the constructor's prefs sync so callers see only commands their
    // own test actions send (same contract as the plain local-daemon stub).
    while cmd_rx.try_recv().is_ok() {}
    (app, cmd_rx)
}

/// Replace the Local link's event receiver with an injected one; keep the
/// sender alive to deliver preloaded events.
pub(super) fn inject_player_rx(app: &mut App) -> mpsc::Sender<PlayerEvent> {
    let (tx, rx) = mpsc::channel();
    app.player_rx = rx;
    tx
}

fn snapshot(item_count: usize) -> Box<mbv_ctrl::UnifiedQueueStateData> {
    Box::new(emby_unified_state(&make_items(item_count), 0))
}

/// A snapshot whose slots are numbered from `base`, so a test can prove an
/// edit was resolved against a specific answered state rather than any
/// earlier queue with the same shape.
fn snapshot_from_base(item_count: usize, base: u64) -> Box<mbv_ctrl::UnifiedQueueStateData> {
    let slots: Vec<mbv_ctrl::UnifiedQueueSlot> = make_items(item_count)
        .into_iter()
        .enumerate()
        .map(|(i, item)| mbv_ctrl::UnifiedQueueSlot {
            slot_id: base + i as u64,
            item: mbv_queue::QueueItem::Emby(Box::new(item)),
        })
        .collect();
    Box::new(mbv_ctrl::UnifiedQueueStateData {
        status: mbv_ctrl::player::PlayerStatus::default(),
        active_slot: slots.first().map(|s| s.slot_id),
        slots,
        revision: 1,
        source: mbv_queue::QueueSource::Remote,
        lineage: mbv_queue::QueueLineage::default(),
        in_flight_transition: None,
        queued_latest_transition: None,
    })
}

fn applied(op: u64, snapshot: Box<mbv_ctrl::UnifiedQueueStateData>) -> PlayerEvent {
    PlayerEvent::QueueOpResult {
        op: QueueOpId(op),
        outcome: QueueOpOutcome::Applied(snapshot),
    }
}

fn one_queue_item() -> Vec<QueueItem> {
    vec![QueueItem::Emby(Box::new(make_items(1)[0].clone()))]
}

#[test]
fn queue_op_adopts_the_earlier_snapshot_inline_before_the_answer() {
    // unified-playback-queue "Queue edits are answered before the next input":
    // a snapshot that arrives ahead of the answer is adopted by the pump
    // itself, not deferred, so the answer applies to current state.
    let (mut app, cmd_rx) = answered_local_daemon_app();
    let tx = inject_player_rx(&mut app);
    tx.send(PlayerEvent::UnifiedQueueUpdated(snapshot(3)))
        .unwrap();
    tx.send(PlayerEvent::QueueOpResult {
        op: QueueOpId(1),
        outcome: QueueOpOutcome::Applied(snapshot(4)),
    })
    .unwrap();

    app.queue_op(
        QueueLink::Local,
        QueueOp::Append {
            items: one_queue_item(),
            before: None,
        },
    );

    assert_eq!(
        app.queue_for_link(QueueLink::Local).total_queue_len(),
        4,
        "the matching answer's snapshot is the adopted state"
    );
    assert!(
        app.deferred_player_events.is_empty(),
        "the earlier snapshot was adopted inline, not deferred"
    );
    assert!(
        matches!(
            cmd_rx.try_recv().unwrap(),
            CtrlCmd::UnifiedQueueAppend {
                op: Some(QueueOpId(1)),
                ..
            }
        ),
        "the answered-capability owner receives the correlated command"
    );
    assert!(
        app.status.is_empty(),
        "a timely answer must not flash a timeout"
    );
}

#[test]
fn suspended_home_queue_op_disconnect_is_drained_as_a_home_event() {
    // queue-owner-process design D4/D6: a suspended home-link event stays on
    // the home drain and cannot disconnect the currently viewed remote owner.
    let (mut app, _cmd_rx) = answered_local_daemon_app();
    let (remote, remote_rx) = mbv_remote_player::RemotePlayer::stub(make_items(2), 0);
    app.switch_to_direct_remote(
        &mbv_emby::test_support::make_session("remote-owner", "mbv"),
        remote,
        remote_rx,
        &mbv_remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    let (tx, rx) = mpsc::channel();
    app.suspended_local.as_mut().unwrap().player_rx = rx;
    tx.send(PlayerEvent::RemoteDisconnected("home owner lost".into()))
        .unwrap();
    tx.send(applied(1, snapshot(2))).unwrap();

    app.queue_op(
        QueueLink::Local,
        QueueOp::Append {
            items: one_queue_item(),
            before: None,
        },
    );

    assert!(
        app.deferred_player_events.is_empty(),
        "a suspended home's disconnect must not be replayed through the live player"
    );
    let mut harness = TickHarness::new(app);
    harness.step();

    assert!(harness.model().app.suspended_local.is_none());
    assert!(
        harness
            .model()
            .application
            .mounted(&mbv_ui_msg::ComponentId::Modal(
                mbv_ui_msg::ModalId::DaemonLost
            ))
    );
}

#[test]
fn await_queue_op_defers_an_interleaved_remote_disconnected_to_the_next_tick() {
    // unified-playback-queue "Queue edits are answered before the next input":
    // an event that is not the answer (here `RemoteDisconnected`) is not
    // handled inside the pump; it is replayed on the next tick, after the
    // answer is adopted.
    let (mut app, _cmd_rx) = answered_local_daemon_app();
    let tx = inject_player_rx(&mut app);
    tx.send(PlayerEvent::RemoteDisconnected("owner gone".into()))
        .unwrap();
    tx.send(PlayerEvent::QueueOpResult {
        op: QueueOpId(9),
        outcome: QueueOpOutcome::Applied(snapshot(4)),
    })
    .unwrap();

    app.await_queue_op(QueueLink::Local, QueueOpId(9));

    assert_eq!(
        app.queue_for_link(QueueLink::Local).total_queue_len(),
        4,
        "the answer is still adopted"
    );
    assert!(
        app.pending_overlay.is_none(),
        "the pump must not handle the disconnect itself"
    );
    assert_eq!(
        app.deferred_player_events.len(),
        1,
        "the disconnect is deferred, in order"
    );

    let mut harness = TickHarness::new(app);
    harness.step();
    assert!(
        harness
            .model()
            .application
            .mounted(&mbv_ui_msg::ComponentId::Modal(
                mbv_ui_msg::ModalId::DaemonLost
            )),
        "the deferred disconnect is handled on the next tick"
    );
}

#[test]
fn await_queue_op_timeout_flashes_and_a_late_applied_is_adopted_afterwards() {
    // unified-playback-queue "Queue edits are answered before the next input":
    // a missing answer times out with a flash and leaves the view unchanged;
    // a late `Applied` is still adopted afterwards as a background snapshot.
    let mut app = make_local_daemon_app_stub(make_items(2));
    app.local_view.adopt_items(make_items(2), 0);
    let tx = inject_player_rx(&mut app);

    app.await_queue_op_with_bound(QueueLink::Local, QueueOpId(3), Duration::ZERO);

    assert_eq!(
        app.status, "Playback owner did not respond to the queue edit",
        "the timeout reports the unanswered edit"
    );
    assert_eq!(app.status_severity, ToastSeverity::Error);
    assert_eq!(
        app.queue_for_link(QueueLink::Local).total_queue_len(),
        2,
        "the timeout leaves the view unchanged"
    );

    tx.send(PlayerEvent::QueueOpResult {
        op: QueueOpId(3),
        outcome: QueueOpOutcome::Applied(snapshot(5)),
    })
    .unwrap();
    let mut harness = TickHarness::new(app);
    harness.step();
    assert_eq!(
        harness
            .model()
            .app
            .queue_for_link(QueueLink::Local)
            .total_queue_len(),
        5,
        "the late Applied is adopted as a background snapshot"
    );
}

#[test]
fn rapid_repeated_removals_each_act_on_the_answered_state() {
    // unified-playback-queue "Queue edits are answered before the next input",
    // scenario "Rapid repeated removals": each removal is resolved and sent
    // against the queue as updated by the previous removal's answer — the
    // Client holds no editable queue of its own.
    let (mut app, cmd_rx) = answered_local_daemon_app();
    let tx = inject_player_rx(&mut app);
    app.local_view.adopt_items(make_items(4), 0);
    // Nothing is playing: the stub player starts active on row 0, which
    // would route row-0 removals into the now-playing confirm flow.
    app.player.update_status(|status| status.active = false);
    // One answer per removal, each holding the queue as the owner keeps it
    // after applying that removal (slot identities re-based per snapshot).
    tx.send(applied(1, snapshot_from_base(3, 100))).unwrap();
    tx.send(applied(2, snapshot_from_base(2, 200))).unwrap();
    tx.send(applied(3, snapshot_from_base(1, 300))).unwrap();

    let first_slot = app.local_view.slot_id_at(0).unwrap();
    app.remove_from_queue(0);
    app.remove_from_queue(0);
    app.remove_from_queue(0);

    let mut sent_slot_ids = Vec::new();
    while let Ok(cmd) = cmd_rx.try_recv() {
        if let mbv_ctrl::CtrlCmd::UnifiedQueueRemoveSlot { slot_id, .. } = cmd {
            sent_slot_ids.push(slot_id);
        }
    }
    assert_eq!(
        sent_slot_ids,
        vec![mbv_ctrl::slot_id_to_u64(first_slot), 100, 200],
        "removal 2 and 3 target slots of the previously answered snapshots"
    );
    assert_eq!(
        app.queue_for_link(QueueLink::Local).total_queue_len(),
        1,
        "three entries were removed"
    );
    assert!(
        app.status.is_empty(),
        "no timeout is flashed when answers arrive"
    );
}

#[test]
fn undoing_a_removal_appends_before_the_entry_now_at_that_index() {
    // unified-playback-queue "Queue undo is an owner operation", scenario
    // "Undo a removal": the inverse edit goes to the owner as an answered
    // Append anchored before the slot now at the removed entry's position.
    let (mut app, cmd_rx) = answered_local_daemon_app();
    let tx = inject_player_rx(&mut app);
    app.local_view.adopt_items(make_items(4), 0);
    app.player.update_status(|status| status.active = false);
    // Owner answer: the third entry (index 2) is gone; the entry that
    // followed it now sits at index 2 with slot 102.
    let mut remaining = make_items(4);
    remaining.remove(2);
    let mut undo_answer = emby_unified_state(&remaining, 0);
    undo_answer.slots.truncate(2);
    undo_answer.slots.push(mbv_ctrl::UnifiedQueueSlot {
        slot_id: 102,
        item: mbv_queue::QueueItem::Emby(Box::new(make_items(4)[3].clone())),
    });
    tx.send(applied(1, Box::new(undo_answer))).unwrap();

    app.remove_from_queue(2);
    app.undo_last_queue_edit(QueueLink::Local);

    let _remove = cmd_rx.try_recv().unwrap();
    let undo = cmd_rx.try_recv().unwrap();
    let mbv_ctrl::CtrlCmd::UnifiedQueueAppend { items, before, op } = undo else {
        panic!("the undo sends an Append, got {undo:?}");
    };
    assert_eq!(items.len(), 1, "the removed item is restored alone");
    assert_eq!(items[0].id(), "id2", "the removed item itself is restored");
    assert_eq!(
        before,
        Some(102),
        "the anchor is the slot now at the removed entry's former position"
    );
    assert!(op.is_some(), "the undo is an answered owner operation");
}

#[test]
fn undoing_a_removal_past_the_end_appends_at_the_end() {
    // unified-playback-queue "Queue undo is an owner operation", scenario
    // "Undo a removal": when the removed entry's former position no longer
    // exists, the anchor is absent and the item is appended at the end.
    let (mut app, cmd_rx) = answered_local_daemon_app();
    let tx = inject_player_rx(&mut app);
    app.local_view.adopt_items(make_items(2), 0);
    app.player.update_status(|status| status.active = false);
    // Owner answer: the last entry is gone; one entry remains (slot 100).
    tx.send(applied(1, snapshot_from_base(1, 100))).unwrap();

    app.remove_from_queue(1);
    app.undo_last_queue_edit(QueueLink::Local);

    let _remove = cmd_rx.try_recv().unwrap();
    let undo = cmd_rx.try_recv().unwrap();
    assert!(
        matches!(
            undo,
            mbv_ctrl::CtrlCmd::UnifiedQueueAppend {
                before: None,
                op: Some(_),
                ..
            }
        ),
        "a former position past the end appends at the end, got {undo:?}"
    );
}

#[test]
fn queue_op_to_a_legacy_owner_returns_without_waiting() {
    // unified-playback-queue "Queue edits are answered before the next input":
    // a peer without the capability gets the legacy form and no wait; the
    // injected receiver stays empty, so any wait here would hang the test.
    let (mut app, cmd_rx) = make_local_daemon_app_stub_with_cmd_rx(make_items(2));
    while cmd_rx.try_recv().is_ok() {}
    let _tx = inject_player_rx(&mut app);

    app.queue_op(
        QueueLink::Local,
        QueueOp::Append {
            items: one_queue_item(),
            before: None,
        },
    );

    assert!(
        app.deferred_player_events.is_empty(),
        "no events were pumped for a legacy owner"
    );
    assert!(app.status.is_empty(), "the legacy send must not flash");
    let received = cmd_rx.try_recv().unwrap();
    assert!(
        matches!(&received, CtrlCmd::UnifiedQueueAppend { op: None, .. }),
        "the legacy form carries no op id, got {received:?}"
    );
}

#[test]
fn queue_refresh_is_an_answered_owner_op_and_shows_only_the_answer() {
    // unified-playback-queue "Clients hold no editable queue", scenario
    // "Queue refresh": the Client asks the owner to refresh its queue, and
    // the refreshed items appear only through the owner's resulting
    // snapshot — the Client merges nothing of its own. Queue refresh is an
    // explicit owner operation, not a side effect of the global F5
    // browse-destination refresh (#745).
    let (mut app, cmd_rx) = answered_local_daemon_app();
    let tx = inject_player_rx(&mut app);
    app.local_view.adopt_items(make_items(2), 0);
    // The owner answers Applied at once with the refreshed queue.
    tx.send(applied(1, snapshot_from_base(3, 500))).unwrap();

    app.queue_op(QueueLink::Local, QueueOp::Refresh);

    assert!(
        matches!(
            cmd_rx.try_recv().unwrap(),
            CtrlCmd::UnifiedQueueRefresh { op: QueueOpId(1) },
        ),
        "the refresh is sent as an answered owner operation"
    );
    assert_eq!(
        app.queue_for_link(QueueLink::Local).total_queue_len(),
        3,
        "the viewed queue is the owner's refreshed snapshot"
    );
    assert_eq!(
        app.queue_for_link(QueueLink::Local)
            .slot_id_at(0)
            .map(mbv_ctrl::slot_id_to_u64),
        Some(500),
        "the adopted snapshot is the owner's answer, not a Client-side merge"
    );
    assert!(app.status.is_empty(), "a timely answer must not flash");
}

#[test]
fn single_item_play_replaces_the_populated_queue_with_the_played_item() {
    // Play parity with the Emby browse single-item path (`play_item`): playing
    // one item replaces the playing-target queue with exactly that item and
    // starts it. The previous append-then-PlaySlot form kept the old queue
    // growing across plays and dropped the jump on a cold owner (Enter on an
    // Audiobookshelf item after an owner restart played nothing).
    let (mut app, cmd_rx) = answered_local_daemon_app();
    let tx = inject_player_rx(&mut app);
    tx.send(PlayerEvent::UnifiedQueueUpdated(snapshot(3)))
        .unwrap();
    tx.send(applied(1, snapshot(1))).unwrap();

    let played = one_queue_item().remove(0);
    assert!(app.submit_queue_item(played, true));

    assert_eq!(
        app.queue_for_link(QueueLink::Local).total_queue_len(),
        1,
        "the played item is the whole queue"
    );
    assert!(
        matches!(
            cmd_rx.try_recv().unwrap(),
            CtrlCmd::UnifiedQueueReplace {
                start_idx: Some(0),
                ..
            }
        ),
        "the play is an answered whole-queue replace-and-start"
    );
    assert!(
        app.status.is_empty(),
        "a timely answer must not flash a timeout"
    );
}

#[test]
fn single_item_play_on_a_legacy_owner_replaces_the_queue_with_the_played_item() {
    // queue-owner-process #857 review follow-up: the legacy wire form must
    // replace the owner queue with exactly the played item (start_idx 0),
    // like the answered form, instead of appending to the displayed queue and
    // resubmitting the whole list.
    let (mut app, cmd_rx) = make_local_daemon_app_stub_with_cmd_rx(make_items(2));
    while cmd_rx.try_recv().is_ok() {}
    let _tx = inject_player_rx(&mut app);

    let played = one_queue_item().remove(0);
    assert!(app.submit_queue_item(played, true));

    match cmd_rx.try_recv().unwrap() {
        CtrlCmd::UnifiedQueueReplace {
            items,
            start_idx,
            op,
            ..
        } => {
            assert_eq!(items.len(), 1, "the played item is the whole queue");
            assert_eq!(start_idx, Some(0), "play starts at the played item");
            assert!(op.is_none(), "the legacy form carries no op id");
        }
        other => panic!("expected the legacy whole-queue replace form, got {other:?}"),
    }
    assert!(app.status.is_empty(), "the legacy send must not flash");
}

#[test]
fn an_accepted_play_at_the_expanded_pinned_width_records_one_collapse_request() {
    // pinned-play-collapses-to-queue row 2.2: a user play's accepted replace
    // reaches the collapse step once. The drain and focus contract is owned
    // by pinned_view_toggle.rs.
    let _guard = PINNED_REQUEST_TEST_LOCK.lock().unwrap();
    *PINNED_REQUEST_OVERRIDE.lock().unwrap() = Some(Ok(()));
    PINNED_REQUESTS.lock().unwrap().clear();
    let (mut app, _cmd_rx) = answered_local_daemon_app();
    let tx = inject_player_rx(&mut app);
    tx.send(applied(1, snapshot(1))).unwrap();
    app.pinned_width = PinnedWidth::Expanded;

    assert!(app.submit_queue_item(one_queue_item().remove(0), true));

    let requests = PINNED_REQUESTS.lock().unwrap().clone();
    assert_eq!(requests.len(), 1, "one collapse request for the play");
    assert_eq!(
        requests[0],
        crate::pin::PinnedSettingRequest::Width(PinnedWidth::Collapsed)
    );
    *PINNED_REQUEST_OVERRIDE.lock().unwrap() = None;
}

#[test]
fn a_not_applied_play_records_no_collapse_request() {
    // pinned-play-collapses-to-queue row 2.2: a rejected replace never
    // reaches the collapse step.
    let _guard = PINNED_REQUEST_TEST_LOCK.lock().unwrap();
    *PINNED_REQUEST_OVERRIDE.lock().unwrap() = Some(Ok(()));
    PINNED_REQUESTS.lock().unwrap().clear();
    let (mut app, _cmd_rx) = answered_local_daemon_app();
    let tx = inject_player_rx(&mut app);
    tx.send(PlayerEvent::QueueOpResult {
        op: QueueOpId(1),
        outcome: QueueOpOutcome::Rejected("the owner refused the replace".into()),
    })
    .unwrap();
    app.pinned_width = PinnedWidth::Expanded;

    assert!(!app.submit_queue_item(one_queue_item().remove(0), true));

    assert!(PINNED_REQUESTS.lock().unwrap().is_empty());
    assert_eq!(app.pinned_width, PinnedWidth::Expanded);
    *PINNED_REQUEST_OVERRIDE.lock().unwrap() = None;
}

#[test]
fn an_enqueue_records_no_collapse_request() {
    // pinned-play-collapses-to-queue row 2.2: enqueue appends without
    // starting playback and must not touch the pinned view.
    let _guard = PINNED_REQUEST_TEST_LOCK.lock().unwrap();
    *PINNED_REQUEST_OVERRIDE.lock().unwrap() = Some(Ok(()));
    PINNED_REQUESTS.lock().unwrap().clear();
    let (mut app, _cmd_rx) = answered_local_daemon_app();
    let tx = inject_player_rx(&mut app);
    tx.send(applied(1, snapshot(3))).unwrap();
    app.pinned_width = PinnedWidth::Expanded;

    assert!(app.submit_queue_item(one_queue_item().remove(0), false));

    assert!(PINNED_REQUESTS.lock().unwrap().is_empty());
    assert_eq!(app.pinned_width, PinnedWidth::Expanded);
    *PINNED_REQUEST_OVERRIDE.lock().unwrap() = None;
}
