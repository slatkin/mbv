use crate::app::App;
use crate::app::dispatch::notify::ToastSeverity;
use mbv_core::service_runtime::ServiceState;

impl App {
    fn signal_running_local_daemon(&mut self, revision: u64) {
        if let Err(error) = mbv_remote_player::signal_local_daemon_service_setup(
            mbv_queue::ServiceKind::Audiobookshelf,
            revision,
        ) {
            self.flash(error.to_string(), ToastSeverity::Warning);
        }
    }

    pub(in crate::app) fn clear_audiobookshelf_authentication(
        &mut self,
    ) -> Result<(), mbv_config::ConfigError> {
        let current_generation = self.audiobookshelf_runtime.generation();
        self.stop_audiobookshelf_socket();
        self.audiobookshelf_runtime
            .cancel_setup(current_generation, ServiceState::NeedsAuthentication);
        self.clear_audiobookshelf_catalog();
        self.stop_active_audiobookshelf_playback();
        self.audiobookshelf_runtime.user = None;
        mbv_config::clear_service_secret_result(mbv_queue::ServiceKind::Audiobookshelf)
    }

    fn stop_active_audiobookshelf_playback(&mut self) {
        let active_index = self.player.status_snapshot().current_idx;
        let active_is_audiobookshelf = self
            .playback_queue()
            .item_at(active_index)
            .is_some_and(mbv_queue::QueueItem::is_audiobookshelf);
        if active_is_audiobookshelf {
            self.player.stop();
        }
    }

    pub(in crate::app) fn apply_audiobookshelf_setup_completion(
        &mut self,
        completion: crate::app::dispatch::session::service_startup::AudiobookshelfSetupCompletion,
    ) {
        if !self.audiobookshelf_runtime.accepts(completion.generation) {
            return;
        }
        match completion.result {
            Ok(candidate) => {
                let existing = self.config.lock().unwrap().audiobookshelf_setup.clone();
                if existing
                    .as_ref()
                    .is_some_and(|setup| setup.server_url != candidate.setup.server_url)
                {
                    self.audiobookshelf_runtime
                        .complete(completion.generation, completion.previous_state);
                    self.setup.pending_audiobookshelf_replacement =
                        Some(crate::app::dispatch::session::service_startup::AudiobookshelfPendingReplacement {
                            candidate,
                            previous_state: completion.previous_state,
                        });
                    self.setup.audiobookshelf_setup_form = None;
                    self.ask_confirm(mbv_ui_model::confirm::ConfirmModal::two_button(
                        "Replace Audiobookshelf? Service-owned setup and state will be cleared."
                            .into(),
                        "Replace",
                        "Cancel",
                        mbv_ui_model::confirm::ConfirmAction::ReplaceAudiobookshelf(
                            completion.generation,
                        ),
                    ));
                    return;
                }
                let user = candidate.user.clone();
                let setup = candidate.setup.clone();
                let result = mbv_audiobookshelf::commit_audiobookshelf_candidate(
                    mbv_audiobookshelf::AudiobookshelfValidatedSetup::new(
                        candidate.setup,
                        candidate.user,
                        candidate.api_key,
                    ),
                );
                if let Ok((_, revision)) = result {
                    let mut committed = setup.clone();
                    committed.revision = revision;
                    self.config.lock().unwrap().audiobookshelf_setup = Some(committed);
                    self.audiobookshelf_runtime
                        .commit_ready(completion.generation, user.clone());
                    self.start_audiobookshelf_socket(completion.generation);
                    self.setup.audiobookshelf_setup_form = None;
                    self.signal_running_local_daemon(revision);
                    self.flash(
                        format!(
                            "Audiobookshelf {} is ready for {}",
                            setup.server_url, user.username
                        ),
                        ToastSeverity::Success,
                    );
                } else {
                    self.audiobookshelf_runtime
                        .complete(completion.generation, completion.previous_state);
                    if let Some(form) = self.setup.audiobookshelf_setup_form.as_mut() {
                        form.busy = false;
                        form.error = "Could not save Audiobookshelf setup".into();
                    }
                }
            }
            Err(error) => {
                self.audiobookshelf_runtime
                    .complete(completion.generation, completion.previous_state);
                if let Some(form) = self.setup.audiobookshelf_setup_form.as_mut() {
                    form.busy = false;
                    form.error = error.to_string();
                }
            }
        }
    }

    pub(in crate::app) fn handle_audiobookshelf_setup_worker_disconnect(&mut self) {
        let previous = self
            .setup
            .audiobookshelf_setup_form
            .as_ref()
            .map_or(ServiceState::NotConfigured, |form| form.previous_state);
        if let Some(form) = self.setup.audiobookshelf_setup_form.as_mut() {
            form.busy = false;
            form.error = "Audiobookshelf setup stopped unexpectedly; retry".into();
        }
        self.audiobookshelf_runtime.state = previous;
    }

