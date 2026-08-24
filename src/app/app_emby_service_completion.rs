use mbv_core::api::EmbyClient;
use mbv_core::playback_queue::QueueItem;
use std::sync::{mpsc, Arc, Mutex};

use super::types_playback::HomeLatestSource;
use super::App;

impl App {
    pub(super) fn emby_client(&self) -> Option<Arc<Mutex<EmbyClient>>> {
        self.emby_runtime.client.clone()
    }

    pub(super) fn emby_snapshot(&self) -> Option<EmbyClient> {
        self.emby_client()
            .map(|client| client.lock().unwrap().clone())
    }

    pub(super) fn apply_emby_completion(&mut self, completion: super::service_startup::Completion) {
        self.transition_emby_failure(Some(completion.generation), completion.result, |kind| {
            mbv_core::config::clear_service_secret_result(kind)
        });
    }

    #[cfg(test)]
    pub(super) fn apply_emby_completion_with_secret_deleter(
        &mut self,
        completion: super::service_startup::Completion,
        delete: impl FnOnce(mbv_core::config::ServiceKind) -> Result<(), String>,
    ) {
        self.transition_emby_failure(Some(completion.generation), completion.result, delete);
    }

    fn transition_emby_failure(
        &mut self,
        generation: Option<mbv_core::service_runtime::SetupGeneration>,
        result: Result<super::service_startup::Startup, mbv_core::service_runtime::EmbyFailure>,
        delete_secret: impl FnOnce(mbv_core::config::ServiceKind) -> Result<(), String>,
    ) {
        use super::notify_actions::ToastSeverity;
        if generation.is_some_and(|generation| !self.emby_runtime.accepts(generation)) {
            log::debug!(target: "startup", "ignored stale Emby startup completion");
            return;
        }
        match result {
            Ok(startup) => {
                let ws_url = startup.client.ws_url();
                // Capability registration is an HTTP POST. Keep it behind
                // successful authentication, but never make Ready
                // application wait on the Emby agent timeout.
                let capability_client = startup.client.clone();
                std::thread::spawn(move || capability_client.register_capabilities());
                let client = Arc::new(Mutex::new(startup.client));
                let (ws_tx, ws_rx) = mpsc::channel();
                self.ws_send_tx = Some(mbv_core::ws::start(ws_url, ws_tx));
                self.ws_rx = ws_rx;
                {
                    let client = client.lock().unwrap();
                    self.player.update_emby_credentials(
                        client.config.server_url.clone(),
                        client.token.clone(),
                    );
                }
                self.emby_runtime.client = Some(client);
                self.apply_emby_bootstrap(startup.bootstrap);
                self.emby_runtime.state = mbv_core::service_runtime::ServiceState::Ready;
                self.sync_subtitle_prefs_from_emby();
                self.flash("Emby is ready".into(), ToastSeverity::Success);
                log::info!(target: "startup", "Emby startup completed");
                // `App::new_independent`'s launch (no daemon attached at
                // construction) has no Emby client yet when it's built, so
                // it can't call `try_auto_reconnect` synchronously the way
                // `App::new_remote` does (construct.rs) -- it has to wait
                // for the async startup this method completes. Guarded on
                // `player_endpoint` being still unset so this only fires on
                // that first-ever completion, not on a later reconfigure
                // that also flows through this branch.
                if self.player_endpoint.is_none() {
                    self.try_auto_reconnect();
                    // Cast reattach doesn't depend on Emby readiness (7.3),
                    // but this is its only synchronous-construction-time
                    // opportunity for `App::new_independent` too, so it
                    // shares this same fires-once-per-launch guard.
                    self.try_cast_auto_reconnect();
                }
            }
            Err(error) => {
                let state = super::service_startup::classify_failure(&error);
                self.emby_runtime.state = state;
                if state == mbv_core::service_runtime::ServiceState::NeedsAuthentication {
                    self.emby_runtime.client = None;
                    self.ws_send_tx = None;
                    self.player
                        .update_emby_credentials(String::new(), String::new());
                    let deletion = delete_secret(mbv_core::config::ServiceKind::Emby);
                    let message = match deletion {
                        Ok(()) => format!("Emby rejected its saved credential: {error}; set up Emby again"),
                        Err(delete_error) => format!(
                            "Emby rejected its saved credential: {error}; could not remove the saved secret ({delete_error}); set up Emby again"
                        ),
                    };
                    self.flash(message, ToastSeverity::Warning);
                } else {
                    self.flash(format!("Emby unavailable: {error}"), ToastSeverity::Warning);
                }
                log::warn!(target: "startup", "Emby startup failed ({state:?}): {error}");
            }
        }
    }

