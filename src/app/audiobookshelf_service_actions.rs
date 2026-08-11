use super::notify_actions::ToastSeverity;
use super::App;
use mbv_core::service_runtime::ServiceState;

impl App {
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

    pub(super) fn remove_audiobookshelf_confirmed(&mut self) {
        if let Err(error) = mbv_core::config::remove_audiobookshelf_setup_and_secret() {
            self.flash(
                format!("Could not remove Audiobookshelf safely: {error}"),
                ToastSeverity::Error,
            );
            return;
        }
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
        let result = mbv_core::config::replace_audiobookshelf_candidate(
            mbv_core::audiobookshelf::AudiobookshelfValidatedSetup::new(
                candidate.setup,
                candidate.user,
                candidate.api_key,
            ),
            || Ok(()),
            || {},
        );
        match result {
            Ok(_) => {
                self.config.lock().unwrap().audiobookshelf_setup = Some(setup.clone());
                self.audiobookshelf_runtime
                    .commit_ready(generation, user.clone());
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
}
