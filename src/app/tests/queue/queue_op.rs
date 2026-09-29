//! Row 5.1 (design D6): the Client's answered queue-op pump.
//!
//! Every test owns `unified-playback-queue` "Queue edits are answered before
//! the next input". Receivers are injected, and the timeout is forced with a
//! zeroed bound — no real sleeps.

use crate::app::dispatch::notify::ToastSeverity;
use crate::app::tests::tick_integration::harness::TickHarness;
use crate::app::tests::{
    close_initial_services, emby_unified_state, make_items, make_local_daemon_app_stub,
    make_local_daemon_app_stub_with_cmd_rx, remote_stub_config,
};
use crate::app::{App, QueueScope};
use mbv_ctrl::player::PlayerEvent;
use mbv_ctrl::{CtrlCmd, QueueOpId, QueueOpOutcome};
use mbv_queue::QueueItem;
use mbv_remote_player::QueueOp;
use std::sync::mpsc;
use std::time::Duration;

/// Local-daemon stub whose owner advertises `answered-queue-ops`.
fn answered_local_daemon_app() -> (App, mpsc::Receiver<CtrlCmd>) {
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
fn inject_player_rx(app: &mut App) -> mpsc::Sender<PlayerEvent> {
    let (tx, rx) = mpsc::channel();
    app.player_rx = rx;
    tx
}

fn snapshot(item_count: usize) -> Box<mbv_ctrl::UnifiedQueueStateData> {
    Box::new(emby_unified_state(&make_items(item_count), 0))
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
        QueueScope::Local,
        QueueOp::Append {
            items: one_queue_item(),
            before: None,
        },
    );

    assert_eq!(
        app.queue_for_scope(QueueScope::Local).total_queue_len(),
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

    app.await_queue_op(QueueScope::Local, QueueOpId(9));

    assert_eq!(
        app.queue_for_scope(QueueScope::Local).total_queue_len(),
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
    app.player_tab.set_items(make_items(2), 0);
    let tx = inject_player_rx(&mut app);

    app.await_queue_op_with_bound(QueueScope::Local, QueueOpId(3), Duration::ZERO);

    assert_eq!(
        app.status, "Playback owner did not respond to the queue edit",
        "the timeout reports the unanswered edit"
    );
    assert_eq!(app.status_severity, ToastSeverity::Error);
    assert_eq!(
        app.queue_for_scope(QueueScope::Local).total_queue_len(),
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
            .queue_for_scope(QueueScope::Local)
            .total_queue_len(),
        5,
        "the late Applied is adopted as a background snapshot"
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
        QueueScope::Local,
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
