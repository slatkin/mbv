use crate::app::dispatch::notify::ToastSeverity;
use crate::app::{App, DaemonLostModal, QUIT_REQUESTED};
use mbv_ctrl::player::{PlayerCommand, PlayerEvent};
use std::sync::atomic::Ordering;

mod progress;

/// What `App::handle_player_event` asks its caller to do next.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::app) enum PlayerEventFlow {
    /// The caller's event loop continues normally.
    Proceed,
    /// The caller's event loop should `continue` (skip render for this tick).
    RestartLoop,
}

impl App {
    /// Mirror mpv's actual volume into `ui_volume` and persist it, so volume
    /// changes made inside the mpv window (not just via mbv's keys) are kept and
    /// restored on the next launch. Skipped while the owner's Emby Session
    /// output holds the volume (D10 puts it in `PlayerStatus`) and while
    /// temporarily muted (so a mute doesn't clobber the saved level with 0).
    pub(in crate::app) fn sync_volume_from_player(&mut self) {
        if self.output_is_emby_session() {
            return;
        }
        if self.pre_mute_volume.is_some() {
            return;
        }
        let player_vol = {
            let s = self.shown_playback_status();
            s.active.then(|| {
                u8::try_from(s.volume.clamp(0, 200)).expect("clamped player volume fits in u8")
            })
        };
        if let Some(v) = player_vol
            && v != self.ui_volume
        {
            self.ui_volume = v;
            self.save_prefs();
        }
    }

    /// Handle a `PlayerEvent` received from the player thread.
    /// Returns [`PlayerEventFlow::RestartLoop`] if the caller's event loop
    /// should `continue` (skip render for this tick).
    ///
    /// The Owner's queue state is adopted here, and the events that report what
    /// the Player owner or the local run did go to [`Self::handle_playback_event`]:
    /// a queue list, a resting queue's snapshot, and a queue snapshot's late
    /// answer change no playhead.
    pub(in crate::app) fn handle_player_event(&mut self, ev: PlayerEvent) -> PlayerEventFlow {
        match ev {
            // Named-queue traffic is Owner state the Client core adopts
            // (gui-named-queues design D7), never a playback observation.
            PlayerEvent::UnifiedQueueUpdated(unified) => {
                let link = self.active_queue_link();
                flow_after(self.handle_unified_queue_updated(link, &unified))
            }
            PlayerEvent::QueueOpResult { op, outcome } => self.handle_queue_op_result(op, outcome),
            ev => self.handle_playback_event(ev),
        }
    }

    /// Handle a late or unmatched `PlayerEvent::QueueOpResult` (row 5.1): the
    /// pump adopts the matching answer inline, so anything arriving here missed
    /// its wait and is adopted like any other owner snapshot. A late
    /// `Rejected` reports nothing more unless it answers a Named-queue command,
    /// whose request ends only here (design D8). The answer to the new-queue
    /// control's own request is not a queue snapshot: it ends that request and
    /// opens the queue it names.
    fn handle_queue_op_result(
        &mut self,
        op: mbv_ctrl::QueueOpId,
        outcome: mbv_ctrl::QueueOpOutcome,
    ) -> PlayerEventFlow {
        match outcome {
            mbv_ctrl::QueueOpOutcome::Applied(snapshot) => {
                if self.take_new_queue_answer(op, Some(&snapshot)) {
                    return PlayerEventFlow::Proceed;
                }
                flow_after(self.handle_unified_queue_updated(self.active_queue_link(), &snapshot))
            }
            mbv_ctrl::QueueOpOutcome::Rejected(reason) => {
                // A Named-queue command has no pump to report its refusal, so
                // this drain is where its request ends and the user learns why
                // (design D8). An edit's own rejection already flashed from the
                // pump that waited for it, so an op that ends no Named-queue
                // command stays silent.
                self.report_named_queue_rejection(op, reason);
                PlayerEventFlow::Proceed
            }
        }
    }

