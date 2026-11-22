use crate::sort_filter::{parse_album_folder_name, strip_article};
use crate::ui_util::natural_sort_key;
use mbv_emby_model::EmbyItem;
use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;
use std::time::Instant;

/// Monotonic counter identifying a loaded source snapshot for a music album
/// browse level. Bumped whenever the level's items change.
pub type SourceRevision = u64;

/// A resolving snapshot of a music album level. Records which album IDs
/// still need a terminal artist identity and which have already resolved.
#[derive(Clone, Debug)]
pub struct MusicGroupCandidate {
    pub revision: SourceRevision,
    pub parent_id: String,
    pub unresolved: HashSet<String>,
    pub resolved: HashMap<String, String>,
    pub created_at: Instant,
}

#[derive(Clone, Debug)]
pub struct GroupedAlbumEntry {
    pub album_index: usize,
    pub album_id: String,
    pub artist: String,
    /// Stable artist identity for this entry's group: the `ArtistItems` ID
    /// resolved against the settled display artist, or a deterministic
    /// fallback key when no payload identity applies. Never derived from
    /// display position.
    // Carried by task 1.3; first read by the Grouped Music tree owner
    // (task 2.1 of add-grouped-music-tree-browser).
    pub artist_key: ArtistKey,
    pub sort_key: String,
    pub year: String,
    pub name: String,
}

/// Stable identity of one settled artist group. `Service` carries the
/// resolved `ArtistItems` item ID — the only artist ID space mbv uses;
/// `/Artists` listing IDs never enter here. `Fallback` is the deterministic
/// grouping key for every absent, unmatched, or ambiguous `ArtistItems`
/// case: the settled display artist itself, so albums without valid
/// payload identity stay grouped exactly as they are today while equal
/// display names with distinct valid IDs stay separate.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ArtistKey {
    Service(String),
    Fallback(String),
}

/// A settled, source-derived grouping for one music album browse level:
/// resolved artist identities, display metadata, precomputed sort keys,
/// sorted album order, group boundaries, and identity lookups.
#[derive(Clone, Debug)]
pub struct GroupedAlbumCatalog {
    pub revision: SourceRevision,
    pub parent_id: String,
    /// Entries sorted by artist, displayed release year, then album name;
    /// `entries[i].album_index` indexes the raw `items` slice the catalog was
    /// built from.
    pub entries: Vec<GroupedAlbumEntry>,
    /// raw album index -> position in `entries`.
    pub index_to_entry: HashMap<usize, usize>,
    /// album id -> position in `entries`.
    pub id_to_entry: HashMap<String, usize>,
}

/// Per-music-album-level grouping lifecycle state.
#[derive(Clone, Debug)]
pub struct MusicGroupingState {
    pub revision: SourceRevision,
    pub candidate: Option<MusicGroupCandidate>,
    pub settled: Option<GroupedAlbumCatalog>,
}

impl Default for MusicGroupingState {
    fn default() -> Self {
        Self::new()
    }
}

impl MusicGroupingState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            revision: 0,
            candidate: None,
            settled: None,
        }
    }
}

/// Synchronous artist resolution chain shared by catalog building and the
/// pre-settle render fallback: `item.artist` -> resolved lookup (cache or
/// fetch result) -> folder-name parse -> literal "Unknown Artist". Never
/// schedules artist resolution work.
#[must_use]
pub fn derive_album_artist(item: &EmbyItem, resolved: Option<&str>) -> String {
    if !item.artist.is_empty() {
        return item.artist.clone();
    }
    if let Some(artist) = resolved
        && !artist.is_empty()
    {
        return artist.to_string();
    }
    if let Some((artist, _, _)) = parse_album_folder_name(&item.name) {
        return artist;
    }
    "Unknown Artist".to_string()
}

