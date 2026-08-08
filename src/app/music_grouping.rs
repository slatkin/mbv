use crate::app::render::{parse_album_folder_name, strip_article};
use crate::app::ui_util::natural_sort_key;
use crate::app::{App, ArtistHeaderSelection};
use mbv_core::api::EmbyItem;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// Monotonic counter identifying a loaded source snapshot for a music album
/// browse level. Bumped whenever the level's items change.
pub(super) type SourceRevision = u64;

/// How long a resolving candidate waits for in-flight artist lookups before
/// its remaining unresolved albums are forced to the deterministic fallback.
const SETTLE_WINDOW: Duration = Duration::from_secs(3);

/// A resolving snapshot of a music album level. Records which album IDs
/// still need a terminal artist identity and which have already resolved.
#[derive(Clone)]
pub(super) struct MusicGroupCandidate {
    pub(super) revision: SourceRevision,
    pub(super) parent_id: String,
    pub(super) unresolved: HashSet<String>,
    pub(super) resolved: HashMap<String, String>,
    pub(super) created_at: Instant,
}

#[derive(Clone)]
pub(super) struct GroupedAlbumEntry {
    pub(super) album_index: usize,
    pub(super) album_id: String,
    pub(super) artist: String,
    pub(super) sort_key: String,
    pub(super) year: String,
    pub(super) name: String,
}

#[derive(Clone)]
pub(super) struct GroupedAlbumGroup {
    pub(super) artist: String,
    pub(super) start: usize,
    pub(super) end: usize,
}

/// A settled, source-derived grouping for one music album browse level:
/// resolved artist identities, display metadata, precomputed sort keys,
/// sorted album order, group boundaries, and identity lookups.
#[derive(Clone)]
pub(super) struct GroupedAlbumCatalog {
    pub(super) revision: SourceRevision,
    pub(super) parent_id: String,
    /// Entries sorted by `sort_key`; `entries[i].album_index` indexes the
    /// raw `items` slice the catalog was built from.
    pub(super) entries: Vec<GroupedAlbumEntry>,
    /// Artist groups as `[start, end)` ranges into `entries`.
    pub(super) groups: Vec<GroupedAlbumGroup>,
    /// raw album index -> position in `entries`.
    pub(super) index_to_entry: HashMap<usize, usize>,
    /// album id -> position in `entries`.
    pub(super) id_to_entry: HashMap<String, usize>,
}

/// Per-music-album-level grouping lifecycle state.
#[derive(Clone)]
pub(super) struct MusicGroupingState {
    pub(super) revision: SourceRevision,
    pub(super) candidate: Option<MusicGroupCandidate>,
    pub(super) settled: Option<GroupedAlbumCatalog>,
}

impl MusicGroupingState {
    pub(super) fn new() -> Self {
        Self {
            revision: 0,
            candidate: None,
            settled: None,
        }
    }
}

impl GroupedAlbumCatalog {
    pub(super) fn group_for_artist(&self, artist: &str) -> Option<&GroupedAlbumGroup> {
        self.groups.iter().find(|g| g.artist == artist)
    }
}

/// Synchronous artist resolution chain shared by catalog building and the
/// pre-settle render fallback: `item.artist` -> resolved lookup (cache or
/// fetch result) -> folder-name parse -> literal "Unknown Artist". Never
/// schedules artist resolution work.
pub(super) fn derive_album_artist(item: &EmbyItem, resolved: Option<&str>) -> String {
    if !item.artist.is_empty() {
        return item.artist.clone();
    }
    if let Some(artist) = resolved {
        if !artist.is_empty() {
            return artist.to_string();
        }
    }
    if let Some((artist, _, _)) = parse_album_folder_name(&item.name) {
        return artist;
    }
    "Unknown Artist".to_string()
}

/// Display `(year, album_name)` for an album item, mirroring the current
/// renderer's rule: Emby-provided artist metadata selects the tagged year and
/// display name, otherwise a folder-name parse wins.
pub(super) fn derive_album_display_name(item: &EmbyItem) -> (String, String) {
    if !item.artist.is_empty() {
        let year_str = if item.production_year > 0 {
            item.production_year.to_string()
        } else {
            String::new()
        };
        (year_str, item.display_name())
    } else if let Some((_, year, album)) = parse_album_folder_name(&item.name) {
        let year_str = if year > 0 {
            year.to_string()
        } else {
            String::new()
        };
        (year_str, album)
    } else {
        (String::new(), item.display_name())
    }
}