    /// The playback half of the player-event dispatch: every event that
    /// reports what the Player owner or the local run did.
    fn handle_playback_event(&mut self, ev: PlayerEvent) -> PlayerEventFlow {
        match ev {
            PlayerEvent::Stopped { .. } => flow_after(self.handle_stopped_event(ev)),
            PlayerEvent::TrackCompleted { .. } => {
                self.handle_track_completed_event(&ev);
                PlayerEventFlow::Proceed
            }
            PlayerEvent::TrackChanged { .. } => {
                self.handle_track_changed_event(&ev);
                PlayerEventFlow::Proceed
            }
            PlayerEvent::PausedChanged(paused) => {
                self.handle_paused_changed(paused);
                PlayerEventFlow::Proceed
            }
            PlayerEvent::OutputStarted => {
                self.handle_output_started();
                PlayerEventFlow::Proceed
            }
            PlayerEvent::NextUpThreshold { .. } | PlayerEvent::IntroEnded => {
                // No action: next-up thresholds are presentation-only; intro completion needs no handling.
                PlayerEventFlow::Proceed
            }
            PlayerEvent::NextUpPlay => {
                self.handle_next_up_play();
                PlayerEventFlow::Proceed
            }
            PlayerEvent::QueueNextUp { next_idx } => {
                self.handle_queue_next_up(next_idx);
                PlayerEventFlow::Proceed
            }
            PlayerEvent::IntroStarted { intro_end_ticks } => {
                self.handle_intro_started(intro_end_ticks);
                PlayerEventFlow::Proceed
            }
            PlayerEvent::SkipIntroPlay => {
                self.status.clear();
                PlayerEventFlow::Proceed
            }
            PlayerEvent::MpvQuit => {
                self.next_up_item = None;
                self.status.clear();
                self.refresh_after_stop();
                PlayerEventFlow::Proceed
            }
            PlayerEvent::CommandRejected(reason) => {
                self.flash(reason, ToastSeverity::Error);
                PlayerEventFlow::Proceed
            }
            PlayerEvent::PlaybackIntent(event) => {
                self.flash(
                    playback_intent_message(&event.outcome).to_string(),
                    ToastSeverity::Neutral,
                );
                PlayerEventFlow::Proceed
            }
            PlayerEvent::PipePlaybackStatus(status) => {
                self.flash(pipe_playback_message(&status), ToastSeverity::Neutral);
                PlayerEventFlow::Proceed
            }
            PlayerEvent::RemoteDisconnected(reason) => {
                flow_after(self.handle_remote_disconnected(&reason))
            }
            PlayerEvent::EmbyAuthorityTaken(reason) => {
                self.handle_emby_authority_taken(reason);
                PlayerEventFlow::Proceed
            }
            PlayerEvent::QueueDesynced(reason) => {
                self.handle_queue_desynced(reason);
                PlayerEventFlow::Proceed
            }
            PlayerEvent::DaemonShutdownAnnounced => {
                self.handle_daemon_shutdown_announced(false);
                PlayerEventFlow::Proceed
            }
            PlayerEvent::AudiobookshelfProgress(ev) => {
                self.handle_audiobookshelf_progress(&ev);
                PlayerEventFlow::Proceed
            }
            PlayerEvent::AudiobookshelfBookProgress(ev) => {
                self.handle_audiobookshelf_book_progress(&ev);
                PlayerEventFlow::Proceed
            }
            PlayerEvent::PlaybackOutput(state) => self.apply_playback_output(state),
            PlayerEvent::NamedQueue(event) => {
                self.adopt_named_queue_event(event);
                PlayerEventFlow::Proceed
            }
            // Both are adopted by `handle_player_event`, which takes them
            // before this half runs: no playback observation comes of either.
            PlayerEvent::UnifiedQueueUpdated(_) | PlayerEvent::QueueOpResult { .. } => {
                PlayerEventFlow::Proceed
            }
            PlayerEvent::SwapPrepare | PlayerEvent::SwapQuit => {
                // The shell drain intercepts both pin-swap events before they
                // reach App dispatch (tray-pin-swap 4.4): the launch snapshot
                // is a shell query and the exit kind is shell-owned. This arm
                // only keeps the match exhaustive.
                PlayerEventFlow::Proceed
            }
        }
    }

    /// Store the Owner's reported playback output (cast-owner-output design
    /// D13): the last report is the App's mirror of the active owner's
    /// output; the reset paths own clearing it.
    fn apply_playback_output(&mut self, state: mbv_ctrl::OutputState) -> PlayerEventFlow {
        self.playback_output = Some(state);
        PlayerEventFlow::Proceed
    }

