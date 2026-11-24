use mbv_audiobookshelf::AudiobookshelfUser;
use mbv_core::service_runtime::{ServiceState, SetupGeneration};
use mbv_emby::EmbyClient;
use std::sync::{Arc, Mutex};

/// The concrete Emby runtime. No placeholder client is created when Emby is
/// absent; `client` is populated only after a real setup is selected.
#[derive(Debug)]
pub(in crate::app) struct EmbyRuntime {
    pub(in crate::app) client: Option<Arc<Mutex<EmbyClient>>>,
    pub(in crate::app) state: ServiceState,
    generation: SetupGeneration,
}

impl Default for EmbyRuntime {
    fn default() -> Self {
        Self::new(false)
    }
}

impl EmbyRuntime {
    pub fn ready(client: Arc<Mutex<EmbyClient>>) -> Self {
        Self {
            client: Some(client),
            state: ServiceState::Ready,
            generation: SetupGeneration::default(),
        }
    }

    #[must_use]
    pub fn new(configured: bool) -> Self {
        Self {
            client: None,
            state: if configured {
                ServiceState::Connecting
            } else {
                ServiceState::NotConfigured
            },
            generation: SetupGeneration::default(),
        }
    }

    #[must_use]
    pub const fn generation(&self) -> SetupGeneration {
        self.generation
    }

    fn begin_attempt(&mut self) -> SetupGeneration {
        self.generation = self.generation.next();
        self.state = ServiceState::Connecting;
        self.generation
    }

    /// Start a retry without discarding the last usable runtime.
    pub fn begin_retry(&mut self) -> SetupGeneration {
        self.begin_attempt()
    }

    pub fn begin_setup(&mut self) -> SetupGeneration {
        self.begin_attempt()
    }

    pub fn cancel_setup(&mut self, generation: SetupGeneration, state: ServiceState) -> bool {
        if !self.accepts(generation) {
            return false;
        }
        self.generation = self.generation.next();
        self.state = state;
        true
    }

    pub fn remove_setup(&mut self) -> SetupGeneration {
        self.generation = self.generation.next();
        self.client = None;
        self.state = ServiceState::NotConfigured;
        self.generation
    }

    #[must_use]
    pub fn accepts(&self, generation: SetupGeneration) -> bool {
        self.generation == generation
    }
}

/// Runtime-only Audiobookshelf identity and availability. The API key is
/// intentionally absent; callers load it from the Service secret boundary.
#[derive(Debug)]
pub(in crate::app) struct AudiobookshelfRuntime {
    pub(in crate::app) user: Option<AudiobookshelfUser>,
    pub(in crate::app) state: ServiceState,
    generation: SetupGeneration,
}

impl AudiobookshelfRuntime {
    #[must_use]
    pub fn new(configured: bool) -> Self {
        Self {
            user: None,
            state: if configured {
                ServiceState::Connecting
            } else {
                ServiceState::NotConfigured
            },
            generation: SetupGeneration::default(),
        }
    }

    #[must_use]
    pub const fn generation(&self) -> SetupGeneration {
        self.generation
    }

    pub fn begin_setup(&mut self) -> SetupGeneration {
        self.generation = self.generation.next();
        self.state = ServiceState::Connecting;
        self.generation
    }

    pub fn complete(&mut self, generation: SetupGeneration, state: ServiceState) -> bool {
        if !self.accepts(generation) {
            return false;
        }
        self.state = state;
        true
    }

    pub fn begin_validation(&mut self) -> SetupGeneration {
        self.generation = self.generation.next();
        self.state = ServiceState::Connecting;
        self.generation
    }

    pub fn remove_setup(&mut self) -> SetupGeneration {
        self.generation = self.generation.next();
        self.user = None;
        self.state = ServiceState::NotConfigured;
        self.generation
    }

    pub fn commit_ready(&mut self, generation: SetupGeneration, user: AudiobookshelfUser) -> bool {
        if self.generation != generation {
            return false;
        }
        self.user = Some(user);
        self.state = ServiceState::Ready;
        true
    }

    pub fn cancel_setup(&mut self, generation: SetupGeneration, state: ServiceState) -> bool {
        if !self.accepts(generation) {
            return false;
        }
        self.generation = self.generation.next();
        self.state = state;
        true
    }

    #[must_use]
    pub fn accepts(&self, generation: SetupGeneration) -> bool {
        self.generation == generation
    }
}
#[cfg(test)]
mod tests {
    use super::{EmbyRuntime, ServiceState};

    #[test]
    fn absent_emby_has_no_client_and_is_not_ready() {
        let runtime = EmbyRuntime::new(false);
        assert_eq!(runtime.state, ServiceState::NotConfigured);
        assert!(runtime.client.is_none());
    }

    #[test]
    fn removal_invalidates_in_flight_completion() {
        let mut runtime = EmbyRuntime::new(true);
        let generation = runtime.generation();
        runtime.remove_setup();
        assert!(!runtime.accepts(generation));
        assert_eq!(runtime.state, ServiceState::NotConfigured);
    }
}
