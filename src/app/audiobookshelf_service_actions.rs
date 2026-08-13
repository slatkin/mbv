use super::notify_actions::ToastSeverity;
use super::App;
use mbv_core::config::QueueState;
use mbv_core::playback_queue::QueueItem;
use mbv_core::service_runtime::ServiceState;

impl App {
    fn update_local_audiobookshelf_context(
        &self,
        context: Option<mbv_core::player::AudiobookshelfPlayerContext>,
    ) {
        self.player.update_audiobookshelf_context(context.clone());
        if let Some(suspended) = &self.suspended_local {
            suspended.player.update_audiobookshelf_context(context);
        }
    }

    pub(super) fn clear_audiobookshelf_authentication(&mut self) -> Result<(), String> {
        self.clear_audiobookshelf_catalog();
        self.update_local_audiobookshelf_context(None);
        self.audiobookshelf_runtime.user = None;
        mbv_core::config::clear_service_secret_result(mbv_core::config::ServiceKind::Audiobookshelf)
    }

    pub(super) fn apply_audiobookshelf_setup_completion(
        &mut self,
        completion: super::service_startup::AudiobookshelfSetupCompletion,
    ) {
        use super::notify_actions::ToastSeverity;
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
                    self.pending_audiobookshelf_replacement =
                        Some(super::service_startup::AudiobookshelfPendingReplacement {
                            candidate,
                            previous_state: completion.previous_state,
                        });
                    self.audiobookshelf_setup_form = None;
                    self.confirm_modal = Some(super::types_confirm::ConfirmModal {
                        title: " Replace Audiobookshelf ".into(),
                        message:
                            "Replace Audiobookshelf? Service-owned setup and state will be cleared."
                                .into(),
                        hint: "[y/Enter] Replace    [Esc] Cancel".into(),
                        on_confirm: super::types_confirm::ConfirmAction::ReplaceAudiobookshelf(
                            completion.generation,
                        ),
                    });
                    return;
                }
                let user = candidate.user.clone();
                let setup = candidate.setup.clone();
                let result = mbv_core::config::commit_audiobookshelf_candidate(
                    mbv_core::audiobookshelf::AudiobookshelfValidatedSetup::new(
                        candidate.setup,
                        candidate.user,
                        candidate.api_key,
                    ),
                );
                match result {
                    Ok(_) => {
                        self.config.lock().unwrap().audiobookshelf_setup = Some(setup.clone());
                        self.audiobookshelf_runtime
                            .commit_ready(completion.generation, user.clone());
                        self.install_audiobookshelf_player_context(completion.generation);
                        self.audiobookshelf_setup_form = None;
                        self.flash(
                            format!(
                                "Audiobookshelf {} is ready for {}",
                                setup.server_url, user.username
                            ),
                            ToastSeverity::Success,
                        );
                    }
                    Err(_) => {
                        self.audiobookshelf_runtime
                            .complete(completion.generation, completion.previous_state);
                        if let Some(form) = self.audiobookshelf_setup_form.as_mut() {
                            form.busy = false;
                            form.error = "Could not save Audiobookshelf setup".into();
                        }
                    }
                }
            }
            Err(error) => {
                self.audiobookshelf_runtime
                    .complete(completion.generation, completion.previous_state);
                if let Some(form) = self.audiobookshelf_setup_form.as_mut() {
                    form.busy = false;
                    form.error = error.to_string();
                }
            }
        }
    }

    pub(super) fn handle_audiobookshelf_setup_worker_disconnect(&mut self) {
        let previous = self
            .audiobookshelf_setup_form
            .as_ref()
            .map_or(ServiceState::NotConfigured, |form| form.previous_state);
        if let Some(form) = self.audiobookshelf_setup_form.as_mut() {
            form.busy = false;
            form.error = "Audiobookshelf setup stopped unexpectedly; retry".into();
        }
        self.audiobookshelf_runtime.state = previous;
    }

    /// Helper that persists a filtered queue or clears the file when empty.
    /// Mirrors Emby's `persist_filtered_queue` but for Audiobookshelf.
    fn persist_filtered_queue_abs(state: &Option<QueueState>) -> Result<(), String> {
        match state {
            Some(state) if !state.items.is_empty() => mbv_core::config::save_queue_state(state),
            _ => mbv_core::config::clear_queue_state(),
        }
    }

    fn clear_audiobookshelf_queue_memory(&mut self) {
        // If the currently active slot is Audiobookshelf, stop playback.
        let active_is_abs = self
            .playback_queue()
            .queue
            .active_slot()
            .is_some_and(|slot| matches!(slot.item, QueueItem::Audiobookshelf(_)));
        if active_is_abs {
            self.player.stop();
        }
        // Filter both local and remote player tabs, keeping Emby + Feed items.
        let mut queues = vec![&mut self.player_tab];
        if let Some(queue) = self.remote_player_tab.as_mut() {
            queues.push(queue);
        }
        for queue in queues {
            let cursor_before = queue.queue_cursor;
            let kept = queue
                .all_queue_items()
                .into_iter()
                .filter(|item| !matches!(item, QueueItem::Audiobookshelf(_)))
                .collect::<Vec<_>>();
            let new_cursor = cursor_before.min(kept.len().saturating_sub(1));
            queue.set_queue_items(kept, new_cursor);
        }
        // Clear transient queue mutation state that might reference ABS slots.
        self.pending_delete_slot = None;
        self.pending_queue_removal = None;
        // If queue_source was tied to ABS (currently QueueSource has no ABS variant,
        // but future-proof: if items empty, reset source).
        if self.player_tab.total_queue_len() == 0 {
            self.queue_source = crate::config::QueueSource::Unknown;
        }
        self.queue_dirty = false;
    }

    pub(super) fn remove_audiobookshelf_confirmed(&mut self) {
        // Snapshot for rollback if persistence fails, mirroring Emby removal.
        let old_queue = mbv_core::config::load_queue_state();
        let filtered = old_queue.as_ref().map(QueueState::without_audiobookshelf);
        // Use the transactional boundary that accepts a clear_owned_state closure.
        // Queue filtering (persisted + in-memory) is performed inside that closure
        // so setup/secret removal and queue purge are atomic from the caller's view.
        let persist_result =
            mbv_core::config::remove_audiobookshelf_setup_and_secret_with_owned_state(
                || Self::persist_filtered_queue_abs(&filtered),
                || {},
            );

        if let Err(error) = persist_result {
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
        self.update_local_audiobookshelf_context(None);
        self.clear_audiobookshelf_queue_memory();
        self.config.lock().unwrap().audiobookshelf_setup = None;
        self.audiobookshelf_runtime.remove_setup();
        self.flash(
            "Audiobookshelf removed; Emby and Feeds remain available".into(),
            ToastSeverity::Success,
        );
    }

    pub(super) fn replace_audiobookshelf_confirmed(
        &mut self,
        generation: mbv_core::service_runtime::SetupGeneration,
    ) {
        if !self.audiobookshelf_runtime.accepts(generation) {
            return;
        }
        let Some(pending) = self.pending_audiobookshelf_replacement.take() else {
            return;
        };
        let candidate = pending.candidate;
        let previous_state = pending.previous_state;
        let user = candidate.user.clone();
        let setup = candidate.setup.clone();

        // Snapshot old queue for rollback explanation (persisted state rollback
        // itself is handled inside the transaction's restore hook, but we also
        // need to restore in-memory queue on failure).
        let old_queue = mbv_core::config::load_queue_state();
        let filtered = old_queue.as_ref().map(QueueState::without_audiobookshelf);
        let old_player_items = self.player_tab.all_queue_items();
        let old_player_cursor = self.player_tab.queue_cursor;
        let old_remote_items = self
            .remote_player_tab
            .as_ref()
            .map(|tab| (tab.all_queue_items(), tab.queue_cursor));

        let result = mbv_core::config::replace_audiobookshelf_candidate(
            mbv_core::audiobookshelf::AudiobookshelfValidatedSetup::new(
                candidate.setup,
                candidate.user,
                candidate.api_key,
            ),
            || Self::persist_filtered_queue_abs(&filtered),
            || {
                // Restore in-memory queues on failure.
                self.player_tab
                    .set_queue_items(old_player_items.clone(), old_player_cursor);
                if let Some((items, cursor)) = old_remote_items.clone() {
                    if let Some(tab) = self.remote_player_tab.as_mut() {
                        tab.set_queue_items(items, cursor);
                    }
                }
                if let Some(q) = old_queue.as_ref() {
                    let _ = mbv_core::config::save_queue_state(q);
                }
            },
        );
        match result {
            Ok(_) => {
                self.clear_audiobookshelf_catalog();
                self.clear_audiobookshelf_queue_memory();
                self.config.lock().unwrap().audiobookshelf_setup = Some(setup.clone());
                self.audiobookshelf_runtime
                    .commit_ready(generation, user.clone());
                self.install_audiobookshelf_player_context(generation);
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

    pub(super) fn install_audiobookshelf_player_context(
        &self,
        generation: mbv_core::service_runtime::SetupGeneration,
    ) {
        let setup = self.config.lock().unwrap().audiobookshelf_setup.clone();
        let credential =
            mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Audiobookshelf);
        let context = setup.zip(credential).and_then(|(setup, credential)| {
            mbv_core::player::AudiobookshelfPlayerContext::new(
                generation,
                setup,
                credential,
                mbv_core::api::device_id(),
            )
        });
        self.update_local_audiobookshelf_context(context);
    }
}
