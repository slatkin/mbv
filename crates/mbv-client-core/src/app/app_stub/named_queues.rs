//! The Named-queue fixtures a Client's own test builds (`2-split-tui-gui-clients`
//! design D4b rule 5): the stub Owner's queue list, one resting queue's
//! snapshot, the event that answers one of the Client's own queue ops, and the
//! whole-`App` constructions the GUI's queue controls and the core's
//! shown-entry tests share. They sit in the core so one fixture serves every
//! test that builds it, and they compile under the `test` feature so a
//! dependent Client's `test` build reaches them.

use std::sync::mpsc;

use crate::app::App;
use crate::app::app_stub::fixtures::{
    close_initial_services, emby_unified_state, make_items, make_local_daemon_app_stub_with_cmd_rx,
    remote_stub_config,
};
use crate::app::app_stub::gui_client_capabilities;
use mbv_ctrl::player::PlayerEvent;
use mbv_ctrl::{NamedQueueEntry, UnifiedQueueStateData};
use mbv_emby_model::EmbyItem;
use mbv_queue::NamedQueueId;

/// The stub Owner's current queue.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn current() -> NamedQueueId {
    NamedQueueId::from_raw(1)
}

/// The stub Owner's resting queue.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn resting() -> NamedQueueId {
    NamedQueueId::from_raw(2)
}

/// The queue list the stub Owner publishes: `current()` holds three slots, the
/// resting queue holds one.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn named_queue_list() -> Vec<NamedQueueEntry> {
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
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn resting_snapshot(items: &[EmbyItem]) -> UnifiedQueueStateData {
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

/// A GUI Client attached to a local Owner that advertises answered queue ops,
/// with the Named-queue list adopted and its constructor traffic drained.
/// Every named-queue edit is an answered op, so a test answers the Client's
/// own op through [`inject_local_events`].
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn gui_app_with_named_queues() -> (App, mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
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
    app.set_client_capabilities(gui_client_capabilities());
    app.adopt_named_queue_list(named_queue_list(), current());
    (app, cmd_rx)
}

/// Replace the Local link's event receiver with an injected one, so a test can
/// answer the Client's own queue op; keep the sender alive to deliver it.
#[cfg(any(test, feature = "test"))]
pub fn inject_local_events(app: &mut App) -> mpsc::Sender<PlayerEvent> {
    let (tx, rx) = mpsc::channel();
    app.set_active_player_events_for_test(rx);
    tx
}

/// The answer to the Client's own queue op `op`, carrying `data`.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn op_answer(op: u64, data: UnifiedQueueStateData) -> PlayerEvent {
    PlayerEvent::QueueOpResult {
        op: mbv_ctrl::QueueOpId(op),
        outcome: mbv_ctrl::QueueOpOutcome::Applied(Box::new(data)),
    }
}

/// The local Owner's own position, distinct from the peer's.
#[cfg(any(test, feature = "test"))]
pub const HOME_POSITION: i64 = 7;

/// The connected peer's own position, distinct from the local Owner's.
#[cfg(any(test, feature = "test"))]
pub const PEER_POSITION: i64 = 42;

/// A GUI Client attached to the local Owner whose link plays at
/// [`HOME_POSITION`], then connected to a peer that plays at
/// [`PEER_POSITION`]: each link's commands land on its own channel. The
/// connect shows the peer entry (design D7 of `gui-named-queues`), as the
/// playback-target chooser does.
///
/// # Panics
/// Panics if the endpoint literal fails to parse: it is a constant, so this
/// never happens.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn gui_app_with_peer() -> (
    App,
    mpsc::Receiver<mbv_ctrl::CtrlCmd>,
    mpsc::Receiver<mbv_ctrl::CtrlCmd>,
) {
    let (mut app, home_cmd_rx) = make_local_daemon_app_stub_with_cmd_rx(make_items(3));
    while home_cmd_rx.try_recv().is_ok() {}
    app.edit_player_status_for_test(|status| status.position_ticks = HOME_POSITION);
    app.set_client_capabilities(gui_client_capabilities());
    app.adopt_named_queue_list(named_queue_list(), current());

    let (peer, peer_rx, peer_cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_with_command_rx(make_items(2), 1);
    app.attach_direct_remote_for_test(
        "peer-host",
        peer,
        peer_rx,
        &mbv_remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    app.edit_player_status_for_test(|status| status.position_ticks = PEER_POSITION);
    while peer_cmd_rx.try_recv().is_ok() {}
    (app, home_cmd_rx, peer_cmd_rx)
}