/// Display `(year, album_name)` for an album item, mirroring the current
/// renderer's rule: Emby-provided artist metadata selects the tagged year and
/// display name, otherwise a folder-name parse wins.
#[must_use]
pub fn derive_album_display_name(item: &EmbyItem) -> (String, String) {
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

/// Extracts the four-digit year shown on an album row from its year text.
///
/// The first four ASCII digits win, whatever hidden or odd characters sit
/// among them, so a tag quirk cannot turn a dated album into an undated one.
fn parse_displayed_year(year: &str) -> Option<u32> {
    let mut value = 0_u32;
    let mut digits = 0_u8;
    for ch in year.chars().filter(char::is_ascii_digit).take(4) {
        value = value * 10 + ch.to_digit(10)?;
        digits += 1;
    }
    (digits == 4 && value > 0).then_some(value)
}

/// One grouped album's full display order key; undated albums sort last.
fn group_album_sort_key(artist: &str, year: &str, name: &str) -> (String, (bool, u32), String) {
    let year = parse_displayed_year(year);
    (
        natural_sort_key(strip_article(artist)),
        (year.is_none(), year.unwrap_or_default()),
        natural_sort_key(name),
    )
}

/// Orders grouped album facts by artist, displayed release year, then album name.
///
/// The input triples are `(artist, year, album_name)`. The year is the same
/// text the row paints, so a provider order or tag quirk cannot disagree with
/// the displayed year. Undated albums follow dated ones; equal years fall
/// back to the album name, then to the input order.
#[must_use]
pub fn sorted_group_album_order(album_info: &[(String, String, String)]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..album_info.len()).collect();
    order.sort_by_key(|&index| {
        let (artist, year, name) = &album_info[index];
        group_album_sort_key(artist, year, name)
    });
    order
}

/// Resolves the display artist for an album item in the grouped music views
/// synchronously, given the album-artist cache. Mirrors
/// `App::resolve_group_album_artist` without borrowing `App`.
fn resolve_group_album_artist<S: BuildHasher>(
    album_artist_cache: &HashMap<String, String, S>,
    item: &EmbyItem,
) -> String {
    derive_album_artist(item, album_artist_cache.get(&item.id).map(String::as_str))
}

/// One album's display info plus its stable artist identity: the settled
/// catalog's resolved entry when available, otherwise the deterministic
/// fallback over the synchronous best-effort artist chain (design D2 of
/// `add-grouped-music-tree-browser`).
fn resolve_group_album<S: BuildHasher>(
    album_artist_cache: &HashMap<String, String, S>,
    albums: &[EmbyItem],
    catalog: Option<&GroupedAlbumCatalog>,
    index: usize,
) -> (String, String, String, ArtistKey) {
    if let Some(cat) = catalog {
        let pos = cat.index_to_entry.get(&index).copied().unwrap_or(0);
        let entry = &cat.entries[pos];
        (
            entry.artist.clone(),
            entry.year.clone(),
            entry.name.clone(),
            entry.artist_key.clone(),
        )
    } else {
        let item = &albums[index];
        let artist = resolve_group_album_artist(album_artist_cache, item);
        let (year, name) = derive_album_display_name(item);
        let key = ArtistKey::Fallback(artist.clone());
        (artist, year, name, key)
    }
}

/// Builds the `(artist, year, album_name)` display info and stable artist
/// identity for every album (design D2 of `add-grouped-music-tree-browser`),
/// consuming the settled catalog when available (no artist derivation) and
/// falling back to a synchronous best-effort chain otherwise.
#[must_use]
pub fn group_album_plan<S: BuildHasher>(
    album_artist_cache: &HashMap<String, String, S>,
    albums: &[EmbyItem],
    catalog: Option<&GroupedAlbumCatalog>,
) -> (Vec<(String, String, String)>, Vec<ArtistKey>) {
    let mut info = Vec::with_capacity(albums.len());
    let mut keys = Vec::with_capacity(albums.len());
    for i in 0..albums.len() {
        let (artist, year, name, key) = resolve_group_album(album_artist_cache, albums, catalog, i);
        info.push((artist, year, name));
        keys.push(key);
    }
    (info, keys)
}

/// Resolves an album's stable artist grouping key from its payload
/// `ArtistItems` pairs against the settled display artist. Pure: no app
/// state, no network.
fn resolve_artist_key(item: &EmbyItem, display_artist: &str) -> ArtistKey {
    match item.matched_artist_item_id(display_artist) {
        Some(id) => ArtistKey::Service(id.to_string()),
        None => ArtistKey::Fallback(display_artist.to_string()),
    }
}

/// Builds the settled grouped catalog for a source snapshot from the raw
/// items and a resolved artist lookup. Pure: no app state, no network.
pub fn build_grouped_album_catalog<S: std::hash::BuildHasher>(
    items: &[EmbyItem],
    resolved: &HashMap<String, String, S>,
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
            artist_key: resolve_artist_key(item, &artist),
            sort_key,
            year,
            name,
        });
    }
    entries.sort_by_key(|entry| group_album_sort_key(&entry.artist, &entry.year, &entry.name));

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
        index_to_entry,
        id_to_entry,
    }
}
