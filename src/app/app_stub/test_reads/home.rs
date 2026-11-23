//! The TUI shell's Home and Feeds test seeds (`split-tui-gui-clients` design
//! D4b rule 5): the snapshot, subscription, and entry fixtures the shell's
//! Home, Feeds, and status-row tests assert a projection of.
//!
//! Each seam names the fixture it builds — a Home snapshot, a Feeds tab with
//! its subscriptions and entries, a Service state — never one field, so the
//! tests that drove those shapes keep their claims and reach no field.

use super::super::App;
use mbv_emby_model::EmbyItem;
use mbv_queue::FeedEntry;
use mbv_ui_model::tab_selection::TabSelection;

impl App {
    /// Hold `items` as Home's Continue Watching snapshot, as a launch load
    /// would.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_home_continue_items_for_test(&mut self, items: Vec<EmbyItem>) {
        self.home_content.continue_items = items;
    }

    /// Hold the Feeds tab the given `subscriptions` configure and the given
    /// `entries` it last fetched, as a startup load would.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_feeds_tab_for_test(
        &mut self,
        subscriptions: Vec<mbv_config::FeedSubscription>,
        entries: Vec<FeedEntry>,
    ) {
        self.feed_tab.subscriptions = subscriptions;
        self.feed_tab.all_entries = entries;
    }

    /// Make `tab` the active left-panel tab without the settling pass a user's
    /// tab change runs: the state a Client holds once it has settled there.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_active_tab_for_test(&mut self, tab: TabSelection) {
        self.tab = tab;
    }

    /// Hold `state` as the Audiobookshelf Service's state, as a setup attempt
    /// leaves it.    #[cfg(any(test, feature = "test"))]
    pub fn seed_audiobookshelf_service_state_for_test(
        &mut self,
        state: mbv_core::service_runtime::ServiceState,
    ) {
        self.audiobookshelf_runtime.state = state;
    }

    /// Hold a ready Emby runtime: a configured, unroutable client whose state
    /// is `Ready`, so the launch-time fetch paths arm with no live Service.
    #[cfg(any(test, feature = "test"))]
    pub fn adopt_ready_emby_runtime_for_test(&mut self) {
        self.emby_runtime = super::artists::ready_emby_runtime_for_test();
    }

    /// Hold `config` as this Client's configuration and `client` as its ready
    /// Emby runtime, as a launch that already holds an Emby setup would.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_configured_emby_client_for_test(
        &mut self,
        config: crate::app::config::Config,
        client: mbv_emby::EmbyClient,
    ) {
        self.config = std::sync::Arc::new(std::sync::Mutex::new(config));
        self.emby_runtime = crate::app::state::service_runtime::EmbyRuntime::ready(
            std::sync::Arc::new(std::sync::Mutex::new(client)),
        );
    }

    /// Drop every live toast, so a test observes only the toast its own
    /// action raised.
    #[cfg(any(test, feature = "test"))]
    pub fn clear_toasts_for_test(&mut self) {
        self.toasts.clear();
    }

    /// Hold the owner's reported playback output, as a drain would.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_playback_output_for_test(&mut self, output: Option<mbv_ctrl::OutputState>) {
        self.playback_output = output;
    }

    /// Hold `items` as the local queue view, cursor at `cursor`, as an owner
    /// snapshot would.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_local_queue_for_test(&mut self, items: Vec<EmbyItem>, cursor: usize) {
        let slots: Vec<mbv_ctrl::UnifiedQueueSlot> = items
            .into_iter()
            .enumerate()
            .map(|(index, item)| mbv_ctrl::UnifiedQueueSlot {
                slot_id: u64::try_from(index).unwrap_or(u64::MAX) + 1,
                item: mbv_queue::QueueItem::Emby(Box::new(item)),
            })
            .collect();
        let state = mbv_ctrl::UnifiedQueueStateData {
            queue: None,
            start_slot: None,
            status: mbv_ctrl::player::PlayerStatus::default(),
            active_slot: None,
            slots,
            revision: 1,
            source: mbv_queue::QueueSource::Unknown,
            lineage: mbv_queue::QueueLineage::default(),
            in_flight_transition: None,
            queued_latest_transition: None,
        };
        let mut view = crate::app::QueueView::from_snapshot(&state);
        view.set_cursor(cursor);
        self.local_view = view;
    }

    /// Hold the local player's status as playing `queue_len` slots with
    /// `current_idx` active, as a status drain would.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_playing_status_for_test(&mut self, current_idx: usize, queue_len: usize) {
        self.player.update_status(|status| {
            status.active = true;
            status.current_idx = current_idx;
            status.queue_len = queue_len;
        });
    }
}
