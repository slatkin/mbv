//! The action seams a Client's test drives where no Client-facing entry
//! point reaches the shell's own path (design D4b rule 5): each delegates to
//! the core method the shell, the Client core tick, or a dispatch arm calls.
//! A seam names the act it drives and the fixture it returns, so no core
//! method becomes `pub` only for a test.

use crate::app::App;

impl App {
    /// Route the local queue to `library_name`'s device, as the Library
    /// Routes picker does: the same suspend-local, connect-remote step.
    pub fn switch_to_library_route_for_test(
        &mut self,
        library_name: &str,
        remote: mbv_remote_player::RemotePlayer,
        remote_rx: std::sync::mpsc::Receiver<mbv_ctrl::player::PlayerEvent>,
        endpoint: &mbv_remote_player::DaemonEndpoint,
    ) {
        self.switch_to_library_route(library_name, remote, remote_rx, endpoint);
    }

    /// Whether the active player link is the local Owner.
    #[must_use]
    pub fn is_local_daemon_for_test(&self) -> bool {
        self.is_local_daemon()
    }

    /// The queue-slot count the suspended local player still holds after the
    /// local fallthrough prepares it, or `0` when no local session prepares.
    #[must_use]
    pub fn prepare_local_player_slots_for_test(&mut self) -> usize {
        self.prepare_local_player()
            .ok()
            .flatten()
            .and_then(|session| {
                session
                    .player
                    .remote()
                    .unified_queue_state()
                    .map(|state| state.slots.len())
            })
            .unwrap_or(0)
    }

    /// Settle the pending queue transfer, as the Client core tick's
    /// snapshot-adoption phase does.
    pub fn settle_pending_queue_transfer_for_test(&mut self) -> bool {
        self.settle_pending_queue_transfer()
    }

    /// Rebuild the panel-target list, as a cast discovery or a route change
    /// does.
    pub fn rebuild_panel_targets_for_test(&mut self) {
        self.rebuild_panel_targets();
    }

    /// The Series row under `cursor` in the library at `lib_idx`, as the TV
    /// workspace reads it.
    #[must_use]
    pub fn selected_series_item_for_test(
        &self,
        lib_idx: usize,
        cursor: usize,
    ) -> Option<mbv_emby_model::EmbyItem> {
        self.selected_series_item(lib_idx, cursor)
    }

    /// Land one series-detail fetch, as the worker's completion does.
    pub fn handle_series_detail_fetched_for_test(
        &mut self,
        series_id: &str,
        detail: mbv_ui_model::browse::SeriesDetail,
    ) {
        self.handle_series_detail_fetched(series_id, detail);
    }

    /// The lineage the queue origin carries, from the owner's reported one:
    /// the fence a mutation is made against.
    #[must_use]
    pub fn queue_origin_lineage_for_test(&self) -> Option<mbv_queue::QueueLineage> {
        self.queue_origin().map(|origin| origin.lineage)
    }
}