    fn clear_audiobookshelf_queue_memory(&mut self) {
        // If the currently active slot is Audiobookshelf, stop playback.
        let active_index = self.player.status_snapshot().current_idx;
        let active_is_abs = self
            .playback_queue()
            .item_at(active_index)
            .is_some_and(mbv_queue::QueueItem::is_audiobookshelf);
        if active_is_abs {
            self.player.stop();
        }
        self.remove_queue_slots_where(mbv_queue::QueueItem::is_audiobookshelf);
        // Clear transient queue mutation state that might reference ABS slots.
        self.pending_delete_slot = None;
        self.queue_dirty = false;
    }

    pub(in crate::app) fn remove_audiobookshelf_confirmed(&mut self) {
        self.stop_audiobookshelf_socket();
        self.stop_active_audiobookshelf_playback();
        if let Err(error) = mbv_config::remove_audiobookshelf_setup_and_secret() {
            // Rollback: restore setup/secret handled inside transaction rollback;
            // The transaction restores durable setup, secret, and queue state;
            // in-memory queues have not been changed on this path.
            self.flash(
                format!("Could not remove Audiobookshelf safely: {error}"),
                ToastSeverity::Error,
            );
            return;
        }

        self.clear_audiobookshelf_catalog();
        self.clear_audiobookshelf_queue_memory();
        self.config.lock().unwrap().audiobookshelf_setup = None;
        self.audiobookshelf_runtime.remove_setup();
        self.signal_running_local_daemon(0);
        self.flash(
            "Audiobookshelf removed; Emby and Feeds remain available".into(),
            ToastSeverity::Success,
        );
    }

    pub(in crate::app) fn replace_audiobookshelf_confirmed(
        &mut self,
        generation: mbv_core::service_runtime::SetupGeneration,
    ) {
        if !self.audiobookshelf_runtime.accepts(generation) {
            return;
        }
        let previous_state = self
            .setup
            .pending_audiobookshelf_replacement
            .as_ref()
            .map_or(self.audiobookshelf_runtime.state, |pending| {
                pending.previous_state
            });
        self.stop_active_audiobookshelf_playback();
        let Some(pending) = self.setup.pending_audiobookshelf_replacement.take() else {
            return;
        };
        let candidate = pending.candidate;
        let user = candidate.user.clone();
        let setup = candidate.setup.clone();

        let result = mbv_audiobookshelf::replace_audiobookshelf_candidate(
            mbv_audiobookshelf::AudiobookshelfValidatedSetup::new(
                candidate.setup,
                candidate.user,
                candidate.api_key,
            ),
            || Ok(()),
            || {},
        );
        match result {
            Ok((_, revision)) => {
                self.audiobookshelf_runtime
                    .cancel_setup(generation, previous_state);
                let replacement_generation = self.audiobookshelf_runtime.generation();
                self.clear_audiobookshelf_catalog();
                self.clear_audiobookshelf_queue_memory();
                let mut committed = setup.clone();
                committed.revision = revision;
                self.config.lock().unwrap().audiobookshelf_setup = Some(committed);
                self.audiobookshelf_runtime
                    .commit_ready(replacement_generation, user.clone());
                self.start_audiobookshelf_socket(replacement_generation);
                self.signal_running_local_daemon(revision);
                self.flash(
                    format!(
                        "Audiobookshelf {} is ready for {}",
                        setup.server_url, user.username
                    ),
                    ToastSeverity::Success,
                );
            }
            Err(error) => {
                self.audiobookshelf_runtime.state = previous_state;
                self.flash(
                    format!("Could not replace Audiobookshelf safely: {error}"),
                    ToastSeverity::Error,
                );
            }
        }
    }

    // ---- Audiobookshelf Socket.IO lifecycle (tasks 2.5-2.6) ----

    /// Open an Audiobookshelf Socket.IO connection for the given setup
    /// generation. Shuts down any existing socket first (for replace).
    pub(in crate::app) fn start_audiobookshelf_socket(
        &mut self,
        generation: mbv_core::service_runtime::SetupGeneration,
    ) {
        // Shutdown any existing socket first (replace scenario).
        self.stop_audiobookshelf_socket();

        let Some((setup, key)) =
            crate::app::dispatch::session::service_startup::audiobookshelf_setup_and_key(
                &self.config.lock().unwrap(),
            )
        else {
            return;
        };
        let Some(url) = mbv_audiobookshelf::socket::socket_url(&setup.server_url) else {
            return;
        };
        let (event_tx, rx) = std::sync::mpsc::channel();
        self.audiobookshelf_socket_tx = Some(mbv_audiobookshelf::socket::start(url, key, event_tx));
        self.audiobookshelf_socket_rx = rx;
        self.audiobookshelf_socket_generation = Some(generation);
    }

