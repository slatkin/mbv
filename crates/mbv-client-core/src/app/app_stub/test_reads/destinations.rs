//! The GUI destinations' remaining fixtures (`split-tui-gui-clients` design
//! D4b rule 5): the Service runtimes a destination's loads gate on, the Home
//! `Recently added` store, the TV Upcoming snapshots, and the deferred
//! navigation the tab and series landings hold.
//!
//! Each seam names the shape a test seeds instead of writing one field, and
//! lives in its own child module so `home.rs` stays within its line ceiling.

use super::super::App;

impl App {
    /// Connect `client` as the Emby client, as a completed Service setup
    /// leaves the runtime.
    #[cfg(any(test, feature = "test"))]
    pub fn restore_emby_client_for_test(&mut self, client: mbv_emby::EmbyClient) {
        self.emby_runtime.client = Some(std::sync::Arc::new(std::sync::Mutex::new(client)));
    }

    /// Drop the connected Emby client, leaving the runtime as it stood: the
    /// state a Client holds while no Service client is connected.
    #[cfg(any(test, feature = "test"))]
    pub fn disconnect_emby_client_for_test(&mut self) {
        self.emby_runtime.client = None;
    }

    /// Take the connected Emby client aside, leaving the runtime with none, and
    /// answer it: a test puts it back with
    /// [`Self::restore_emby_client_for_test`].
    ///
    /// # Panics
    ///
    /// Panics when the Emby client lock was poisoned.
    #[cfg(any(test, feature = "test"))]
    pub fn withhold_emby_client_for_test(&mut self) -> Option<mbv_emby::EmbyClient> {
        self.emby_runtime.client.take().map(|client| {
            client
                .lock()
                .unwrap_or_else(|poisoned| {
                    panic!("the Emby client lock was poisoned: {poisoned:?}")
                })
                .clone()
        })
    }

    /// Hold the Audiobookshelf runtime in `starting`: the catalog is still
    /// landing, so a saved launch location waits for it.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_audiobookshelf_runtime_starting_for_test(&mut self, starting: bool) {
        self.audiobookshelf_runtime =
            crate::app::state::service_runtime::AudiobookshelfRuntime::new(starting);
    }

    /// Hold `rows` as the TV Upcoming snapshots of the library `library_id`.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_tv_upcoming_snapshot_for_test(
        &mut self,
        library_id: &str,
        rows: Vec<mbv_emby_model::UpcomingPremiere>,
    ) {
        self.tv_upcoming_snapshots
            .insert(library_id.to_string(), rows);
    }

    /// Hold `items` as the destination `source`'s Latest items, as its landed
    /// fetch leaves them.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_home_recently_added_for_test(
        &mut self,
        source: crate::app::state::playback::DestinationLatestSource,
        items: Vec<mbv_queue::QueueItem>,
    ) {
        self.home_recently_added.insert(source, items);
    }

    /// Hold `receivers` as the Cast receivers discovery last found, as the
    /// chooser's target list presents them.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_cast_receivers_for_test(
        &mut self,
        receivers: Vec<mbv_cast::discovery::CastReceiver>,
    ) {
        self.cast_receivers = receivers;
    }

    /// Add `subscription` to the Feeds tab, as the configured feeds leave it.
    #[cfg(any(test, feature = "test"))]
    pub fn push_feed_subscription_for_test(&mut self, subscription: mbv_config::FeedSubscription) {
        self.feed_tab.subscriptions.push(subscription);
    }

    /// Hold `tab` as the left-panel tab index a deferred navigation switch will
    /// land on, or `None` when no switch is armed.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_pending_navigate_tab_switch_for_test(&mut self, tab: Option<usize>) {
        self.pending_navigate_tab_switch = tab;
    }

    /// The left-panel tab index a deferred navigation switch will land on, or
    /// `None` when no switch is armed.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn pending_navigate_tab_switch_for_test(&self) -> Option<usize> {
        self.pending_navigate_tab_switch
    }

    /// Hold `landing` as the series landing a deferred navigation still owes.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_pending_series_landing_for_test(
        &mut self,
        landing: crate::app::state::events::PendingSeriesLanding,
    ) {
        self.pending_series_landing = Some(landing);
    }

    /// Whether no deferred series landing is armed.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn pending_series_landing_is_none_for_test(&self) -> bool {
        self.pending_series_landing.is_none()
    }
}