    fn handle_emby_authority_taken(&mut self, reason: String) {
        // Authority notification leaves the connection open; do not restore local mode.
        self.flash(reason, ToastSeverity::Warning);
    }

    fn handle_queue_desynced(&mut self, reason: String) {
        self.flash(reason, ToastSeverity::Neutral);
    }

    fn handle_paused_changed(&mut self, paused: bool) {
        // Persist Feed position on pause (one write per pause event).
        let (active, current_idx) = {
            let status = self.player.status_snapshot();
            (status.active, status.current_idx)
        };
        if paused
            && active
            && let Some(slot_id) = self.playback_queue().slot_id_at(current_idx)
        {
            self.persist_feed_slot_position(slot_id);
        }
    }

    fn handle_output_started(&mut self) {
        // If a seek was pending for a Feed slot, persist the resulting position now.
        if let Some(slot_id) = self.feed_seek_pending_slot.take() {
            self.persist_feed_slot_position(slot_id);
        }
    }

    fn handle_intro_started(&mut self, intro_end_ticks: i64) {
        // mbvd never auto-seeks on this event itself — it always
        // reports the boundary neutrally, regardless of daemon-host
        // config, so this client's own `always_skip_intro` is the
        // only thing that decides whether to skip.
        if self.config.lock().unwrap().always_skip_intro {
            let secs = mbv_emby_model::ticks_to_seconds(intro_end_ticks);
            let _ = self.player.send_command(PlayerCommand::SeekAbsolute(secs));
            let _ = self.player.send_command(PlayerCommand::SkipIntroDismiss);
        }
    }

    pub(in crate::app) fn handle_daemon_shutdown_announced(&mut self, from_home_link: bool) {
        // The owner connection is gone; its playback output is no longer the
        // active one (cast-owner-output design D13 reset rule).
        self.playback_output = None;
        self.drop_staged_queue_transfer_on_teardown();
        if from_home_link || self.is_local_daemon() {
            self.suspended_local = None;
            self.pending_exit_message =
                Some("mbv: the Owner process was stopped — exiting.".to_string());
            QUIT_REQUESTED.store(true, Ordering::Relaxed);
        } else {
            self.restore_local_mode("Daemon disconnected — returned to local mode");
            self.refresh_after_stop();
        }
    }

    /// Handle a `PlayerEvent::Stopped` (extracted from `handle_player_event`).
    /// Returns true if the caller's event loop should `continue`.
    fn handle_stopped_event(&mut self, ev: PlayerEvent) -> bool {
        let PlayerEvent::Stopped {
            slot_id,
            position_ticks,
            played,
            consume,
            error,
            ..
        } = ev
        else {
            return false;
        };
        tracing::info!(name: "player.stopped.received", target: "player", { slot = ?slot_id, position_seconds = position_ticks / mbv_emby_model::TICKS_PER_SECOND, played, error = %error.as_deref().unwrap_or("none") }, "player stopped");
        if self.player.is_remote_disconnected() {
            return self.handle_stopped_remote_disconnected();
        }
        let deleted_slot = self.pending_delete_slot.take();
        let deleted_selection = self.pending_delete_selection.take();
        let is_delete =
            deleted_slot.is_some() || deleted_selection.as_ref().is_some_and(|s| !s.is_empty());
        match slot_id {
            Some(slot_id) => {
                if !is_delete {
                    self.apply_stopped_slot_progress(slot_id, position_ticks, played);
                }
            }
            None => {
                tracing::warn!(name: "player.stopped.slot_missing", target: "player", "stopped event has no live slot; skipping progress update");
            }
        }
        self.next_up_item = None;
        self.status.clear();
        let link = self.active_queue_link();
        self.finish_stopped_consumption(link, deleted_slot, deleted_selection, slot_id, consume);
        self.refresh_after_stop();
        false
    }

