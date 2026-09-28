/// Identity of one row in the Home content list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeRowTarget {
    pub item_id: Option<String>,
    pub source: Option<String>,
    pub from_continue_watching: bool,
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
