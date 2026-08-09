use super::notify_actions::ToastSeverity;
use super::types_confirm::ConfirmAction;
use super::types_playback::PendingQueueAction;
use mbv_core::player::PlayerCommand;

use super::search_sidebar::SearchDrainOutcome;
use super::App;

impl App {
    /// Drain and act on notification-originated actions (skip-intro, next-up,
    /// clear-queue confirmation, notif-failure flag). Extracted from `run()`'s
    /// loop body; returns whether any action was received so the caller can
    /// fold that into its own `had_events` for render scheduling.
    pub(super) fn drain_notif_actions(&mut self) -> bool {
        let mut produced = false;
        while let Ok(action) = self.notif_action_rx.try_recv() {
            produced = true;
            match action.as_str() {
                "skip_intro:skip" => {
                    if let Some(end_ticks) = self.skip_intro_end_ticks.take() {
                        let secs = end_ticks as f64 / mbv_core::api::TICKS_PER_SECOND as f64;
                        self.player.send_command(PlayerCommand::SeekAbsolute(secs));
                        self.player.send_command(PlayerCommand::SkipIntroDismiss);
                        self.status.clear();
                    }
                }
                "next_up:play" => {
                    if let Some(item) = self.next_up_item.take() {
                        if let Some(idx) = self
                            .playback_queue()
                            .items
                            .iter()
                            .position(|i| i.id == item.id)
                        {
                            let label = item.playback_label();
                            self.player.send_command(PlayerCommand::JumpTo(idx));
                            self.playback_queue_mut().queue_cursor = idx;
                            self.flash(label, ToastSeverity::Success);
                        }
                    }
                    self.status.clear();
                }
                "next_up:skip" => {
                    self.next_up_item = None;
                    self.player.send_command(PlayerCommand::NextUpDismiss);
                    self.status.clear();
                }
                "clear:yes" => {
                    if matches!(
                        self.confirm_modal.as_ref().map(|m| &m.on_confirm),
                        Some(ConfirmAction::ClearQueue)
                    ) {
                        self.confirm_modal = None;
                        self.replace_queue_or_prompt(PendingQueueAction::ClearQueue);
                    }
                }
                "__notif_failed__" => {
                    self.notif_failed = true;
                }
                _ => {} // dismissed, "ignore", "cancel", or empty: leave TUI prompt untouched
            }
        }
        produced
    }

    /// Drain the search-results channel and apply any results to the
    /// search sidebar. Extracted from `run()`'s loop body; returns whether
    /// any results were received so the caller can fold that into `had_events`.
    pub(super) fn drain_search_results(&mut self) -> bool {
        let mut outcome = SearchDrainOutcome { received: 0 };
        self.drain_search_sidebar_results(&mut outcome);
        let produced = outcome.received > 0;
        // Errors are surfaced inline in the search sidebar (last_drain_error);
        // no redundant flash needed.
        produced
    }

    /// Flush a debounced search query if the deadline has passed. Called
    /// from the run loop every frame so queries fire after the user pauses
    /// typing for SEARCH_DEBOUNCE_MS.
    pub(super) fn maybe_flush_search_debounce(&mut self) -> bool {
        let Some(deadline) = self.search_debounce_deadline else {
            return false;
        };
        if std::time::Instant::now() < deadline {
            return false;
        }
        let Some(query) = self.search_debounce_pending.take() else {
            self.search_debounce_deadline = None;
            return false;
        };
        self.search_debounce_deadline = None;
        let client = self.client.lock().unwrap().clone();
        self.spawn_search_sidebar_query(client, query);
        true
    }

    /// Drain the sessions-poll channel, dispatching each event to
    /// `handle_session_event`. Extracted from `run()`'s loop body; returns
    /// whether any event was received so the caller can fold that into
    /// `had_events`.
    pub(super) fn drain_session_events(&mut self) -> bool {
        let mut produced = false;
        while let Ok(ev) = self.sessions_rx.try_recv() {
            produced = true;
            self.handle_session_event(ev);
        }
        produced
    }
}
