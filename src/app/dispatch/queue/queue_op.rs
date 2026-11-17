//! Answered owner queue operations (queue-owner-process row 5.1, design D6).
//!
//! `App::queue_op` sends one [`QueueOp`] to the link's Player owner and,
//! when the owner advertises `answered-queue-ops`, pumps that link's event
//! receiver until the correlated `QueueOpResult` arrives or
//! [`QUEUE_OP_ANSWER_BOUND`] passes. Snapshots seen along the way are adopted
//! inline so the answer applies to current state; every other event is
//! deferred to the drain for the link that produced it.

use super::{App, QueueLink, ToastSeverity};
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
    /// Send a whole-queue replacement to `link`'s Player owner as an
    /// answered op (row 5.3, design D6): the owner replaces its canonical
    /// queue, begins playback at `start_idx`, and answers with the snapshot
    /// the Client adopts — the Client holds no editable queue of its own.
    /// Slot identities are minted client-side (1..=n).
    ///
    /// `queue` names the Named queue the replacement reaches (design D7 of
    /// `gui-named-queues`): the GUI Client's shown queue, so a replacement of
    /// a resting queue makes it current and plays it, or `None` for the
    /// current queue.
    pub(in crate::app) fn replace_queue_on_owner(
        &mut self,
        link: QueueLink,
        items: Vec<mbv_queue::QueueItem>,
        start_idx: usize,
        source: mbv_queue::QueueSource,
        queue: Option<mbv_queue::NamedQueueId>,
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
            link,
            mbv_remote_player::QueueOp::Replace {
                items: slots,
                start_idx: Some(start_idx),
                source,
                start_position_secs: None,
            },
            queue,
        )
    }

    pub(in crate::app) fn replace_emby_queue_on_owner(
        &mut self,
        link: QueueLink,
        items: Vec<mbv_emby_model::EmbyItem>,
        start_idx: usize,
        source: mbv_queue::QueueSource,
        queue: Option<mbv_queue::NamedQueueId>,
    ) -> QueueOpEdit {
        self.replace_queue_on_owner(
            link,
            items
                .into_iter()
                .map(|item| mbv_queue::QueueItem::Emby(Box::new(item)))
                .collect(),
            start_idx,
            source,
            queue,
        )
    }

    /// Send `operation` to `link`'s Player owner, then adopt its answer.
    /// Owners without the capability get the legacy form and no wait (the
    /// displayed queue follows their later snapshots instead). Returns
    /// whether the owner's queue now holds the edit (design D6):
    /// [`QueueOpEdit::Applied`] on an `Applied` answer, [`QueueOpEdit::SentLegacy`]
    /// for a legacy owner, and [`QueueOpEdit::NotApplied`] on rejection,
    /// timeout, or a failed send.
    ///
    /// `queue` names the Named queue the operation reaches (design D7 of
    /// `gui-named-queues`): the GUI Client's shown queue for an edit of the
    /// shown queue, the undone entry's queue for an undo, or `None` for the
    /// current queue. Each caller states its own target, so an edit resolved
    /// against one queue's slots can never be sent as another queue's edit.
    pub(in crate::app) fn queue_op(
        &mut self,
        link: QueueLink,
        operation: mbv_remote_player::QueueOp,
        queue: Option<mbv_queue::NamedQueueId>,
    ) -> QueueOpEdit {
        let remote = self.queue_link(link).0.remote();
        let answer = match remote.send_queue_op(operation, queue) {
            Ok(answer) => answer,
            Err(e) => {
                tracing::warn!(name: "queue.edit.send_failed", target: "queue", error = %e, "the queue edit did not reach the Player owner");
                self.flash_error(&e);
                return QueueOpEdit::NotApplied;
            }
        };
        let Some(id) = answer else {
            // Legacy owner: the edit is sent, and there is no answer to wait for.
            return QueueOpEdit::SentLegacy;
        };
        if self.await_queue_op(link, id) {
            QueueOpEdit::Applied
        } else {
            tracing::warn!(name: "queue.edit.not_applied", target: "queue", "the Player owner did not apply the queue edit in the answer bound");
            QueueOpEdit::NotApplied
        }
    }

    /// Pump `link`'s link receiver until `QueueOpResult{op: id}` or
    /// [`QUEUE_OP_ANSWER_BOUND`]. Adopted inline: `UnifiedQueueUpdated` and
    /// the matching `Applied`. Other events are deferred to the next tick's
    /// drain for the link that produced them. Returns whether the matching
    /// answer was `Applied`; a late or unmatched
    /// `Applied` is adopted as a background snapshot, a late `Rejected`
    /// reports nothing more.
    pub(in crate::app) fn await_queue_op(&mut self, link: QueueLink, id: QueueOpId) -> bool {
        self.await_queue_op_with_bound(link, id, QUEUE_OP_ANSWER_BOUND)
    }

    /// Test seam: `bound` is injectable so tests can force the timeout
    /// without real sleeps (zeroed bound on an empty receiver).
    pub(in crate::app) fn await_queue_op_with_bound(
        &mut self,
        link: QueueLink,
        id: QueueOpId,
        bound: Duration,
    ) -> bool {
        self.pump_queue_op_answer(link, id, bound)
    }

    /// Pump `link`'s link receiver until `QueueOpResult{op: id}` or `bound`.
    /// Adopted inline: `UnifiedQueueUpdated`, the local link's Named-queue
    /// traffic, and the matching `Applied`. The new-queue control's answer is
    /// routed to its own request when it is the foreign op that arrives first
    /// (design D8 of `gui-named-queues`). Deferred to the next tick's
    /// source-link drain: every other event. Returns whether the matching
    /// answer was `Applied`; a late or unmatched `Applied` is adopted as a
    /// background snapshot, a late `Rejected` reports nothing more.
    fn pump_queue_op_answer(&mut self, link: QueueLink, id: QueueOpId, bound: Duration) -> bool {
        let deferred_home = link == QueueLink::Local && self.suspended_local.is_some();
        let deadline = Instant::now() + bound;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let event = self.queue_link(link).1.recv_timeout(remaining);
            match event {
                Ok(PlayerEvent::UnifiedQueueUpdated(snapshot)) => {
                    self.adopt_queue_op_snapshot(link, &snapshot, false);
                }
                Ok(PlayerEvent::QueueOpResult { op, outcome }) if op == id => match outcome {
                    mbv_ctrl::QueueOpOutcome::Applied(snapshot) => {
                        self.adopt_queue_op_snapshot(link, &snapshot, true);
                        return true;
                    }
                    mbv_ctrl::QueueOpOutcome::Rejected(reason) => {
                        self.flash(reason, ToastSeverity::Error);
                        return false;
                    }
                },
                Ok(PlayerEvent::QueueOpResult { op, outcome }) => {
                    self.adopt_foreign_answer(link, op, outcome);
                }
                Ok(PlayerEvent::NamedQueue(event)) if link == QueueLink::Local => {
                    // Owner Named-queue state, adopted inline like a snapshot:
                    // the queue list must not wait behind the deferred drain,
                    // because the create answer that follows it shows the new
                    // queue. A stale list leaves that show with no resting
                    // view and no subscription, so the panel would paint the
                    // current queue's rows under the new queue's name (design
                    // D7, D8).
                    self.adopt_named_queue_event(event);
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

    /// Adopt one answer that is not the op this pump waits for (design D8 of
    /// `gui-named-queues`): another op's answer, or a duplicate. The
    /// new-queue control's own answer can be the one that arrives here while an
    /// edit's pump waits, so it must reach its request instead of being adopted
    /// as background, or the control would wait for it forever and the queue it
    /// made would never be shown. A refused Named-queue command reports here
    /// too, because it has no pump of its own and this pump reads the link that
    /// would have answered it. Every other answer is adopted like any other
    /// owner snapshot.
    fn adopt_foreign_answer(
        &mut self,
        link: QueueLink,
        op: QueueOpId,
        outcome: mbv_ctrl::QueueOpOutcome,
    ) {
        match outcome {
            mbv_ctrl::QueueOpOutcome::Applied(snapshot) => {
                if !self.take_new_queue_answer(op, Some(&snapshot)) {
                    self.adopt_queue_op_snapshot(link, &snapshot, false);
                }
            }
            mbv_ctrl::QueueOpOutcome::Rejected(reason) => {
                self.report_named_queue_rejection(op, reason);
            }
        }
    }

    /// Adopt one owner snapshot seen by the answer pump: through
    /// `adopt_home_snapshot` for a suspended home link, like the tick drain,
    /// and through the live `UnifiedQueueUpdated` adoption otherwise. A
    /// snapshot that names a Named queue is a resting queue's answer (design
    /// D3, D5): its slots refill the shown resting view, and the current
    /// queue's view, its source fence, and the playback status stay as they
    /// were, because a resting queue holds no playhead.
    fn adopt_queue_op_snapshot(
        &mut self,
        link: QueueLink,
        unified: &UnifiedQueueStateData,
        own_answer: bool,
    ) {
        if let Some(queue) = unified.queue {
            self.adopt_named_queue_snapshot(queue, unified);
            return;
        }
        if own_answer {
            // The answer's status belongs to the link that answered, whatever
            // the Client shows: an edit of the local link while a peer holds
            // the active one must never write the local Owner's status into
            // the peer's mirror (design D7 of `gui-named-queues`).
            self.queue_owner_player(link)
                .set_status(unified.status.clone());
            self.queue_for_link_mut(link).adopt(
                unified,
                crate::app::state::queue_view::AdoptCause::OwnAnswer,
            );
            if link == QueueLink::Local {
                self.adopt_owner_source(unified);
            }
        } else if link == QueueLink::Local && self.suspended_local.is_some() {
            self.adopt_home_snapshot(unified);
        } else {
            let _ = self.handle_unified_queue_updated(link, unified);
        }
    }
}
