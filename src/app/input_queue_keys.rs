use super::{App, LEFT_WIDTH_DEFAULT, LEFT_WIDTH_STEP};
use std::time::{Duration, Instant};

/// How long a queue navigation gesture (Up/Down/click) holds the cursor
/// against background events that would snap it to the now-playing item.
const QUEUE_NAV_CURSOR_HOLD: Duration = Duration::from_millis(500);

impl App {
    /// Apply a QueueComponent-owned column resize intent.
    pub(super) fn resize_queue_column(&mut self, wider: bool) {
        if wider {
            self.queue_column_width += LEFT_WIDTH_STEP;
        } else {
            self.queue_column_width = self
                .queue_column_width
                .saturating_sub(LEFT_WIDTH_STEP)
                .max(LEFT_WIDTH_DEFAULT);
        }
        self.save_prefs();
    }

    /// Record that the user just navigated the queue, arming a short
    /// hold window during which background events must not snap the
    /// cursor to the now-playing item.
    pub(super) fn mark_queue_cursor_user_active(&mut self) {
        self.last_nav_at = Instant::now();
    }

    /// Whether a recent user navigation gesture should prevent a
    /// background event from overwriting `queue_cursor`.
    pub(super) fn queue_cursor_held_by_user(&self) -> bool {
        self.last_nav_at.elapsed() < QUEUE_NAV_CURSOR_HOLD
    }
}
