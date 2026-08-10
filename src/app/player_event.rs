use super::notify_actions::ToastSeverity;
use super::{App, DaemonLostModal, QUIT_REQUESTED};
use mbv_core::player::{PlayerCommand, PlayerEvent};
use std::sync::atomic::Ordering;

impl App {
    /// Mirror mpv's actual volume into `ui_volume` and persist it, so volume
    /// changes made inside the mpv window (not just via mbv's keys) are kept and
    /// restored on the next launch. Skipped while controlling a remote session
    /// (the remote owns its volume) and while temporarily muted (so a mute
    /// doesn't clobber the saved level with 0).
    pub(super) fn sync_volume_from_player(&mut self) {
        if self.connected_session_id.is_some() {
            return;
        }
        if self.pre_mute_volume.is_some() {
            return;
        }
        let player_vol = {
            let s = self.player.status.lock().unwrap();
            if s.active {
                Some(s.volume.clamp(0, 200) as u8)
            } else {
                None
            }
        };
        if let Some(v) = player_vol {
            if v != self.ui_volume {
                self.ui_volume = v;
                self.save_prefs();
            }
        }
    }

    /// Handle a PlayerEvent received from the player thread.
    /// Returns true if the caller's event loop should `continue` (skip render for this tick).
    pub(super) fn handle_player_event(&mut self, ev: PlayerEvent) -> bool {
        match ev {
            PlayerEvent::Stopped {
                idx,
                position_ticks,
                played,
                consume,
                progress_report_accepted,
                error,
            } => {
                log::info!(target: "player", "Stopped event: idx={idx} position_ticks={}s played={played} error={error:?}",
                    position_ticks / mbv_core::api::TICKS_PER_SECOND);
                if self.player.is_remote_disconnected() {
                    self.next_up_item = None;
                    self.skip_intro_end_ticks = None;
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
                    // today's silent-fallback behavior (task 7.2).
                    if self.is_local_daemon() {
                        self.raise_daemon_lost_modal();
                    } else {
                        self.restore_local_mode("Daemon disconnected — returned to local mode");
                    }
                    self.refresh_after_stop();
                    return true;
                }
                let is_delete = self.pending_delete_slot.take().is_some();
                let preserve_local_state = !self.has_direct_remote_queue();
                // Resolve the raw mpv index to a slot right away.
                let slot_id = self.playback_queue().resolve_slot_at(idx);
                match slot_id {
                    Some(slot_id) => {
                        if !is_delete {
                            let position = if played {
                                0
                            } else if let Some(slot) = self.playback_queue().queue.slot(slot_id) {
                                if position_ticks > 0 && !slot.item.is_audio() {
                                    position_ticks
                                } else {
                                    slot.item.playback_position_ticks()
                                }
                            } else {
                                0
                            };
                            let queue = self.playback_queue_mut();
                            let _ = queue.queue.apply_progress(slot_id, position, played);
                            if progress_report_accepted {
                                let _ = queue.queue.mark_progress_sync_pending(slot_id);
                            }
                            queue.clamp_cursor();
                            if played {
                                log::info!(target: "player", "Stopped: marked played, position reset to 0");
                            } else if position_ticks > 0 {
                                log::info!(target: "player", "Stopped: saved position={}s", position_ticks / mbv_core::api::TICKS_PER_SECOND);
                            } else {
                                log::info!(target: "player", "Stopped: position not saved (position_ticks={position_ticks})");
                            }
                        }
                        if preserve_local_state {
                            if let Some(slot) = self.playback_queue().queue.slot(slot_id) {
                                self.last_played_item_id = Some(slot.item.id().to_string());
                                self.last_played_completed = played;
                            }
                        }
                    }
                    None => {
                        log::warn!(target: "player", "Stopped: idx={idx} maps to no live slot; \
                            skipping progress update");
                    }
                }
                self.next_up_item = None;
                self.skip_intro_end_ticks = None;
                self.status.clear();
                if is_delete {
                    // The removal, undo-push, and cursor-clamp already happened
                    // immediately at confirm time (input_confirm_keys.rs), so
                    // the visible list update isn't blocked on this round trip.
                    // All that's left here is telling the player session to
                    // drop the slot from its own internal queue mirror and
                    // mpv's playlist — that still depends on this event, since
                    // nothing told it about the removal until now.
                    self.player.send_command(PlayerCommand::QueueRemove(idx));
                } else {
                    let (should_consume, is_audio) = match slot_id {
                        Some(slot_id) => self.should_consume_slot(slot_id, consume),
                        None => (false, false),
                    };
                    if should_consume {
                        let slot_id = slot_id.expect("should_consume implies a resolved slot");
                        let removed_id = self.consume_slot_from_active_playback_queue(slot_id);
                        self.playback_queue_mut().clamp_cursor();
                        log::info!(target: "consume", "Stopped-path: removed slot_id={slot_id:?} \
                            removed_id={removed_id:?}");
                        if removed_id.is_none() {
                            log::warn!(target: "consume", "Stopped-path: slot_id={slot_id:?} not \
                                found, removal SKIPPED");
                        }
                        if is_audio {
                            self.on_audio_consumed();
                        } else {
                            self.on_video_consumed();
                        }
                    }
                }
                self.playback_queue_mut().queue.clear_active_slot();
                self.refresh_after_stop();
                if !self.has_direct_remote_queue() {
                    self.save_queue_state();
                }
            }
            PlayerEvent::TrackCompleted {
                idx,
                position_ticks,
                played,
                consume,
                progress_report_accepted,
            } => {
                // Resolve the raw mpv index to a slot right away.
                let Some(slot_id) = self.playback_queue().resolve_slot_at(idx) else {
                    log::warn!(target: "consume", "TrackCompleted: idx={idx} maps to no live slot; dropping");
                    return false;
                };
                let position = if played {
                    0
                } else if let Some(slot) = self.playback_queue().queue.slot(slot_id) {
                    // Only record meaningful progress (≥ 30 s) for video;
                    // audio and startup noise keep the prior value.
                    if position_ticks >= 300_000_000 && !slot.item.is_audio() {
                        position_ticks
                    } else {
                        slot.item.playback_position_ticks()
                    }
                } else {
                    return false;
                };
                let queue = self.playback_queue_mut();
                let _ = queue.queue.apply_progress(slot_id, position, played);
                if progress_report_accepted {
                    let _ = queue.queue.mark_progress_sync_pending(slot_id);
                }
                queue.clamp_cursor();
                let (should_consume, is_audio) = self.should_consume_slot(slot_id, consume);
                if should_consume {
                    self.pending_queue_removal = Some((slot_id, is_audio));
                }
            }
            PlayerEvent::TrackChanged(idx) => {
                self.visualizer_failed = false;
                self.skip_intro_end_ticks = None;
                self.next_up_item = None;
                if self.status.starts_with("Next up:") {
                    self.status.clear();
                }
                // Resolve the incoming index to a slot *before* draining any
                // deferred consume: `idx` is the player's report from
                // before it was told (via the QueueRemove sent below) that
                // the completed slot was removed, so it still lines up with
                // the queue's current, pre-removal shape.
                let target_slot_id = self.playback_queue().resolve_slot_at(idx);

                if let Some((slot_id, was_audio)) = self.pending_queue_removal.take() {
                    let len_before = self.playback_queue().total_queue_len();
                    let removed_id = self.consume_slot_from_active_playback_queue(slot_id);
                    let len_after = len_before - removed_id.is_some() as usize;
                    log::info!(target: "consume", "TrackChanged: consuming pending removal slot_id={slot_id:?} \
                        new_idx={idx} len_before={len_before} len_after={len_after} removed_id={removed_id:?}");
                    if removed_id.is_none() {
                        log::warn!(target: "consume", "TrackChanged: slot_id={slot_id:?} not found, \
                            removal SKIPPED");
                    }
                    if was_audio {
                        self.on_audio_consumed();
                    } else {
                        self.on_video_consumed();
                    }
                }

                // Activate the resolved slot by identity (order-independent,
                // unlike raw index arithmetic) and derive the display
                // cursor from its post-removal position — this stays
                // correct regardless of where the just-consumed slot sat
                // relative to `idx`.
                let adjusted = match target_slot_id {
                    Some(slot_id) => {
                        let _ = self.playback_queue_mut().queue.set_active_slot(slot_id);
                        self.playback_queue()
                            .queue
                            .slot_index(slot_id)
                            .unwrap_or(idx)
                    }
                    None => {
                        log::warn!(target: "player", "TrackChanged: idx={idx} maps to no live \
                            slot; skipping activation");
                        idx
                    }
                };
                self.player.status.lock().unwrap().current_idx = adjusted;
                self.playback_queue_mut().queue_cursor = adjusted;
                if !self.has_direct_remote_queue() {
                    if let Some(item) = self.playback_queue().emby_item_at(adjusted) {
                        self.last_played_item_id = Some(item.id.clone());
                    }
                }
                if !self.has_direct_remote_queue() {
                    let queue = self.playback_queue();
                    log::info!(target: "consume", "TrackChanged: post-save queue len={} ids={:?}",
                        queue.total_queue_len(), queue.slots().iter().map(|s| s.item.id()).collect::<Vec<_>>());
                    self.save_queue_state();
                }
            }
            PlayerEvent::QueueNextUp { next_idx } => {
                if let Some(item) = self.playback_queue().clone_emby_item_at(next_idx) {
                    let item_id = item.id.clone();
                    let show_title = item.series_name.clone();
                    let ep_title = item.name.clone();
                    let artist = item.artist.clone();
                    let label = item.playback_label();
                    self.next_up_item = Some(item.clone());
                    let next_up_msg = format!("Next up: {} (Y/n)", label);
                    self.notify_with_actions(
                        &item.name,
                        "Next up?",
                        &[("next_up:play", "Play Now"), ("next_up:skip", "Skip")],
                    );
                    self.status = next_up_msg;
                    self.status_expires = None;
                    // Daemon sends NextUpShow to mpv directly; only send from local player.
                    if !self.player.is_remote() {
                        self.player.send_command(PlayerCommand::NextUpShow {
                            item_id,
                            show_title,
                            ep_title,
                            artist,
                        });
                    }
                }
            }
            PlayerEvent::NextUpThreshold { .. } => {
                // Series episodes now use play_queue; this only fires for movies
                // (always_play_next=false or non-series content). No action needed.
            }
            PlayerEvent::NextUpPlay => {
                log::warn!(target: "app", "next-up: play triggered");
                if let Some(item) = self.next_up_item.take() {
                    let label = item.playback_label();
                    if let Some(idx) = self
                        .playback_queue()
                        .slots()
                        .iter()
                        .position(|s| matches!(&s.item, mbv_core::playback_queue::QueueItem::Emby(e) if e.id == item.id))
                    {
                        self.player.send_command(PlayerCommand::JumpTo(idx));
                        self.playback_queue_mut().queue_cursor = idx;
                        self.flash(label, ToastSeverity::Neutral);
                    } else {
                        log::warn!(target: "app", "next-up: item not in queue, cannot jump");
                    }
                } else {
                    log::warn!(target: "app", "next-up: NextUpPlay fired but next_up_item is None");
                }
            }
            PlayerEvent::QueueUpdated {
                items,
                cursor,
                source,
            } => {
                let pending_local_cursor = self.pending_queue_edit_cursor.take();
                let total = items.len();
                let cursor = if self.has_direct_remote_queue() {
                    self.pending_remote_move_cursor
                        .take()
                        .filter(|pending_cursor| *pending_cursor < total)
                        .unwrap_or(cursor)
                } else {
                    pending_local_cursor
                        .filter(|pending_cursor| *pending_cursor < total)
                        .unwrap_or(cursor)
                };
                let queue = self.playback_queue_mut();
                queue.set_items(items, cursor);
                if !self.has_direct_remote_queue() {
                    self.queue_source = source;
                }
            }
            PlayerEvent::UnifiedQueueUpdated(unified) => {
                let total = unified.slots.len();

                // Derive the presentation cursor from the active slot index.
                let active_index = unified
                    .active_slot
                    .and_then(|sid| unified.slots.iter().position(|s| s.slot_id == sid));
                let active_cursor = active_index.unwrap_or(0);

                let pending_local_cursor = self.pending_queue_edit_cursor.take();
                let cursor = if self.has_direct_remote_queue() {
                    self.pending_remote_move_cursor
                        .take()
                        .filter(|pc| *pc < total)
                        .unwrap_or(active_cursor)
                } else {
                    pending_local_cursor
                        .filter(|pc| *pc < total)
                        .unwrap_or(active_cursor)
                };

                let source = unified.source.clone();
                let queue = self.playback_queue_mut();
                queue.set_unified_state(&unified, cursor);
                self.queue_source = source;
            }
            PlayerEvent::IntroStarted { intro_end_ticks } => {
                // mbvd never auto-seeks on this event itself — it always
                // reports the boundary neutrally, regardless of daemon-host
                // config, so this client's own `always_skip_intro` is the
                // only thing that decides whether to skip.
                if self.client.lock().unwrap().config.always_skip_intro {
                    let secs = intro_end_ticks as f64 / mbv_core::api::TICKS_PER_SECOND as f64;
                    self.player.send_command(PlayerCommand::SeekAbsolute(secs));
                    self.player.send_command(PlayerCommand::SkipIntroDismiss);
                } else {
                    self.skip_intro_end_ticks = Some(intro_end_ticks);
                    let playing_title = self
                        .playback_queue()
                        .item_at(self.playback_queue().queue_cursor)
                        .map(|i| i.title().to_string())
                        .unwrap_or_else(|| "mbv".into());
                    self.notify_with_actions(
                        &playing_title,
                        "Skip intro?",
                        &[("skip_intro:skip", "Skip"), ("skip_intro:ignore", "Ignore")],
                    );
                    self.status = "Skip intro? (Y/n)".into();
                    self.status_expires = None;
                }
            }
            PlayerEvent::IntroEnded => {
                if self.skip_intro_end_ticks.take().is_some() {
                    self.status.clear();
                }
            }
            PlayerEvent::SkipIntroPlay => {
                self.skip_intro_end_ticks = None;
                self.status.clear();
            }
            PlayerEvent::MpvQuit => {
                self.next_up_item = None;
                self.skip_intro_end_ticks = None;
                self.status.clear();
                self.refresh_after_stop();
            }
            PlayerEvent::CommandRejected(reason) => {
                self.pending_remote_move_cursor = None;
                self.flash(reason, ToastSeverity::Neutral);
            }
            PlayerEvent::PlaybackIntent(event) => {
                use mbv_core::ctrl::PlaybackIntentOutcome;
                let message = match event.outcome {
                    PlaybackIntentOutcome::Accepted => "Playback request accepted",
                    PlaybackIntentOutcome::Applied => "Playback request applied",
                    PlaybackIntentOutcome::Coalesced { .. } => "Playback request already pending",
                    PlaybackIntentOutcome::Superseded => "Playback request superseded",
                    PlaybackIntentOutcome::Rejected { ref reason } => {
                        use mbv_core::ctrl::PlaybackIntentRejection;
                        match reason {
                            PlaybackIntentRejection::EmptyTarget => "Nothing to play",
                            PlaybackIntentRejection::ResolutionFailed => {
                                "Couldn't load playback items"
                            }
                            PlaybackIntentRejection::AudioOnly => "Can't play audio in video mode",
                            PlaybackIntentRejection::InvalidTarget => "Invalid playback target",
                            PlaybackIntentRejection::Unavailable => "Playback unavailable",
                        }
                    }
                };
                self.flash(message.to_string(), ToastSeverity::Neutral);
            }
            PlayerEvent::PipePlaybackStatus(status) => {
                use mbv_core::ctrl::PipePlaybackPhase;
                let message = match status.phase {
                    PipePlaybackPhase::Resolving => "Resolving pipe playback target".to_string(),
                    PipePlaybackPhase::PlayerOpening => "Opening player output".to_string(),
                    PipePlaybackPhase::OutputStarted => {
                        "Output started; downstream delay is unknown".to_string()
                    }
                    PipePlaybackPhase::OutputBuffering => {
                        let remaining = status.estimated_remaining_ms.unwrap_or_default();
                        format!(
                            "Output started; estimated output buffering (~{} ms remaining)",
                            remaining
                        )
                    }
                };
                // These statuses only originate from a direct pipe-output
                // daemon. Local, attached-Emby, and ordinary daemon routes
                // never receive the event, so their presentation is unchanged.
                self.flash(message, ToastSeverity::Neutral);
            }
            PlayerEvent::PausedChanged(_) | PlayerEvent::OutputStarted => {}
            PlayerEvent::RemoteDisconnected(reason) => {
                self.restore_local_mode(&reason);
                self.refresh_after_stop();
                return true;
            }
            PlayerEvent::EmbyAuthorityTaken(reason) => {
                // Authority-change notification: Emby remote has taken authority.
                // The connection stays open — do NOT call restore_local_mode().
                // Just flash the status so the user knows commands are temporarily rejected.
                self.flash(reason, ToastSeverity::Warning);
            }
            PlayerEvent::QueueDesynced(reason) => {
                self.flash(reason, ToastSeverity::Neutral);
            }
            // The announced-shutdown counterpart to the unannounced-loss
            // modal raised from PlayerEvent::Stopped above (task 7.2): a
            // local-daemon client prints one line and exits cleanly; a
            // client of a genuinely remote daemon keeps today's behavior.
            PlayerEvent::DaemonShutdownAnnounced => {
                if self.is_local_daemon() {
                    self.pending_exit_message =
                        Some("mbv: the local daemon was stopped — exiting.".to_string());
                    QUIT_REQUESTED.store(true, Ordering::Relaxed);
                } else {
                    self.restore_local_mode("Daemon disconnected — returned to local mode");
                    self.refresh_after_stop();
                }
            }
        }
        false
    }

    /// Raises the blocking daemon-lost modal (task 7.1), replacing whatever
    /// other blocking overlay was showing -- only one is ever active.
    fn raise_daemon_lost_modal(&mut self) {
        self.confirm_modal = None;
        self.save_playlist_dialog = None;
        let last_playing_title = {
            let idx = self.player.status.lock().unwrap().current_idx;
            self.playback_queue()
                .item_at(idx)
                .map(|item| item.title().to_string())
        };
        self.daemon_lost_modal = Some(DaemonLostModal {
            last_playing_title,
            daemon_log_path: crate::state_dir()
                .join("local-daemon.log")
                .display()
                .to_string(),
            restart_error: None,
        });
    }
}
