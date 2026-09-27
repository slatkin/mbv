/// Runtime-only availability of a configured Remote Service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    NotConfigured,
    Connecting,
    Ready,
    NeedsAuthentication,
    Unavailable,
}

/// A monotonically increasing identity for one Remote Service setup.
/// Completion messages must carry the generation they started with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SetupGeneration(u64);

impl SetupGeneration {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}