    /// Complete a confirmed removal from the suspended home link's `Stopped`
    /// (design D7 of `gui-named-queues`). The Client stopped the local Owner's
    /// playback because a confirmed removal named its now-playing row, so the
    /// local Owner's `Stopped` is where the removal reaches it. With no pending
    /// removal the event changes nothing: the local run's presentation belongs
    /// to the shown owner, not to this drain.
    pub(in crate::app) fn finish_home_stop(
        &mut self,
        slot_id: Option<mbv_queue::QueueSlotId>,
        consume: bool,
    ) {
        if self.pending_delete_slot.is_none() && self.pending_delete_selection.is_none() {
            return;
        }
        let deleted_slot = self.pending_delete_slot.take();
        let deleted_selection = self.pending_delete_selection.take();
        self.finish_stopped_consumption(
            crate::app::QueueLink::Local,
            deleted_slot,
            deleted_selection,
            slot_id,
            consume,
        );
    }

    /// The remote-disconnect prefix of `handle_stopped_event` (task 7.2/7.4
    /// paths): modal-or-reattach-or-fallback, always ending the tick.
    fn handle_stopped_remote_disconnected(&mut self) -> bool {
        self.next_up_item = None;
        self.drop_staged_queue_transfer_on_teardown();
        // An announced shutdown never reaches here: the reader
        // thread sends PlayerEvent::DaemonShutdownAnnounced
        // instead of a synthetic Stopped for that case (see the
        // arm below). Assert the invariant rather than silently
        // trusting it -- getting it backwards is exactly the
        // spurious-modal-vs-silent-exit boundary task 7.4 tests.
        debug_assert!(
            !self.player.is_shutdown_announced(),
            "an announced daemon shutdown must never surface as PlayerEvent::Stopped"
        );
        // A client of a local daemon can offer to restart it; a
        // client of a genuinely remote daemon cannot, and keeps
        // today's silent-fallback behavior (task 7.2). Before
        // any fallback, try an auto-reconnect reattach to the
        // same remote daemon when the option is enabled.
        if self.is_local_daemon() {
            self.raise_daemon_lost_modal();
        } else if self.try_reattach_remote_daemon() {
            return true;
        } else {
            self.restore_local_mode("Daemon disconnected — returned to local mode");
        }
        self.refresh_after_stop();
        true
    }

    /// Progress/bookkeeping half of a normal `Stopped`: derive and persist Feed lifecycle state.
    fn apply_stopped_slot_progress(
        &mut self,
        slot_id: mbv_queue::QueueSlotId,
        position_ticks: i64,
        played: bool,
    ) {
        let Some(slot) = self.playback_queue().slot(slot_id) else {
            return;
        };
        let observation = mbv_queue::ProgressObservation::Stopped {
            position_ticks,
            played,
        };
        let position = observation.position_to_record(&slot.item);
        let feed_runtime =
            matches!(slot.item, mbv_queue::QueueItem::Feed(_)).then(|| slot.item.runtime_ticks());
        // Persist Feed lifecycle state from the report; the owner publishes the queue update.
        if let Some(runtime) = feed_runtime {
            let feed_completed = played || (runtime > 0 && position >= runtime * 95 / 100);
            self.persist_feed_slot_lifecycle(slot_id, position, feed_completed);
        }
        // The Player owner applies the report to its queue and publishes it.
    }

    /// Delete-vs-consume tail of a normal `Stopped`: send a confirmed removal
    /// to `link`'s Player owner, or run the consume reaction. `link` is the
    /// link the stop came from — the queue the run played — never the queue
    /// the panel shows, so a stop on one link never edits the other's queue.
    fn finish_stopped_consumption(
        &mut self,
        link: crate::app::QueueLink,
        deleted_slot: Option<mbv_queue::QueueSlotId>,
        deleted_selection: Option<Vec<mbv_queue::QueueSlotId>>,
        slot_id: Option<mbv_queue::QueueSlotId>,
        consume: bool,
    ) {
        if let Some(selection) = deleted_selection.filter(|s| !s.is_empty()) {
            // A confirmed multi-delete that included now-playing removes
            // every named slot in one owner edit after the stop.
            let raw: Vec<u64> = selection
                .iter()
                .map(|slot_id| mbv_ctrl::slot_id_to_u64(*slot_id))
                .collect();
            self.queue_op(
                link,
                mbv_remote_player::QueueOp::RemoveSlots { slot_ids: raw },
                None,
            );
        } else if deleted_slot.is_some() {
            // Confirmation deferred removal until playback stopped; the Client
            // now asks the owner to remove the stable slot identity.
            if let Some(deleted_slot) = deleted_slot {
                self.queue_op(
                    link,
                    mbv_remote_player::QueueOp::RemoveSlot {
                        slot_id: mbv_ctrl::slot_id_to_u64(deleted_slot),
                    },
                    None,
                );
            }
        } else {
            let (should_consume, is_audio) = match slot_id {
                Some(slot_id) => self.should_consume_slot(slot_id, consume),
                None => (false, false),
            };
            if should_consume {
                if is_audio {
                    self.on_audio_consumed();
                } else {
                    self.on_video_consumed();
                }
            }
        }
    }