/// Builds the settled grouped catalog for a source snapshot from the raw
/// items and a resolved artist lookup. Pure: no app state, no network.
pub(super) fn build_grouped_album_catalog(
    items: &[EmbyItem],
    resolved: &HashMap<String, String>,
) -> GroupedAlbumCatalog {
    let mut entries: Vec<GroupedAlbumEntry> = Vec::with_capacity(items.len());
    for (album_index, item) in items.iter().enumerate() {
        let artist = derive_album_artist(item, resolved.get(&item.id).map(String::as_str));
        let (year, name) = derive_album_display_name(item);
        let sort_key = natural_sort_key(strip_article(&artist));
        entries.push(GroupedAlbumEntry {
            album_index,
            album_id: item.id.clone(),
            artist: artist.clone(),
            sort_key,
            year,
            name,
        });
    }
    entries.sort_by_key(|e| e.sort_key.clone());

    let mut groups: Vec<GroupedAlbumGroup> = Vec::new();
    let mut start = 0;
    while start < entries.len() {
        let artist = entries[start].artist.clone();
        let mut end = start + 1;
        while end < entries.len() && entries[end].artist == artist {
            end += 1;
        }
        groups.push(GroupedAlbumGroup { artist, start, end });
        start = end;
    }

    let mut index_to_entry = HashMap::with_capacity(entries.len());
    let mut id_to_entry = HashMap::with_capacity(entries.len());
    for (pos, entry) in entries.iter().enumerate() {
        index_to_entry.insert(entry.album_index, pos);
        id_to_entry.insert(entry.album_id.clone(), pos);
    }

    GroupedAlbumCatalog {
        revision: 0,
        parent_id: String::new(),
        entries,
        groups,
        index_to_entry,
        id_to_entry,
    }
}

impl App {
    /// Starts (or supersedes) the grouping candidate for the current music
    /// album level when its items change: on load, refresh, or page append.
    /// Albums already carrying an artist identity (item tag, cache, or an
    /// empty cache tombstone) are terminal up front; the rest are scheduled
    /// for bounded artist lookups. A prior settled catalog stays visible
    /// while the replacement resolves.
    pub(super) fn start_or_supersede_music_grouping(&mut self, lib_idx: usize) {
        if !self.is_music_group_view(lib_idx) {
            return;
        }
        let to_fetch: Vec<String> = {
            let lib = &mut self.libs[lib_idx];
            let Some(level) = lib.nav_stack.last_mut() else {
                return;
            };
            let state = level
                .music_grouping
                .get_or_insert_with(MusicGroupingState::new);
            state.revision = state.revision.saturating_add(1);
            let mut candidate = MusicGroupCandidate {
                revision: state.revision,
                parent_id: level.parent_id.clone(),
                unresolved: HashSet::new(),
                resolved: HashMap::new(),
                created_at: Instant::now(),
            };
            for item in &level.items {
                if !item.artist.is_empty() {
                    continue;
                }
                match self.album_artist_cache.get(&item.id) {
                    Some(cached) if !cached.is_empty() => {
                        candidate.resolved.insert(item.id.clone(), cached.clone());
                    }
                    Some(_) => {}
                    None => {
                        candidate.unresolved.insert(item.id.clone());
                    }
                }
            }
            let to_fetch = candidate.unresolved.iter().cloned().collect();
            state.candidate = Some(candidate);
            to_fetch
        };
        if to_fetch.is_empty() {
            self.commit_music_grouping_candidate(lib_idx);
            return;
        }
        for album_id in to_fetch {
            self.fetch_album_artist(album_id);
        }
    }

