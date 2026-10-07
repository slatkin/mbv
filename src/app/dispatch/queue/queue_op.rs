//! Answered owner queue operations (queue-owner-process row 5.1, design D6).
//!
//! `App::queue_op` sends one [`QueueOp`] to the scope's Player owner and,
//! when the owner advertises `answered-queue-ops`, pumps that link's event
//! receiver until the correlated `QueueOpResult` arrives or
//! [`QUEUE_OP_ANSWER_BOUND`] passes. Snapshots seen along the way are adopted
//! inline so the answer applies to current state; every other event is
//! deferred to the drain for the link that produced it.

use super::{App, QueueScope, ToastSeverity};
use mbv_ctrl::player::PlayerEvent;
use mbv_ctrl::{QueueOpId, UnifiedQueueStateData};
use std::time::{Duration, Instant};

/// Whether an edit sent through [`App::send_queue_edit`] reached the
/// owner's queue: `Applied` when the owner answered `Applied`, `SentLegacy`
/// when a legacy owner took the edit with no answer to wait for, and
/// `NotApplied` when the owner rejected it or did not answer in time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum QueueOpEdit {
    Applied,
    SentLegacy,
    NotApplied,
}

/// How long `await_queue_op` waits for the owner's answer before reporting
/// the owner did not respond and leaving the edit unapplied (design D6).
pub(in crate::app) const QUEUE_OP_ANSWER_BOUND: Duration = Duration::from_millis(250);

impl App {
    /// Send a whole-queue replacement to `scope`'s Player owner as an
    /// answered op (row 5.3, design D6): the owner replaces its canonical
    /// queue, begins playback at `start_idx`, and answers with the snapshot
    /// the Client adopts — the Client holds no editable queue of its own.
    /// Slot identities are minted client-side (1..=n), like the idle load.
    pub(in crate::app) fn replace_queue_on_owner(
        &mut self,
        scope: QueueScope,
        items: Vec<mbv_queue::QueueItem>,
        start_idx: usize,
        source: mbv_queue::QueueSource,
    ) -> QueueOpEdit {
        let slots: Vec<mbv_ctrl::UnifiedQueueSlot> = items
            .into_iter()
            .enumerate()
            .map(|(index, item)| mbv_ctrl::UnifiedQueueSlot {
                slot_id: (index + 1) as u64,
                item,
            })
            .collect();
        self.queue_op(
            scope,
            mbv_remote_player::QueueOp::Replace {
                items: slots,
                start_idx: Some(start_idx),
                source,
            },
        )
    }

    pub(in crate::app) fn replace_emby_queue_on_owner(
        &mut self,
        scope: QueueScope,
        items: Vec<mbv_emby_model::EmbyItem>,
        start_idx: usize,
        source: mbv_queue::QueueSource,
    ) -> QueueOpEdit {
        self.replace_queue_on_owner(
            scope,
            items
                .into_iter()
                .map(|item| mbv_queue::QueueItem::Emby(Box::new(item)))
                .collect(),
            start_idx,
            source,
        )
    }

    /// Send `operation` to `scope`'s Player owner, then adopt its answer.
    /// Owners without the capability get the legacy form and no wait (the
    /// displayed queue follows their later snapshots instead). Returns
    /// whether the owner's queue now holds the edit (design D6):
    /// [`QueueOpEdit::Applied`] on an `Applied` answer, [`QueueOpEdit::SentLegacy`]
    /// for a legacy owner, and [`QueueOpEdit::NotApplied`] on rejection,
    /// timeout, or a failed send.
    pub(in crate::app) fn queue_op(
        &mut self,
        scope: QueueScope,
        operation: mbv_remote_player::QueueOp,
    ) -> QueueOpEdit {
        let remote = self.queue_link(scope).0.remote();
        let answer = match remote.send_queue_op(operation) {
            Ok(answer) => answer,
            Err(e) => {
                self.flash_error(&e);
                return QueueOpEdit::NotApplied;
            }
        };
        let Some(id) = answer else {
            // Legacy owner: the edit is sent, and there is no answer to wait for.
            return QueueOpEdit::SentLegacy;
        };
        if self.await_queue_op(scope, id) {
            QueueOpEdit::Applied
        } else {
            QueueOpEdit::NotApplied
        }
    }

    /// Pump `scope`'s link receiver until `QueueOpResult{op: id}` or
    /// [`QUEUE_OP_ANSWER_BOUND`]. Adopted inline: `UnifiedQueueUpdated` and
    /// the matching `Applied`. Other events are deferred to the next tick's
    /// drain for the link that produced them. Returns whether the matching
    /// answer was `Applied`; a late or unmatched
    /// `Applied` is adopted as a background snapshot, a late `Rejected`
    /// reports nothing more.
    pub(in crate::app) fn await_queue_op(&mut self, scope: QueueScope, id: QueueOpId) -> bool {
        self.await_queue_op_with_bound(scope, id, QUEUE_OP_ANSWER_BOUND)
    }

