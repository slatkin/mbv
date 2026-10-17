// Shared fixtures for the Emby Session output tests: the queue items, the
// Session entries, the scripted-queue setup, and the call-log views that the
// hand-off, end, and device-window test modules all drive.

use super::*;

use super::cast_output::{connect_output_client_with_caps, select_output};
use super::cast_playback::drained_events;
use super::emby_session_output::{
    host_session_output, pump_session_event, session_selection, wait_for_session_jobs,
};
use super::r#loop::{TestLoop, test_loop_with_queue};

use crate::emby_session::fake::{FakeSessionApi, SessionCall, session};
use mbv_emby::SessionInfo;

/// An Emby queue item with a runtime, so the position estimate is capped and
/// the captured hand-off position or the end rule has one to compare with.
pub(super) fn timed(name: &str, runtime_ticks: i64) -> QueueItem {
    let mut emby = item(name, "Video", "Movie");
    emby.runtime_ticks = runtime_ticks;
    QueueItem::Emby(Box::new(emby))
}

/// The Session entry that presents `item_id` at `position_ticks`.
pub(super) fn session_presenting_at(
    item_id: &str,
    position_ticks: i64,
    runtime_ticks: i64,
) -> SessionInfo {
    SessionInfo {
        now_playing: Some(item_id.to_string()),
        now_playing_item_id: Some(item_id.to_string()),
        now_playing_item_type: Some("Movie".to_string()),
        position_ticks,
        runtime_ticks,
        ..session("device-1", "session-1")
    }
}

/// Selects the fake's Session as the output over a scripted queue, returning
/// the loop, the fake's call log, and the selecting Client's event receiver.
pub(super) fn session_output_with_queue(
    fake: FakeSessionApi,
    items: Vec<QueueItem>,
) -> (
    TestLoop,
    Arc<Mutex<Vec<SessionCall>>>,
    mpsc::Receiver<CtrlOutbound>,
) {
    let calls = fake.calls();
    let mut t = test_loop_with_queue(DaemonRole::Local, items, 0);
    host_session_output(&mut t, fake);
    let (client_id, reply_tx, rx) =
        connect_output_client_with_caps(&t.event_loop.ctrl_clients, true, true);
    select_output(&mut t, client_id, &reply_tx, session_selection());
    pump_session_event(&mut t);
    (t, calls, rx)
}

/// Plays `slot` through the queue's own play-slot command and lets the
/// Session's job run.
pub(super) fn play_slot(t: &mut TestLoop, client_id: u64, slot: mbv_queue::QueueSlotId) {
    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::UnifiedQueuePlaySlot {
            slot_id: mbv_ctrl::slot_id_to_u64(slot),
            op: None,
        },
        client_id,
        mpsc::channel().0,
    ));
    wait_for_session_jobs(&t.event_loop.owner.output);
}

/// The one `play` call the Session receives for `item_ids` (design D7, D14),
/// from the start: the active item and, when the queue has one, the next
/// admissible slot beside it.
pub(super) fn play(item_ids: &[&str]) -> SessionCall {
    play_at(item_ids, 0)
}

/// The one `play` call the Session receives for `item_ids` from
/// `start_ticks`.
pub(super) fn play_at(item_ids: &[&str], start_ticks: i64) -> SessionCall {
    SessionCall::Play {
        session_id: "session-1".to_string(),
        item_ids: item_ids.iter().map(|id| (*id).to_string()).collect(),
        start_ticks,
    }
}

/// The one `play_last` call the Session receives for `item_ids` (design D15):
/// the slots appended after the item the device holds next.
pub(super) fn play_last(item_ids: &[&str]) -> SessionCall {
    SessionCall::PlayLast {
        session_id: "session-1".to_string(),
        item_ids: item_ids.iter().map(|id| (*id).to_string()).collect(),
    }
}

/// The `play_last` calls the Session received, in order.
pub(super) fn play_last_calls(calls: &Arc<Mutex<Vec<SessionCall>>>) -> Vec<SessionCall> {
    calls
        .lock()
        .unwrap()
        .iter()
        .filter(|call| matches!(call, SessionCall::PlayLast { .. }))
        .cloned()
        .collect()
}

/// The `CommandRejected` reports a client received, in order.
pub(super) fn rejections(rx: &mpsc::Receiver<CtrlOutbound>) -> Vec<String> {
    drained_events(rx)
        .into_iter()
        .filter_map(|event| match event {
            CtrlEvent::CommandRejected(reason) => Some(reason),
            _ => None,
        })
        .collect()
}

/// Removes `slot` from the owner's queue through the queue edit command, so a
/// test drives the queue edit that makes the queued next item stale (design
/// D14).
pub(super) fn remove_slot(t: &mut TestLoop, client_id: u64, slot: mbv_queue::QueueSlotId) {
    t.event_loop.handle_event(DaemonEvent::Ctrl(
        CtrlCmd::UnifiedQueueRemoveSlot {
            slot_id: mbv_ctrl::slot_id_to_u64(slot),
            op: None,
        },
        client_id,
        mpsc::channel().0,
    ));
}
