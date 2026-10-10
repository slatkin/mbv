/// A Home tree root (design D1 of `home-recently-added-tree`). Roots resolve
/// locally in the Home owner and never cross the component boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeRoot {
    Continue,
    RecentlyAdded,
}

/// An age branch under the `Recently added` root. Like roots, age branches
/// resolve locally in the Home owner and never cross the component boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecentAge {
    New,
    Last7Days,
    Last14Days,
}

/// One Home tree node's target. The section is part of the identity, so one
/// item under both roots yields two distinct [`Self::Item`] targets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomeTreeTarget {
    Root(HomeRoot),
    Age(RecentAge),
    Item(HomeItemTarget),
}

/// One Home item row's Service-qualified identity. A `Continue` row is Emby
/// content addressed by item id; a `Recently added` row is any Service's item
/// addressed by its queue-item content id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomeItemTarget {
    Continue(mbv_ids::ItemId),
    Recent(mbv_queue::QueueItemContentId),
}

impl HomeItemTarget {
    /// Build the `Continue` target for one Emby item id.
    #[must_use]
    pub fn continue_item(item_id: impl Into<String>) -> Self {
        Self::Continue(mbv_ids::ItemId::new(item_id))
    }
}

/// Stable, component-resolved identity for one focused Grouped Music artist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusicArtistTarget {
    pub artist_id: Option<String>,
    pub artist_name: String,
    pub album_targets: Vec<String>,
    pub revision: u64,
}

impl MusicArtistTarget {
    #[must_use]
    pub fn same_source(&self, other: &Self) -> bool {
        self.artist_id == other.artist_id
            && self.artist_name == other.artist_name
            && self.album_targets == other.album_targets
    }
}
