//! The core facts only the GUI Client reads.
//!
//! A Client reaches no `App` field (change `split-tui-gui-clients`, design
//! D4/D4b rule 1). The facts here are core state that only the GUI Client's
//! production code asks for: the shown queue entry, the Named-queue list, the
//! queue-shuffle and Audiobookshelf-admission capabilities, the queue
//! filter's held bit, the peer queue's length, and the configuration's
//! autoload rule. A fact both Clients read lives in `shell_reads` instead; the
//! state itself stays in the core, and a write to it goes through
//! `gui_writes` or the shared write module.

use crate::app::App;
use crate::app::state::playback::ShownEntry;
use mbv_ctrl::NamedQueueEntry;
use mbv_queue::NamedQueueId;

impl App {
    /// The entry the Queue panel shows: a local Named queue or the connected
    /// peer entry, `None` while this Client shows neither.
    #[must_use]
    pub fn shown_entry(&self) -> Option<ShownEntry> {
        self.shown_entry
    }

    /// The local Owner's Named queues in queue order, as the last published
    /// list holds them.
    #[must_use]
    pub fn named_queue_entries(&self) -> &[NamedQueueEntry] {
        self.named_queues.entries()
    }

    /// How many Named queues the local Owner's last published list holds.
    #[must_use]
    pub fn named_queue_count(&self) -> usize {
        self.named_queues.entries().len()
    }

    /// The current Named queue: the one that holds the playhead.
    #[must_use]
    pub fn current_named_queue(&self) -> Option<NamedQueueId> {
        self.named_queues.current()
    }

    /// Whether the local Owner still holds `queue`.
    #[must_use]
    pub fn named_queue_holds(&self, queue: NamedQueueId) -> bool {
        self.named_queues.holds(queue)
    }

    /// The connected peer Player owner's queue length, or `0` while this
    /// Client holds no peer view.
    #[must_use]
    pub fn remote_queue_len(&self) -> usize {
        self.remote_view
            .as_ref()
            .map_or(0, crate::app::QueueView::total_queue_len)
    }

    /// Whether the Player owner the queue plays accepts a whole-queue
    /// reorder.
    #[must_use]
    pub fn supports_queue_shuffle(&self) -> bool {
        self.player.supports_queue_shuffle()
    }

    /// Whether the Player owner the queue plays accepts Audiobookshelf items.
    #[must_use]
    pub fn can_admit_audiobookshelf(&self) -> bool {
        self.player.can_admit_audiobookshelf()
    }

    /// Whether the queue filter's text entry holds the keyboard.
    #[must_use]
    pub fn queue_filter_active(&self) -> bool {
        self.queue_filter_active
    }

    /// Whether a played album expands to its full tracklist.
    #[must_use]
    pub fn autoload_enabled(&self) -> bool {
        self.with_config(|config| config.autoload)
    }
}