    /// Handle a `PlayerEvent::TrackCompleted` (extracted from
    /// `handle_player_event`).
    fn handle_track_completed_event(&mut self, ev: &PlayerEvent) {
        let PlayerEvent::TrackCompleted {
            slot_id,
            position_ticks,
            played,
            consume,
            ..
        } = *ev
        else {
            return;
        };
        let Some(slot) = self.playback_queue().slot(slot_id) else {
            tracing::warn!(name: "consume.track_completed.slot_missing", target: "consume", slot = ?slot_id, "completed track has no live slot; dropping");
            return;
        };
        let observation = mbv_queue::ProgressObservation::Completed {
            position_ticks,
            played,
        };
        let position = observation.position_to_record(&slot.item);
        let feed_runtime =
            matches!(slot.item, mbv_queue::QueueItem::Feed(_)).then(|| slot.item.runtime_ticks());
        // Persist Feed lifecycle state from the report. TrackCompleted with
        // `played` means EOF; for Feed entries,
        // only known-runtime EOF marks played (unknown runtime keeps
        // played=false per spec).
        if let Some(runtime) = feed_runtime {
            let feed_completed = played && runtime > 0;
            self.persist_feed_slot_lifecycle(slot_id, position, feed_completed);
        }
        let (should_consume, is_audio) = self.should_consume_slot(slot_id, consume);
        if should_consume {
            if is_audio {
                self.on_audio_consumed();
            } else {
                self.on_video_consumed();
            }
        }
    }

    /// Handle a `PlayerEvent::TrackChanged` (extracted from
    /// `handle_player_event`).
    fn handle_track_changed_event(&mut self, ev: &PlayerEvent) {
        let PlayerEvent::TrackChanged {
            slot_id: target_slot_id,
            ..
        } = *ev
        else {
            return;
        };
        self.visualizer_failed = false;
        self.next_up_item = None;
        if self.status.starts_with("Next up:") {
            self.status.clear();
        }

        // Activate by owner-assigned identity. Slot identity is stable
        // across the pending-removal consume above, so resolving it to
        // a display position afterward is order-independent.
        let adjusted = if let Some(index) = self.playback_queue().slot_index(target_slot_id) {
            index
        } else {
            tracing::warn!(name: "player.track_changed.slot_missing", target: "player", slot = ?target_slot_id, "track change has no live slot; skipping activation");
            let status = self.player.status_snapshot();
            if status.active { status.current_idx } else { 0 }
        };
        if !self.queue_cursor_held_by_user() {
            self.playback_queue_mut().set_cursor(adjusted);
        }
    }

    /// Handle a `PlayerEvent::NextUpPlay` (extracted from
    /// `handle_player_event`).
    fn handle_next_up_play(&mut self) {
        tracing::warn!(name: "app.next_up.play_triggered", target: "app", "next-up play triggered");
        if let Some(item) = self.next_up_item.take() {
            let label = item.playback_label();
            if let Some(idx) =
                self.playback_queue().slots().iter().position(
                    |s| matches!(&s.item, mbv_queue::QueueItem::Emby(e) if e.id == item.id),
                )
            {
                let slot_id = self.playback_queue().slots()[idx].slot_id;
                let accepted = self.request_slot_jump(slot_id);
                if accepted {
                    self.flash(label, ToastSeverity::Neutral);
                } else {
                    self.flash(
                        "Playback owner rejected the queue selection".into(),
                        ToastSeverity::Error,
                    );
                }
            } else {
                tracing::warn!(name: "app.next_up.item_not_in_queue", target: "app", "next-up item not in queue; cannot jump");
            }
        } else {
            tracing::warn!(name: "app.next_up.item_missing", target: "app", "next-up play event has no item");
        }
    }

