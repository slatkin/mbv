//! The GUI Library's shared per-library request state (PR 1021 fix 8): a
//! request is absent before it starts, `Requesting` while a worker fetch runs,
//! and `Landed` with the rows once it ends. A fetch that returns no rows or
//! fails lands empty, so the list paints its empty state and no frame requests
//! it again. The TUI's own request stores keep their existing rules.

/// One GUI Library list request (PR 1021 fix 8): the map entry's absence is
/// the third state, `Requesting` blocks a second fetch, and `Landed` carries
/// the fetched rows — empty for a legitimate no-result or a failed fetch.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum GuiSectionRequest<T> {
    /// The worker fetch is in flight.
    Requesting,
    /// The fetch ended; its rows may be empty.
    Landed(Vec<T>),
}

impl<T> GuiSectionRequest<T> {
    /// The landed request for `items`, including an empty list (PR 1021 fix 8).
    pub(crate) fn landed(items: Vec<T>) -> Self {
        Self::Landed(items)
    }

    /// The landed rows; empty while the request is still in flight.
    pub(crate) fn items(&self) -> &[T] {
        match self {
            Self::Requesting => &[],
            Self::Landed(items) => items,
        }
    }

    /// Whether the request has ended, so no frame starts it again.
    pub(crate) fn is_landed(&self) -> bool {
        matches!(self, Self::Landed(_))
    }
}
