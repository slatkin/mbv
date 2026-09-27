use crate::library::LibraryKey;

/// Stable coordination identity for a `MediaList` selection. This identifies
/// the list that produced a projection or delayed action; it never carries
/// selection membership.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SelectionOrigin {
    Library(LibrarySelectionOrigin),
    Queue,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LibrarySelectionOrigin {
    Home,
    Feeds,
    Service(LibraryKey),
}

impl From<LibraryKey> for LibrarySelectionOrigin {
    fn from(key: LibraryKey) -> Self {
        match key {
            LibraryKey::Home => Self::Home,
            LibraryKey::Feeds => Self::Feeds,
            key @ LibraryKey::Service { .. } => Self::Service(key),
        }
    }
}

impl From<LibrarySelectionOrigin> for LibraryKey {
    fn from(origin: LibrarySelectionOrigin) -> Self {
        match origin {
            LibrarySelectionOrigin::Home => Self::Home,
            LibrarySelectionOrigin::Feeds => Self::Feeds,
            LibrarySelectionOrigin::Service(key) => key,
        }
    }
}