    /// Adopt a snapshot from the suspended home link into the Local queue
    /// without changing the viewed Player's status.
    pub(in crate::app) fn adopt_home_snapshot(
        &mut self,
        unified: &mbv_ctrl::UnifiedQueueStateData,
    ) {
        // A snapshot that names a resting Named queue is that queue's own
        // state here too (gui-named-queues design D3, D7): the answer to an
        // edit of a resting queue arrives on the home link while a peer holds
        // the active one, and it fills the shown resting view rather than the
        // current queue's.
        if let Some(queue) = unified.queue {
            self.adopt_named_queue_snapshot(queue, unified);
            return;
        }
        // The home link's status is its own proxy's, so the player panel, the
        // transport gates, and the track chips read the local Owner's live
        // playback while a local entry is shown (design D7). The active peer's
        // mirror never takes it.
        self.local_queue_player().set_status(unified.status.clone());
        self.local_view.adopt(
            unified,
            crate::app::state::queue_view::AdoptCause::Background {
                held: self.queue_cursor_held_by_user(),
            },
        );
        // Same source adoption/reconciliation as a live owner snapshot
        // (design D6): the home link's snapshots must reconcile a pending
        // playlist-save source update too.
        self.adopt_owner_source(unified);
    }

    /// Handle a `PlayerEvent::UnifiedQueueUpdated` from `link` (extracted from
    /// `handle_player_event`). `link` is the link the event arrived on — the
    /// active connection, or the link an answer pump read — so the adopted
    /// status, the queue view, and the source all stay that link's facts while
    /// another link is shown (design D7). Returns true when the local
    /// generation fence ends the tick early.
    pub(in crate::app) fn handle_unified_queue_updated(
        &mut self,
        link: crate::app::QueueLink,
        unified: &mbv_ctrl::UnifiedQueueStateData,
    ) -> bool {
        // A snapshot that names a resting Named queue is that queue's own
        // state, never the current queue's (gui-named-queues design D3, D7):
        // it fills the shown resting view and leaves the current queue's view
        // and the playback status alone, because a resting queue holds no
        // playhead and its status is a default.
        if let Some(queue) = unified.queue {
            self.adopt_named_queue_snapshot(queue, unified);
            return false;
        }
        // Adopt the owner snapshot as one value. Do not combine its
        // queue with a separately delivered PlayerStatus coordinate.
        self.queue_owner_player(link)
            .set_status(unified.status.clone());
        let held = self.queue_cursor_held_by_user();
        self.queue_for_link_mut(link).adopt(
            unified,
            crate::app::state::queue_view::AdoptCause::Background { held },
        );
        if link == crate::app::QueueLink::Local {
            self.adopt_owner_source(unified);
        }
        false
    }

    /// Raises the blocking daemon-lost modal (task 7.1), replacing whatever
    /// other blocking overlay was showing -- only one is ever active.
    pub(in crate::app) fn raise_daemon_lost_modal(&mut self) {
        // A new-queue request the lost link owed an answer will never receive
        // it, so the control works again once an Owner is back. The same holds
        // for every Named-queue command the link owed an answer.
        self.pending_new_queue = None;
        self.pending_named_queue_ops.clear();
        // Closing the context menu is re-homed: the DaemonLost `OverlayRequest`
        // arm calls `dismiss_blocking_modals`, which now also unmounts the
        // ContextMenu component (task 5.3c). `pending_overlay` is a single slot,
        // so it cannot both dismiss the menu and raise DaemonLost here.
        let last_playing_title = {
            let idx = self.player.status_snapshot().current_idx;
            self.playback_queue()
                .item_at(idx)
                .map(|item| item.title().to_string())
        };
        let modal = DaemonLostModal {
            last_playing_title,
            daemon_log_path: mbv_config::state_dir()
                .join("local-daemon.log")
                .display()
                .to_string(),
            restart_error: None,
        };
        // The GUI Client presents the request from its own drain (it mounts no
        // TUI modal component).
        self.pending_overlay = Some(mbv_ui_model::overlay::OverlayRequest::DaemonLost(modal));
    }

