//! The GUI core's mark seams (`gui-change-driven-sections` rows 2.7 and 2.8):
//! the navigation clock, the playing slot, and an image fetch's loading state.
//! A core test moves each one without input, so the tick's deadline and
//! playhead compares see the change the way the production path produces it.
//! The seams sit in their own child module so `test_reads.rs` stays within its
//! line ceiling.

use std::time::{Duration, Instant};

use super::super::App;

impl App {
    /// Move the navigation clock back by `by`, so the navigation-idle gate
    /// opens as if the last navigation happened `by` ago. A core test pins the
    /// deadline this way, with no real wait.
    ///
    /// # Panics
    ///
    /// Panics when the platform clock cannot reach back by `by`, which a test's
    /// short delay never causes.
    #[cfg(any(test, feature = "test"))]
    pub fn backdate_navigation_for_test(&mut self, by: Duration) {
        self.last_nav_at = Instant::now()
            .checked_sub(by)
            .expect("the clock reaches back by the test's navigation delay");
    }

    /// Move the playing slot to `index` with no input, as the Owner's status
    /// does when playback advances (the playhead's own change, design D3 rule
    /// 4).
    #[cfg(any(test, feature = "test"))]
    pub fn set_playing_index_for_test(&mut self, index: usize) {
        self.player
            .update_status(|status| status.current_idx = index);
    }

    /// Whether `key`'s image fetch is in flight (the reservation a fetch takes
    /// before it starts): a core test observes that a neighbour warm started.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn card_image_loading_for_test(&self, key: &str) -> bool {
        self.images.is_loading(key)
    }
}
