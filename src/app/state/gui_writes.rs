//! The core state only the GUI Client changes.
//!
//! A Client reaches no `App` field (change `split-tui-gui-clients`, design
//! D4/D4b rules 1 and 2). Each method here names the intent the GUI Client
//! runs and keeps the field, its guard, and its ordering in the core: the
//! queue filter's text-entry rules, the queue-rename hand-off it presents,
//! the overlay hand-off it drops on a reset, the row-art cache touch, and the
//! Audiobookshelf browse records its opens settle.

use crate::app::App;
use mbv_queue::NamedQueueId;

impl App {
    /// Close the queue filter's text entry and clear its text: Esc, or a
    /// Backspace that emptied it.
    pub fn close_queue_filter(&mut self) {
        self.queue_filter_active = false;
        self.queue_filter.clear();
    }

    /// Close the queue filter's text entry and keep its text: Enter, so the
    /// filter keeps narrowing the rows while the keyboard returns to the
    /// panel.
    pub fn finish_queue_filter_edit(&mut self) {
        self.queue_filter_active = false;
    }

    /// Append one typed character to the queue filter's text.
    pub fn push_queue_filter_char(&mut self, character: char) {
        self.queue_filter.push(character);
    }

    /// Drop the queue filter's last character. A Backspace on empty text
    /// closes the entry and clears it.
    pub fn pop_queue_filter_char(&mut self) {
        if self.queue_filter.pop().is_none() {
            self.close_queue_filter();
        }
    }

    /// Take the queue a new-queue request created and the rename draft its
    /// name opens with, if the core holds one for this Client.
    pub fn take_pending_queue_rename(&mut self) -> Option<(NamedQueueId, String)> {
        self.pending_queue_rename.take()
    }

    /// Drop the overlay request the core handed to its Client: a GUI reset
    /// discards the pending shell request with the open overlay.
    pub fn clear_pending_overlay(&mut self) {
        self.pending_overlay = None;
    }

    /// Mark the row-art cache entry stored under `key` as read, so the row
    /// cache evicts rows the Client still paints last.
    pub fn touch_row_image(&mut self, key: &str) {
        self.images.touch_row_image(key);
    }

    /// Clear the failure record a cancelled or retried podcast catalog load
    /// left on the browse state at `index`.
    pub fn clear_audiobookshelf_browse_error(&mut self, index: usize) {
        if let Some(state) = self.audiobookshelf_browse.get_mut(index) {
            state.error = None;
        }
    }

    /// Clear the failure record a cancelled or retried book catalog load left
    /// on the browse state at `index`.
    pub fn clear_audiobookshelf_book_error(&mut self, index: usize) {
        if let Some(state) = self.audiobookshelf_book_browse.get_mut(index) {
            state.error = None;
        }
    }

    /// Hold `show` as the show pill the podcast library's bounded fan-out
    /// reads, when it changes.
    pub fn set_audiobookshelf_show_pill(&mut self, index: usize, show: Option<String>) {
        if let Some(state) = self.audiobookshelf_browse.get_mut(index)
            && state.committed_show_pill != show
        {
            state.committed_show_pill = show;
        }
    }
}
