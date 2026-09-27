use crate::app::ui_model::library::LibraryKey;

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
