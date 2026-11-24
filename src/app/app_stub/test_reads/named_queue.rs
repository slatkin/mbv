//! The Named queue and Player-event seams a Client's test drives where no
//! Client-facing entry point reaches (`split-tui-gui-clients` design D4b rule
//! 5): the Owner's published list, an edit the Client sends, the player-event
//! handler's own arm, and the pending-request facts a queue control leaves.

use super::super::App;

impl App {
    /// Hold the Owner's published Named queue `entries` with `current` as the
    /// queue that holds the playhead, as the list event leaves them.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_named_queue_list_for_test(
        &mut self,
        entries: Vec<mbv_ctrl::NamedQueueEntry>,
        current: mbv_queue::NamedQueueId,
    ) {
        self.adopt_named_queue_list(entries, current);
    }

    /// Append `items` to the shown queue's Owner slot, `before` naming the
    /// slot they enter ahead of, and answer whether the edit reached the
    /// Owner: the path a Library enqueue runs.
    #[cfg(any(test, feature = "test"))]
    pub fn append_on_owner_for_test(
        &mut self,
        items: Vec<mbv_queue::QueueItem>,
        before: Option<u64>,
    ) -> bool {
        self.append_on_owner(items, before)
    }

    /// The resting Named queue whose own view the Queue panel shows, or `None`
    /// while the panel shows the current queue's own view.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn shown_resting_queue_id_for_test(&self) -> Option<mbv_queue::NamedQueueId> {
        self.shown_resting_queue_id()
    }

    /// Whether the new-queue control holds no unanswered request.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn pending_new_queue_is_none_for_test(&self) -> bool {
        self.pending_new_queue.is_none()
    }

    /// Whether the link owes no Named-queue command an answer.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn pending_named_queue_ops_is_empty_for_test(&self) -> bool {
        self.pending_named_queue_ops.is_empty()
    }

    /// Run one Player event through the core's own handler, as the Client tick
    /// runs the event it drains.
    #[cfg(any(test, feature = "test"))]
    pub fn apply_player_event_for_test(&mut self, event: mbv_ctrl::player::PlayerEvent) {
        let _ = self.handle_player_event(event);
    }

    /// Raise the blocking daemon-lost modal, as a lost Owner link does.
    #[cfg(any(test, feature = "test"))]
    pub fn raise_daemon_lost_modal_for_test(&mut self) {
        self.raise_daemon_lost_modal();
    }
}
