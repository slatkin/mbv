use super::App;

impl App {
    pub(super) fn test_audiobookshelf_connection(&mut self) {
        if self.audiobookshelf_runtime.state
            == mbv_core::service_runtime::ServiceState::NotConfigured
        {
            return;
        }
        let config = self.config.lock().unwrap().clone();
        let generation = self.audiobookshelf_runtime.begin_validation();
        self.audiobookshelf_test_rx = Some(super::service_startup::start_audiobookshelf(
            config,
            generation,
            super::service_startup::AudiobookshelfCompletionKind::Test,
        ));
    }

    pub(super) fn clear_audiobookshelf_catalog(&mut self) {
        self.audiobookshelf_catalog_rx = None;
        self.audiobookshelf_libraries.clear();
        self.audiobookshelf_browse.clear();
        self.audiobookshelf_book_browse.clear();
        self.clear_audiobookshelf_images();
    }

    pub(super) fn clear_audiobookshelf_images(&mut self) {
        self.card_image_states
            .retain(|key, _| !key.starts_with(super::images::AUDIOBOOKSHELF_CACHE_KEY_PREFIX));
        self.card_image_loading
            .retain(|key| !key.starts_with(super::images::AUDIOBOOKSHELF_CACHE_KEY_PREFIX));
        self.pending_image_fetches.retain(|request| {
            !matches!(
                request.source,
                super::images::ImageSource::Audiobookshelf { .. }
            )
        });
        crate::config::clear_image_disk_cache_prefix(
            super::images::AUDIOBOOKSHELF_CACHE_KEY_PREFIX,
        );
    }
    pub(super) fn apply_audiobookshelf_completion(
        &mut self,
        completion: super::service_startup::AudiobookshelfCompletion,
    ) {
        use super::notify_actions::ToastSeverity;
        if !self.audiobookshelf_runtime.accepts(completion.generation) {
            log::debug!(target: "startup", "ignored stale Audiobookshelf completion");
            return;
        }
        match completion.result {
            Ok(user) => {
                let Some(setup) = self.config.lock().unwrap().audiobookshelf_setup.clone() else {
                    return;
                };
                self.audiobookshelf_runtime
                    .commit_ready(completion.generation, user.clone());
                self.start_audiobookshelf_socket(completion.generation);
                self.install_audiobookshelf_player_context(completion.generation);
                self.audiobookshelf_catalog_rx =
                    Some(super::service_startup::start_audiobookshelf_catalog(
                        self.config.lock().unwrap().clone(),
                        completion.generation,
                    ));
                if matches!(
                    completion.kind,
                    super::service_startup::AudiobookshelfCompletionKind::Test
                ) {
                    self.flash(
                        format!(
                            "Audiobookshelf {} is ready for {}",
                            setup.server_url, user.username
                        ),
                        ToastSeverity::Success,
                    );
                }
            }
            Err(error) => {
                let state = super::service_startup::classify_audiobookshelf_failure(&error);
                self.audiobookshelf_runtime
                    .complete(completion.generation, state);
                if state == mbv_core::service_runtime::ServiceState::NeedsAuthentication {
                    let deletion = self.clear_audiobookshelf_authentication();
                    self.flash(
                        match deletion {
                            Ok(()) => "Audiobookshelf rejected its saved credential; set it up again".into(),
                            Err(error) => format!("Audiobookshelf rejected its saved credential; could not remove it: {error}"),
                        },
                        ToastSeverity::Warning,
                    );
                } else {
                    self.flash(
                        format!("Audiobookshelf unavailable: {error}"),
                        ToastSeverity::Warning,
                    );
                }
            }
        }
    }

    pub(super) fn handle_audiobookshelf_worker_disconnect(
        &mut self,
        generation: mbv_core::service_runtime::SetupGeneration,
    ) {
        if !self.audiobookshelf_runtime.accepts(generation) {
            return;
        }
        let config = self.config.lock().unwrap().clone();
        let state = if config.audiobookshelf_setup.is_some()
            && mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Audiobookshelf)
                .is_some()
        {
            mbv_core::service_runtime::ServiceState::Unavailable
        } else if config.audiobookshelf_setup.is_some() {
            mbv_core::service_runtime::ServiceState::NeedsAuthentication
        } else {
            mbv_core::service_runtime::ServiceState::NotConfigured
        };
        self.audiobookshelf_runtime.complete(generation, state);
    }
}
