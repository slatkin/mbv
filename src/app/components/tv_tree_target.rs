//! Stable identities for rows in the TV show tree.

/// The closed identity vocabulary accepted by TV's embedded `TreeBrowser`.
/// Child identities include their ancestors because Emby child IDs are not
/// guaranteed to be unique outside their parent scope.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(in crate::app) enum TvTreeTarget {
    Show(String),
    Season {
        show: String,
        season: String,
    },
    Episode {
        show: String,
        season: String,
        episode: String,
    },
}
