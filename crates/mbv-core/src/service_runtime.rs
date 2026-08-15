use crate::api::EmbyClient;
use crate::api::EmbyItem;
use crate::audiobookshelf::AudiobookshelfUser;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbyFailureClass {
    AuthenticationRejected,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbyFailure {
    pub class: EmbyFailureClass,
    pub message: String,
}

impl EmbyFailure {
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            class: EmbyFailureClass::Unavailable,
            message: message.into(),
        }
    }
}

impl From<String> for EmbyFailure {
    fn from(message: String) -> Self {
        Self::unavailable(message)
    }
}

impl std::fmt::Display for EmbyFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Runtime-only availability of a configured Remote Service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    NotConfigured,
    Connecting,
    Ready,
    NeedsAuthentication,
    Unavailable,
}

/// One Home "Latest" section carried in the bootstrap: the display title, the
/// per-view identity used to key the merge into `HomePane.latest` (the view
/// id maps to `HomeLatestSource::Emby(view_id)`), and the view's items.
#[derive(Debug, Clone)]
pub struct EmbyLatestSection {
    pub title: String,
    pub view_id: String,
    pub items: Vec<EmbyItem>,
}

/// Network results needed to populate the existing Emby Home/library model.
/// The UI applies this value only after the bounded startup worker completes.
#[derive(Debug, Clone, Default)]
pub struct EmbyBootstrap {
    pub continue_items: Vec<EmbyItem>,
    pub views: Vec<EmbyItem>,
    pub latest: Vec<EmbyLatestSection>,
}

/// A monotonically increasing identity for one Remote Service setup.
/// Completion messages must carry the generation they started with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SetupGeneration(u64);

impl SetupGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }

    const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

/// The concrete Emby runtime. No placeholder client is created when Emby is
/// absent; `client` is populated only after a real setup is selected.
pub struct EmbyRuntime {
    pub client: Option<Arc<Mutex<EmbyClient>>>,
    pub state: ServiceState,
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

    pub const fn generation(&self) -> SetupGeneration {
        self.generation
    }

    pub fn replace_setup(&mut self) -> SetupGeneration {
        self.generation = self.generation.next();
        self.client = None;
        self.state = ServiceState::Connecting;
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

    pub fn accepts(&self, generation: SetupGeneration) -> bool {
        self.generation == generation
    }

    pub fn complete(&mut self, generation: SetupGeneration, state: ServiceState) -> bool {
        if !self.accepts(generation) {
            return false;
        }
        self.state = state;
        true
    }
}

/// Runtime-only Audiobookshelf identity and availability. The API key is
/// intentionally absent; callers load it from the Service secret boundary.
pub struct AudiobookshelfRuntime {
    pub user: Option<AudiobookshelfUser>,
    pub state: ServiceState,
    generation: SetupGeneration,
}

impl AudiobookshelfRuntime {
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

    pub const fn generation(&self) -> SetupGeneration {
        self.generation
    }

    pub fn begin_setup(&mut self) -> SetupGeneration {
        self.generation = self.generation.next();
        self.state = ServiceState::Connecting;
        self.generation
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

    pub fn complete(&mut self, generation: SetupGeneration, state: ServiceState) -> bool {
        if !self.accepts(generation) {
            return false;
        }
        self.state = state;
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
    fn setup_replacement_rejects_stale_completion() {
        let mut runtime = EmbyRuntime::new(true);
        let first = runtime.generation();
        let second = runtime.replace_setup();
        assert_ne!(first, second);
        assert!(!runtime.complete(first, ServiceState::Ready));
        assert_eq!(runtime.state, ServiceState::Connecting);
        assert!(runtime.complete(second, ServiceState::Unavailable));
    }

    #[test]
    fn removal_invalidates_in_flight_completion() {
        let mut runtime = EmbyRuntime::new(true);
        let generation = runtime.generation();
        runtime.remove_setup();
        assert!(!runtime.complete(generation, ServiceState::Ready));
        assert_eq!(runtime.state, ServiceState::NotConfigured);
    }
}