    /// Central boundary for an authenticated Emby request made after startup.
    /// Only classified failures reach this path; ordinary empty results do not.
    pub(super) fn handle_emby_runtime_failure(
        &mut self,
        error: mbv_core::service_runtime::EmbyFailure,
    ) {
        self.transition_emby_failure(None, Err(error), |kind| {
            mbv_core::config::clear_service_secret_result(kind)
        });
    }

    #[cfg(test)]
    pub(super) fn handle_emby_runtime_failure_with_secret_deleter(
        &mut self,
        error: mbv_core::service_runtime::EmbyFailure,
        delete: impl FnOnce(mbv_core::config::ServiceKind) -> Result<(), String>,
    ) {
        self.transition_emby_failure(None, Err(error), delete);
    }

    pub(super) fn handle_emby_startup_worker_disconnect(
        &mut self,
        generation: mbv_core::service_runtime::SetupGeneration,
    ) {
        if !self.emby_runtime.accepts(generation) {
            return;
        }
        let config = self.config.lock().unwrap().clone();
        self.emby_runtime.state = if config.emby_setup.is_some()
            && mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Emby).is_some()
        {
            mbv_core::service_runtime::ServiceState::Unavailable
        } else if config.emby_setup.is_some() {
            mbv_core::service_runtime::ServiceState::NeedsAuthentication
        } else {
            mbv_core::service_runtime::ServiceState::NotConfigured
        };
        self.flash(
            super::service_startup::startup_status(self.emby_runtime.state).into(),
            super::notify_actions::ToastSeverity::Warning,
        );
    }

    pub(super) fn apply_emby_setup_completion(
        &mut self,
        completion: super::service_startup::SetupCompletion,
    ) {
        self.apply_emby_setup_completion_inner(completion, true);
    }

    #[cfg(test)]
    pub(super) fn apply_emby_setup_completion_without_network(
        &mut self,
        completion: super::service_startup::SetupCompletion,
    ) {
        self.apply_emby_setup_completion_inner(completion, false);
    }

    fn apply_emby_setup_completion_inner(
        &mut self,
        completion: super::service_startup::SetupCompletion,
        start_network: bool,
    ) {
        use super::notify_actions::ToastSeverity;
        if !self.emby_runtime.accepts(completion.generation) {
            log::debug!(target: "startup", "ignored stale Emby setup completion");
            return;
        }
        match completion.result {
            Ok(startup) => {
                let existing = self.config.lock().unwrap().emby_setup.clone();
                if existing.as_ref().is_some_and(|old| {
                    !super::service_startup::setup_identity_allows_commit(Some(old), &startup.setup)
                }) {
                    let generation = completion.generation;
                    let mut startup = startup;
                    startup.client.config.username.clear();
                    startup.client.config.password.clear();
                    startup.client.config.api_key.clear();
                    self.emby_runtime.state = completion.previous_state;
                    self.pending_emby_replacement = Some(startup);
                    self.emby_setup_form = None;
                    self.ask_confirm(super::types_confirm::ConfirmModal {
                        title: " Replace Emby ".into(),
                        message: "Replace Emby? The previous server's queues, positions, routes, caches, and credential will be cleared.".into(),
                        hint: "[y/Enter] Replace    [Esc] Cancel".into(),
                        on_confirm: super::types_confirm::ConfirmAction::ReplaceEmby(generation),
                    });
                    return;
                }
                let token = startup.client.token.clone();
                if let Err(error) =
                    mbv_core::config::persist_emby_setup_and_secret(&startup.setup, &token)
                {
                    self.emby_runtime.state = completion.previous_state;
                    if let Some(form) = self.emby_setup_form.as_mut() {
                        form.busy = false;
                        form.error = error;
                        form.fields[2].clear();
                    }
                    return;
                }
                let ws_url = startup.client.ws_url();
                if start_network {
                    let capability_client = startup.client.clone();
                    std::thread::spawn(move || capability_client.register_capabilities());
                }
                let setup = startup.setup.clone();
                let client = Arc::new(Mutex::new(startup.client));
                if start_network {
                    let (ws_tx, ws_rx) = mpsc::channel();
                    self.ws_send_tx = Some(mbv_core::ws::start(ws_url, ws_tx));
                    self.ws_rx = ws_rx;
                }
                let (server_url, token) = {
                    let client = client.lock().unwrap();
                    (client.config.server_url.clone(), client.token.clone())
                };
                self.player.update_emby_credentials(server_url, token);
                self.emby_runtime.client = Some(client);
                self.apply_emby_bootstrap(startup.bootstrap);
                self.emby_runtime.state = mbv_core::service_runtime::ServiceState::Ready;
                if start_network {
                    self.sync_subtitle_prefs_from_emby();
                }
                {
                    let mut config = self.config.lock().unwrap();
                    config.emby_setup = Some(setup);
                    config.server_url = config
                        .emby_setup
                        .as_ref()
                        .map(|setup| setup.server_url.clone())
                        .unwrap_or_default();
                    config.username.clear();
                    config.password.clear();
                    config.api_key.clear();
                }
                self.emby_setup_form = None;
                self.flash("Emby is ready".into(), ToastSeverity::Success);
            }
            Err(error) => {
                self.emby_runtime.state = completion.previous_state;
                if let Some(form) = self.emby_setup_form.as_mut() {
                    form.busy = false;
                    form.error = error;
                    form.fields[2].clear();
                }
            }
        }
    }

    pub(super) fn apply_emby_bootstrap(
        &mut self,
        bootstrap: mbv_core::service_runtime::EmbyBootstrap,
    ) {
        self.home.continue_items = bootstrap.continue_items;
        self.rebuild_library_tabs_from_views(&bootstrap.views);
        for lib_idx in 0..self.libs.len() {
            self.start_album_index(lib_idx, false);
        }

        // Merge, not replace: drop only the Emby entries and splice the fresh
        // Emby entries back at their previous positions, leaving entries from
        // other providers (Audiobookshelf/Feeds) untouched (#543 Part 1).
        let emby_sections: Vec<(String, HomeLatestSource, Vec<QueueItem>)> = bootstrap
            .latest
            .into_iter()
            .filter(|section| {
                let lower = section.title.to_lowercase();
                !self.hidden_latest.contains(&lower) && !self.hidden_libraries.contains(&lower)
            })
            .map(|section| {
                (
                    section.title,
                    HomeLatestSource::Emby(section.view_id),
                    section
                        .items
                        .into_iter()
                        .map(|item| QueueItem::Emby(Box::new(item)))
                        .collect(),
                )
            })
            .collect();
        super::library_load_actions::merge_home_sections(
            &mut self.home.latest,
            emby_sections,
            |source| matches!(source, HomeLatestSource::Emby(_)),
        );
        let sections = 1 + self.home.latest.len();
        self.home.section = self.home.section.min(sections.saturating_sub(1));
        self.home_loading = false;
    }
}