    /// Advances the current music album level's candidate with an arriving
    /// artist result. Only the candidate whose revision still matches the
    /// active browse level may commit; superseded candidates are discarded.
    pub(super) fn advance_music_grouping_candidates(&mut self, album_id: &str, artist: &str) {
        for lib_idx in 0..self.libs.len() {
            if !self.is_music_group_view(lib_idx) {
                continue;
            }
            let to_commit = {
                let lib = &mut self.libs[lib_idx];
                let Some(level) = lib.nav_stack.last_mut() else {
                    continue;
                };
                let Some(state) = level.music_grouping.as_mut() else {
                    continue;
                };
                let Some(candidate) = state.candidate.as_mut() else {
                    continue;
                };
                if !candidate.unresolved.remove(album_id) {
                    continue;
                }
                candidate
                    .resolved
                    .insert(album_id.to_string(), artist.to_string());
                if candidate.created_at.elapsed() >= SETTLE_WINDOW {
                    candidate.unresolved.clear();
                }
                candidate.unresolved.is_empty()
            };
            if to_commit {
                self.commit_music_grouping_candidate(lib_idx);
            }
        }
    }

    /// Force-settles candidates whose lookup window expired, including the
    /// case where every lookup failed before producing an event.
    pub(super) fn expire_music_grouping_candidates(&mut self) {
        let mut expired = Vec::new();
        for lib_idx in 0..self.libs.len() {
            if !self.is_music_group_view(lib_idx) {
                continue;
            }
            let should_commit = {
                let lib = &mut self.libs[lib_idx];
                let Some(level) = lib.nav_stack.last_mut() else {
                    continue;
                };
                let Some(state) = level.music_grouping.as_mut() else {
                    continue;
                };
                let Some(candidate) = state.candidate.as_mut() else {
                    continue;
                };
                if candidate.created_at.elapsed() < SETTLE_WINDOW {
                    false
                } else {
                    candidate.unresolved.clear();
                    true
                }
            };
            if should_commit {
                expired.push(lib_idx);
            }
        }
        for lib_idx in expired {
            self.commit_music_grouping_candidate(lib_idx);
        }
    }

    /// Commits the candidate's settled catalog when its source revision still
    /// matches the active browse level, anchoring the cursor and artist-header
    /// focus by album identity so a replacement keeps the selection in view.
    fn commit_music_grouping_candidate(&mut self, lib_idx: usize) {
        let lib = &mut self.libs[lib_idx];
        let Some(level) = lib.nav_stack.last_mut() else {
            return;
        };
        let Some(state) = level.music_grouping.as_mut() else {
            return;
        };
        let Some(candidate) = state.candidate.take() else {
            return;
        };
        if candidate.revision != state.revision || candidate.parent_id != level.parent_id {
            return;
        }
        let had_settled = state.settled.is_some();
        let anchor = had_settled.then(|| {
            (
                level.items.get(level.cursor).map(|item| item.id.clone()),
                lib.artist_header_focus
                    .as_ref()
                    .map(|s| s.first_album_id.clone()),
            )
        });
        let mut catalog = build_grouped_album_catalog(&level.items, &candidate.resolved);
        catalog.revision = candidate.revision;
        catalog.parent_id = candidate.parent_id;
        state.settled = Some(catalog);
        let catalog = state.settled.as_ref().expect("catalog just inserted");
        match anchor {
            Some((selected_album_id, header_first_album_id)) => {
                if let Some(id) = selected_album_id {
                    if let Some(&pos) = catalog.id_to_entry.get(&id) {
                        level.cursor = catalog.entries[pos].album_index;
                    } else if let Some(first) = catalog.entries.first() {
                        level.cursor = first.album_index;
                    }
                }
                if header_first_album_id.is_some() {
                    if let Some(selection) = lib.artist_header_focus.clone() {
                        match catalog.group_for_artist(&selection.artist_label) {
                            Some(group) => {
                                let first_album_id = catalog.entries[group.start].album_id.clone();
                                lib.artist_header_focus = Some(ArtistHeaderSelection {
                                    first_album_id,
                                    artist_label: selection.artist_label,
                                });
                            }
                            None => lib.artist_header_focus = None,
                        }
                    }
                }
            }
            None => {
                if let Some(first) = catalog.entries.first() {
                    level.cursor = first.album_index;
                }
            }
        }
    }
}
