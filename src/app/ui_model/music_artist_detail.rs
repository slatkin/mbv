use crate::app::ui_model::library::LibraryKey;
use crate::app::ui_model::msg::MusicArtistTarget;
use mbv_emby_model::EmbyItem;

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub(in crate::app) struct ArtistDetailKey {
    pub(in crate::app) destination: LibraryKey,
    pub(in crate::app) generation: u64,
    pub(in crate::app) artist_id: String,
    pub(in crate::app) revision: u64,
}

#[derive(Debug, Clone, Default)]
pub(in crate::app) struct ArtistDetailCacheEntry {
    pub(in crate::app) tracks: Vec<EmbyItem>,
    /// A terminal query failure (the Service rejected or does not support the
    /// `ArtistIds` query). Projection switches to the documented per-album
    /// aggregation fallback, and repeated pushes do not turn the Service
    /// error into an unbounded request loop.
    pub(in crate::app) failed: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::app) enum ArtistArtworkStatus {
    Loading,
    Ready,
    None,
}

/// Immediate facts for the focused artist. These derive from the settled
/// in-scope album set, so they are available while tracks/artwork are still
/// loading. Read by the task-6.4 Hero wiring.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(in crate::app) struct ArtistSummary {
    pub(in crate::app) name: String,
    pub(in crate::app) album_count: usize,
    pub(in crate::app) year_start: Option<u32>,
    pub(in crate::app) year_end: Option<u32>,
}

impl ArtistSummary {
    pub(in crate::app) fn year_span(&self) -> Option<String> {
        match (self.year_start, self.year_end) {
            (Some(start), Some(end)) if start != end => Some(format!("{start}\u{2013}{end}")),
            (Some(year), _) | (_, Some(year)) => Some(year.to_string()),
            (None, None) => None,
        }
    }
}

/// One settled album's slice of the artist Workspace snapshot (task 6.3):
/// tracks of that album only, in disc/track order.
#[derive(Clone)]
pub(in crate::app) struct ArtistTrackGroup {
    pub(in crate::app) album_id: String,
    pub(in crate::app) album_title: String,
    pub(in crate::app) tracks: Vec<EmbyItem>,
}

/// The shell-owned projection for one focused artist root. Its summary facts
/// and grouped rows feed the Music Hero and canonical track Workspace
/// (`MusicContent`, task 6.4). The legacy typed artist-artwork cache remains
/// retained for its completion machinery, but the Hero image source is now the
/// selected album's existing artwork path.
#[derive(Clone)]
pub(in crate::app) struct ArtistDetailProjection {
    pub(in crate::app) target: MusicArtistTarget,
    /// Immediate artist facts (task 6.2): the Hero's artist name, in-scope
    /// album count, and year span.
    pub(in crate::app) summary: ArtistSummary,
    pub(in crate::app) track_groups: Vec<ArtistTrackGroup>,
}

pub(in crate::app) fn track_matches_album(track: &EmbyItem, album_id: &str) -> bool {
    !album_id.is_empty() && track.album_id == album_id
}