    /// Look-ahead hint for the Next-Up card (extracted from
    /// `handle_player_event`).
    fn handle_queue_next_up(&mut self, next_idx: usize) {
        if let Some(item) = self.playback_queue().emby_item_at(next_idx).cloned() {
            self.next_up_item = Some(item);
        }
    }

    /// The disconnect half of the daemon-loss paths (extracted from
    /// `handle_player_event`): modal-or-reattach-or-fallback, always ending
    /// the tick.
    fn handle_remote_disconnected(&mut self, reason: &str) -> bool {
        // The owner connection is gone; its playback output is no longer the
        // active one (cast-owner-output design D13 reset rule).
        self.playback_output = None;
        self.next_up_item = None;
        self.drop_staged_queue_transfer_on_teardown();
        if self.is_local_daemon() {
            self.suspended_local = None;
            self.raise_daemon_lost_modal();
            self.refresh_after_stop();
            return true;
        }
        if self.try_reattach_remote_daemon() {
            return true;
        }
        self.restore_local_mode(reason);
        self.refresh_after_stop();
        true
    }

    /// Persist a Feed slot's current player position, when the slot still
    /// exists and carries a feed identity. Shared by the pause and
    /// seek-completion paths (extracted from `handle_player_event`).
    fn persist_feed_slot_position(&mut self, slot_id: mbv_queue::QueueSlotId) {
        if let Some(slot) = self.playback_queue().slot(slot_id)
            && let mbv_queue::QueueItem::Feed(ref entry) = slot.item
            && entry.feed_id.is_some()
        {
            let pos_ticks = self.player.status_snapshot().position_ticks;
            self.persist_feed_slot_lifecycle(slot_id, pos_ticks, false);
        }
    }
}

fn flow_after(restart_loop: bool) -> PlayerEventFlow {
    if restart_loop {
        PlayerEventFlow::RestartLoop
    } else {
        PlayerEventFlow::Proceed
    }
}

/// The one message for a correlated direct-daemon playback-intent outcome
/// (extracted from `handle_player_event`).
fn playback_intent_message(outcome: &mbv_ctrl::PlaybackIntentOutcome) -> &'static str {
    use mbv_ctrl::{PlaybackIntentOutcome, PlaybackIntentRejection};
    match outcome {
        PlaybackIntentOutcome::Accepted => "Playback request accepted",
        PlaybackIntentOutcome::Applied => "Playback request applied",
        PlaybackIntentOutcome::Coalesced { .. } => "Playback request already pending",
        PlaybackIntentOutcome::Superseded => "Playback request superseded",
        PlaybackIntentOutcome::Rejected { reason } => match reason {
            PlaybackIntentRejection::EmptyTarget => "Nothing to play",
            PlaybackIntentRejection::ResolutionFailed => "Couldn't load playback items",
            PlaybackIntentRejection::AudioOnly => "Can't play audio in video mode",
            PlaybackIntentRejection::InvalidTarget => "Invalid playback target",
            PlaybackIntentRejection::Unavailable => "Playback unavailable",
        },
    }
}

/// The one message for a direct-daemon pipe-output status (extracted from
/// `handle_player_event`). These statuses only originate from a direct
/// pipe-output daemon. Local, attached-Emby, and ordinary daemon routes
/// never receive the event, so their presentation is unchanged.
fn pipe_playback_message(status: &mbv_ctrl::PipePlaybackStatus) -> String {
    use mbv_ctrl::PipePlaybackPhase;
    match status.phase {
        PipePlaybackPhase::Resolving => "Resolving pipe playback target".to_string(),
        PipePlaybackPhase::PlayerOpening => "Opening player output".to_string(),
        PipePlaybackPhase::OutputStarted => {
            "Output started; downstream delay is unknown".to_string()
        }
        PipePlaybackPhase::OutputBuffering => {
            let remaining = status.estimated_remaining_ms.unwrap_or_default();
            format!("Output started; estimated output buffering (~{remaining} ms remaining)")
        }
    }
}
