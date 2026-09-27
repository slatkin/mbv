use crate::library::LibraryKey;
use crate::msg::MusicArtistTarget;
use mbv_emby_model::EmbyItem;

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub struct ArtistDetailKey {
    pub destination: LibraryKey,
    pub generation: u64,
    pub artist_id: String,
    pub revision: u64,
}

#[derive(Debug, Clone, Default)]
pub struct ArtistDetailCacheEntry {
    pub tracks: Vec<EmbyItem>,
    /// A terminal query failure (the Service rejected or does not support the
    /// `ArtistIds` query). Projection switches to the documented per-album
    /// aggregation fallback, and repeated pushes do not turn the Service
    /// error into an unbounded request loop.
    pub failed: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ArtistArtworkStatus {
    Loading,
    Ready,
    None,
}

/// Immediate facts for the focused artist. These derive from the settled
/// in-scope album set, so they are available while tracks/artwork are still
/// loading. Read by the task-6.4 Hero wiring.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ArtistSummary {
    pub name: String,
    pub album_count: usize,
    pub year_start: Option<u32>,
    pub year_end: Option<u32>,
}

impl ArtistSummary {
    #[must_use]
    pub fn year_span(&self) -> Option<String> {
        match (self.year_start, self.year_end) {
            (Some(start), Some(end)) if start != end => Some(format!("{start}\u{2013}{end}")),
            (Some(year), _) | (_, Some(year)) => Some(year.to_string()),
            (None, None) => None,
        }
    }
}

/// One settled album's slice of the artist Workspace snapshot (task 6.3):
/// tracks of that album only, in disc/track order.
#[derive(Clone, Debug)]
pub struct ArtistTrackGroup {
    pub album_id: String,
    pub album_title: String,
    pub tracks: Vec<EmbyItem>,
}

/// The shell-owned projection for one focused artist root. Its summary facts
/// and grouped rows feed the Music Hero and canonical track Workspace
/// (`MusicContent`, task 6.4). The legacy typed artist-artwork cache remains
/// retained for its completion machinery, but the Hero image source is now the
/// selected album's existing artwork path.
#[derive(Clone, Debug)]
pub struct ArtistDetailProjection {
    pub target: MusicArtistTarget,
    /// Immediate artist facts (task 6.2): the Hero's artist name, in-scope
    /// album count, and year span.
    pub summary: ArtistSummary,
    pub track_groups: Vec<ArtistTrackGroup>,
}

#[must_use]
pub fn track_matches_album(track: &EmbyItem, album_id: &str) -> bool {
    !album_id.is_empty() && track.album_id == album_id
}
