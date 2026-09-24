//! Grouped Music's stable, destination-facing tree-row identity.
//!
//! `MusicTreeTarget` is the only row identity the shared
//! [`TreeBrowser`](crate::app::components::list::tree_browser::TreeBrowser)
//! boundary accepts: the shared owner's arena identifiers stay private to
//! `src/app/components/list/tree_browser/`, so Music addresses rows through
//! these stable targets and never through a projection position.

use crate::app::state::music_grouping::ArtistKey;

/// The stable identity of one Grouped Music tree row (design D2/D6). Every arm
/// mirrors the shared owner's interning key one-for-one, so a target is stable
/// across ordinary settled-catalog replacement and never a projection row
/// position.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(in crate::app) enum MusicTreeTarget {
    Artist(ArtistKey),
    Album(String),
    Track { album: String, track: String },
}

impl MusicTreeTarget {
    /// The stable album target when this target is an album leaf; artist roots
    /// and cached tracks have none.
    pub(in crate::app) fn album_leaf_target(&self) -> Option<&str> {
        match self {
            Self::Album(target) => Some(target),
            Self::Artist(_) | Self::Track { .. } => None,
        }
    }

    /// Whether this target is an artist root.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn is_artist(&self) -> bool {
        matches!(self, Self::Artist(_))
    }
}
