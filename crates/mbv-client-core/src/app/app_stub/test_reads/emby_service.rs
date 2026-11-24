//! The Emby Service runtime's test seams (`split-tui-gui-clients` design
//! D4b rule 5): the startup, setup, and failure outcomes a TUI-staying test
//! drives through the production transitions, and the Service state it
//! reads. They sit in their own child module so `home.rs` stays within its
//! line ceiling, as the GUI Feeds and Search seams do.

use super::super::App;

impl App {
    /// Apply `completion`'s Emby startup outcome through the production
    /// transition, as the startup worker's answer does.
    #[cfg(any(test, feature = "test"))]
    pub fn apply_emby_completion_for_test(
        &mut self,
        completion: crate::app::dispatch::session::service_startup::Completion,
    ) -> Option<crate::app::state::playback::HomeContent> {
        self.apply_emby_completion(completion)
    }

    /// Apply `completion`'s Emby setup outcome through the production
    /// transition without its network start, as a setup answer the test
    /// delivers itself does.
    #[cfg(any(test, feature = "test"))]
    pub fn apply_emby_setup_completion_without_network_for_test(
        &mut self,
        completion: crate::app::dispatch::session::service_startup::SetupCompletion,
    ) -> Option<crate::app::state::playback::HomeContent> {
        self.apply_emby_setup_completion_without_network(completion)
    }

    /// Land one Emby startup failure on the current setup generation, as the
    /// startup worker's own `Err` completion does.
    #[cfg(any(test, feature = "test"))]
    pub fn apply_emby_startup_failure_for_test(&mut self, failure: mbv_emby::EmbyFailure) {
        let generation = self.emby_setup_generation_for_test();
        let _ = self.apply_emby_completion_for_test(
            crate::app::dispatch::session::service_startup::Completion {
                generation,
                result: Err(failure),
            },
        );
    }

    /// Land one Emby setup failure on the current setup generation, as the
    /// setup worker's own `Err` completion does.
    #[cfg(any(test, feature = "test"))]
    pub fn apply_emby_setup_failure_for_test(
        &mut self,
        previous_state: mbv_core::service_runtime::ServiceState,
        error: mbv_emby::EmbyError,
    ) {
        let generation = self.emby_setup_generation_for_test();
        let _ = self.apply_emby_setup_completion_without_network_for_test(
            crate::app::dispatch::session::service_startup::SetupCompletion {
                generation,
                previous_state,
                result: Err(error),
            },
        );
    }

    /// Run the Emby startup-worker disconnect through the production
    /// transition, as a dropped worker does.
    #[cfg(any(test, feature = "test"))]
    pub fn handle_emby_startup_worker_disconnect_for_test(
        &mut self,
        generation: mbv_core::service_runtime::SetupGeneration,
    ) {
        self.handle_emby_startup_worker_disconnect(generation);
    }

    /// Run the Emby runtime failure through the production transition, as a
    /// failed request does.
    #[cfg(any(test, feature = "test"))]
    pub fn handle_emby_runtime_failure_for_test(&mut self, error: mbv_emby::EmbyFailure) {
        self.handle_emby_runtime_failure(error);
    }

    /// The Emby Service's state, as a completion or a connect leaves it.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn emby_service_state_for_test(&self) -> mbv_core::service_runtime::ServiceState {
        self.emby_runtime.state
    }
}