    /// Shut down the Audiobookshelf Socket.IO connection (if any) and
    /// replace the receiver with a dummy so the drain loop has no effect.
    pub(in crate::app) fn stop_audiobookshelf_socket(&mut self) {
        if let Some(tx) = self.audiobookshelf_socket_tx.take() {
            let _ = tx.send(());
        }
        let (_, rx) = std::sync::mpsc::channel();
        self.audiobookshelf_socket_rx = rx;
        self.audiobookshelf_socket_generation = None;
    }

    /// Handle a decoded socket event. The progress-merge body (task 3.1-3.3)
    /// is delegated to `apply_audiobookshelf_socket_progress`.
    pub(in crate::app) fn handle_audiobookshelf_socket_event(
        &mut self,
        ev: mbv_audiobookshelf::socket::SocketEvent,
    ) {
        match ev {
            // Authenticated is a deliberate no-op here; Open, ConnectAck are
            // consumed by the background thread and never forwarded to the
            // app.
            mbv_audiobookshelf::socket::SocketEvent::Authenticated
            | mbv_audiobookshelf::socket::SocketEvent::Open { .. }
            | mbv_audiobookshelf::socket::SocketEvent::ConnectAck => {}
            mbv_audiobookshelf::socket::SocketEvent::InvalidToken => {
                // Task 2.3: surface the same ABS authentication failure
                // classification used elsewhere; do NOT clear the installed
                // API key alone from this.
                self.audiobookshelf_runtime.state = ServiceState::NeedsAuthentication;
                self.flash(
                    "Audiobookshelf rejected its credential over the socket connection".into(),
                    ToastSeverity::Warning,
                );
            }
            mbv_audiobookshelf::socket::SocketEvent::ProgressUpdated(progress) => {
                self.apply_audiobookshelf_socket_progress(&progress);
            }
        }
    }

    /// Apply a `user_item_progress_updated` event from the socket.
    ///
    /// Generation-gate, skip the active Player-owned slot, relay progress to
    /// the home owner, and merge the event into browse state without a REST call.
    fn apply_audiobookshelf_socket_progress(
        &mut self,
        progress: &mbv_audiobookshelf::socket::AudiobookshelfProgress,
    ) {
        // Task 3.3: drop events from a superseded connection generation.
        let Some(r#gen) = self.audiobookshelf_socket_generation else {
            return;
        };
        if !self.audiobookshelf_runtime.accepts(r#gen) {
            return;
        }

        // Task 3.2: never touch the actively Player-owned slot.
        let content_id = mbv_queue::QueueItemContentId::Audiobookshelf {
            library_item_id: progress.library_item_id.clone(),
            episode_id: progress.episode_id.clone(),
        };
        if self.player_owns_active_match(&content_id) {
            return;
        }

        // Task 3.1: only merge when the episode is known in browse or
        // queue (the socket spec says unmatched episodes SHALL apply
        // no change — no browse-map insert, unlike the daemon route).
        let known = self.audiobookshelf_browse.iter().any(|state| {
            state.progress.contains_key(&(
                progress.library_item_id.clone(),
                progress.episode_id.clone(),
            )) || state
                .detail_cache
                .get(&progress.library_item_id)
                .is_some_and(|eps| {
                    eps.iter().any(|ep| {
                        ep.library_item_id == progress.library_item_id
                            && ep.episode_id == progress.episode_id
                    })
                })
        }) || self.local_view.slots().iter().any(|slot| {
            slot.item.as_audiobookshelf().is_some_and(|ep| {
                ep.library_item_id == progress.library_item_id
                    && ep.episode_id == progress.episode_id
            })
        });
        if !known {
            return;
        }

        // Task 3.1: merge in place (no REST call) via the existing
        // shared reconcile path that the daemon-route ack also uses.
        let update = mbv_ctrl::ProgressUpdate {
            content_id,
            position_ticks: crate::app::dispatch::audiobookshelf::browse::seconds_to_ticks(
                progress.current_time_seconds,
            ),
            finished: progress.is_finished,
        };
        let result = {
            let (player, _) = self.queue_link(crate::app::QueueScope::Local);
            player
                .remote()
                .send_queue_op(mbv_remote_player::QueueOp::ApplyProgress {
                    updates: vec![update],
                })
        };
        if let Err(error) = result {
            self.flash(error.to_string(), ToastSeverity::Warning);
        }
        self.reconcile_audiobookshelf_progress(
            &progress.library_item_id,
            &progress.episode_id,
            progress.current_time_seconds,
            progress.is_finished,
        );
    }

    /// Returns `true` when the active slot in the Player owner's queue matches
    /// the given provider-qualified content identity. Shared by the socket
    /// merge and the accepted-mark local apply (standard-media-context-menus
    /// design D6): both paths must leave the actively owned session untouched.
    pub(in crate::app) fn player_owns_active_match(
        &self,
        content_id: &mbv_queue::QueueItemContentId,
    ) -> bool {
        let active_index = self.player.status_snapshot().current_idx;
        self.playback_queue()
            .item_at(active_index)
            .is_some_and(|item| item.content_id() == *content_id)
    }
}