    /// Test seam: `bound` is injectable so tests can force the timeout
    /// without real sleeps (zeroed bound on an empty receiver).
    pub(in crate::app) fn await_queue_op_with_bound(
        &mut self,
        scope: QueueScope,
        id: QueueOpId,
        bound: Duration,
    ) -> bool {
        self.pump_queue_op_answer(scope, id, bound)
    }

    /// Pump the current Player link's event receiver until the correlated
    /// `UnifiedQueueLoadResult` arrives or [`QUEUE_OP_ANSWER_BOUND`] passes —
    /// the same answer discipline as the queue-op pump, for the idle-load
    /// request sent through [`App::send_idle_queue_load`]. The matched answer
    /// is flashed exactly as the tick drain would flash it; every other event
    /// is deferred to the next tick's source-link drain. Returns whether the
    /// load was accepted.
    pub(in crate::app) fn await_queue_load(
        &mut self,
        request_id: mbv_ctrl::QueueLoadRequestId,
    ) -> bool {
        let deadline = Instant::now() + QUEUE_OP_ANSWER_BOUND;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match self.player_rx.recv_timeout(remaining) {
                Ok(PlayerEvent::UnifiedQueueLoadResult {
                    request_id: answered,
                    result,
                }) if answered == request_id => {
                    let accepted = matches!(result, mbv_ctrl::QueueLoadResult::Accepted);
                    self.handle_unified_queue_load_result(result);
                    return accepted;
                }
                Ok(other) => self.deferred_player_events.push_back(other),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    self.flash(
                        "Playback owner did not respond to the queue load".to_string(),
                        ToastSeverity::Error,
                    );
                    return false;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return false;
                }
            }
        }
    }

    /// Pump `scope`'s link receiver until `QueueOpResult{op: id}` or `bound`.
    /// Adopted inline: `UnifiedQueueUpdated` and the matching `Applied`.
    /// Deferred to the next tick's source-link drain: every other event.
    /// Returns whether the matching answer was `Applied`; a late or unmatched
    /// `Applied` is adopted as a background snapshot, a late `Rejected`
    /// reports nothing more.
    fn pump_queue_op_answer(&mut self, scope: QueueScope, id: QueueOpId, bound: Duration) -> bool {
        let deferred_home = scope == QueueScope::Local && self.suspended_local.is_some();
        let deadline = Instant::now() + bound;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let event = self.queue_link(scope).1.recv_timeout(remaining);
            match event {
                Ok(PlayerEvent::UnifiedQueueUpdated(snapshot)) => {
                    self.adopt_queue_op_snapshot(scope, &snapshot, false);
                }
                Ok(PlayerEvent::QueueOpResult { op, outcome }) if op == id => match outcome {
                    mbv_ctrl::QueueOpOutcome::Applied(snapshot) => {
                        self.adopt_queue_op_snapshot(scope, &snapshot, true);
                        return true;
                    }
                    mbv_ctrl::QueueOpOutcome::Rejected(reason) => {
                        self.flash(reason, ToastSeverity::Error);
                        return false;
                    }
                },
                Ok(PlayerEvent::QueueOpResult { outcome, .. }) => {
                    // Another op's answer (or a duplicate): adopt its state
                    // like any other owner snapshot, and keep waiting.
                    if let mbv_ctrl::QueueOpOutcome::Applied(snapshot) = outcome {
                        self.adopt_queue_op_snapshot(scope, &snapshot, false);
                    }
                }
                Ok(other) => {
                    if deferred_home {
                        self.deferred_home_events.push_back(other);
                    } else {
                        self.deferred_player_events.push_back(other);
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    self.flash(
                        "Playback owner did not respond to the queue edit".to_string(),
                        ToastSeverity::Error,
                    );
                    return false;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return false;
                }
            }
        }
    }

    /// Adopt one owner snapshot seen by the answer pump: through
    /// `adopt_home_snapshot` for a suspended home link, like the tick drain,
    /// and through the live `UnifiedQueueUpdated` adoption otherwise.
    fn adopt_queue_op_snapshot(
        &mut self,
        scope: QueueScope,
        unified: &UnifiedQueueStateData,
        own_answer: bool,
    ) {
        if own_answer {
            if scope == self.playing_queue_scope() {
                self.player.set_status(unified.status.clone());
            }
            self.queue_for_scope_mut(scope).adopt(
                unified,
                crate::app::state::queue_view::AdoptCause::OwnAnswer,
            );
            if scope == QueueScope::Local {
                self.adopt_owner_source(unified);
            }
        } else if scope == QueueScope::Local && self.suspended_local.is_some() {
            self.adopt_home_snapshot(unified);
        } else {
            let _ = self.handle_unified_queue_updated(unified);
        }
    }
}
