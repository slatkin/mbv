//! Answered owner queue operations (queue-owner-process row 5.1, design D6).
//!
//! `App::queue_op` sends one [`QueueOp`] to the scope's Player owner and,
//! when the owner advertises `answered-queue-ops`, pumps that link's event
//! receiver until the correlated `QueueOpResult` arrives or
//! [`QUEUE_OP_ANSWER_BOUND`] passes. Snapshots seen along the way are adopted
//! inline so the answer applies to current state; every other event is
//! deferred to [`App::deferred_player_events`] for the next tick's drain.

use super::{App, QueueScope, ToastSeverity};
use mbv_ctrl::player::PlayerEvent;
use mbv_ctrl::{QueueOpId, UnifiedQueueStateData};
use std::time::{Duration, Instant};

/// How long `await_queue_op` waits for the owner's answer before reporting
/// the owner did not respond and leaving the edit unapplied (design D6).
pub(in crate::app) const QUEUE_OP_ANSWER_BOUND: Duration = Duration::from_millis(250);

impl App {
    /// Send `operation` to `scope`'s Player owner, then adopt its answer.
    /// Owners without the capability get the legacy form and no wait (the
    /// displayed queue follows their later snapshots instead).
    pub(in crate::app) fn queue_op(
        &mut self,
        scope: QueueScope,
        operation: mbv_remote_player::QueueOp,
    ) {
        let remote = self.queue_link(scope).0.as_remote();
        let Some(remote) = remote else {
            return;
        };
        let answer = match remote.send_queue_op(operation) {
            Ok(answer) => answer,
            Err(e) => {
                self.flash_error(&e);
                return;
            }
        };
        let Some(id) = answer else {
            // Legacy owner: the edit is sent, and there is no answer to wait for.
            return;
        };
        self.await_queue_op(scope, id);
    }

    /// Pump `scope`'s link receiver until `QueueOpResult{op: id}` or
    /// [`QUEUE_OP_ANSWER_BOUND`]. Adopted inline: `UnifiedQueueUpdated` and
    /// the matching `Applied`. Deferred to the next tick: every other event.
    /// A late or unmatched `Applied` is adopted as a background snapshot; a
    /// late `Rejected` reports nothing more.
    pub(in crate::app) fn await_queue_op(&mut self, scope: QueueScope, id: QueueOpId) {
        self.await_queue_op_with_bound(scope, id, QUEUE_OP_ANSWER_BOUND);
    }

    /// Test seam: `bound` is injectable so tests can force the timeout
    /// without real sleeps (zeroed bound on an empty receiver).
    pub(in crate::app) fn await_queue_op_with_bound(
        &mut self,
        scope: QueueScope,
        id: QueueOpId,
        bound: Duration,
    ) {
        let deadline = Instant::now() + bound;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let event = self.queue_link(scope).1.recv_timeout(remaining);
            match event {
                Ok(PlayerEvent::UnifiedQueueUpdated(snapshot)) => {
                    self.adopt_queue_op_snapshot(scope, &snapshot);
                }
                Ok(PlayerEvent::QueueOpResult { op, outcome }) if op == id => {
                    match outcome {
                        mbv_ctrl::QueueOpOutcome::Applied(snapshot) => {
                            self.adopt_queue_op_snapshot(scope, &snapshot);
                        }
                        mbv_ctrl::QueueOpOutcome::Rejected(reason) => {
                            self.flash(reason, ToastSeverity::Error);
                        }
                    }
                    return;
                }
                Ok(PlayerEvent::QueueOpResult { outcome, .. }) => {
                    // Another op's answer (or a duplicate): adopt its state
                    // like any other owner snapshot, and keep waiting.
                    if let mbv_ctrl::QueueOpOutcome::Applied(snapshot) = outcome {
                        self.adopt_queue_op_snapshot(scope, &snapshot);
                    }
                }
                Ok(other) => {
                    self.deferred_player_events.push_back(other);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    self.flash(
                        "Playback owner did not respond to the queue edit".to_string(),
                        ToastSeverity::Error,
                    );
                    return;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return;
                }
            }
        }
    }

    /// Adopt one owner snapshot seen by the answer pump: through
    /// `adopt_home_snapshot` for a suspended home link, like the tick drain,
    /// and through the live `UnifiedQueueUpdated` adoption otherwise.
    fn adopt_queue_op_snapshot(&mut self, scope: QueueScope, unified: &UnifiedQueueStateData) {
        if scope == QueueScope::Local && self.suspended_local.is_some() {
            self.adopt_home_snapshot(unified);
        } else {
            let _ = self.handle_unified_queue_updated(unified);
        }
    }
}
