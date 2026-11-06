//! The per-list capacity report (design D6).

/// How many whole items of one shared list a Client painted fully visible in
/// the current frame.
///
/// The core's one source of paging stride and viewport clamping
/// (`client-interaction-core`: clients report capacity in items). A Client
/// never derives a stride or clamp from its own geometry outside this report.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ListCapacity {
    whole_items: usize,
}

impl ListCapacity {
    /// Report `whole_items` fully visible items.
    #[must_use]
    pub const fn new(whole_items: usize) -> Self {
        Self { whole_items }
    }

    /// The reported count of fully visible items.
    #[must_use]
    pub const fn whole_items(self) -> usize {
        self.whole_items
    }
}
