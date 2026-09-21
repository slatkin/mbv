use crate::app::music_grouping::{
    derive_album_artist, derive_album_display_name, ArtistKey, GroupedAlbumCatalog,
};
use crate::app::render::{natural_sort_key, strip_article};
use std::collections::HashMap;

/// Sorted album display order for a set of `(artist, year, name)` info
/// triples: indices ordered by the artist's natural sort key (articles
/// stripped). Mirrors the catalog builder's sort so the fallback path
/// (no settled catalog yet) matches the settled ordering exactly.
pub(crate) fn sorted_group_album_order(album_info: &[(String, String, String)]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..album_info.len()).collect();
    order.sort_by_key(|&i| natural_sort_key(strip_article(&album_info[i].0)));
    order
}

/// Resolves the display artist for an album item in the grouped music views
/// synchronously, given the album-artist cache. Mirrors
/// `App::resolve_group_album_artist` without borrowing `App`.
fn resolve_group_album_artist(
    album_artist_cache: &HashMap<String, String>,
    item: &mbv_core::api::EmbyItem,
) -> String {
    derive_album_artist(item, album_artist_cache.get(&item.id).map(String::as_str))
}

/// One album's display info plus its stable artist identity: the settled
/// catalog's resolved entry when available, otherwise the deterministic
/// fallback over the synchronous best-effort artist chain (design D2 of
/// `add-grouped-music-tree-browser`).
fn resolve_group_album(
    album_artist_cache: &HashMap<String, String>,
    albums: &[mbv_core::api::EmbyItem],
    catalog: Option<&GroupedAlbumCatalog>,
    index: usize,
) -> (String, String, String, ArtistKey) {
    match catalog {
        Some(cat) => {
            let pos = cat.index_to_entry.get(&index).copied().unwrap_or(0);
            let entry = &cat.entries[pos];
            (
                entry.artist.clone(),
                entry.year.clone(),
                entry.name.clone(),
                entry.artist_key.clone(),
            )
        }
        None => {
            let item = &albums[index];
            let artist = resolve_group_album_artist(album_artist_cache, item);
            let (year, name) = derive_album_display_name(item);
            let key = ArtistKey::Fallback(artist.clone());
            (artist, year, name, key)
        }
    }
}

/// Builds the stable artist identity for every album (parallel to
/// `group_album_info`, design D2 of `add-grouped-music-tree-browser`). The
/// Grouped Music tree owner is the only consumer.
pub(in crate::app::render) fn group_album_artist_keys(
    album_artist_cache: &HashMap<String, String>,
    albums: &[mbv_core::api::EmbyItem],
    catalog: Option<&GroupedAlbumCatalog>,
) -> Vec<ArtistKey> {
    (0..albums.len())
        .map(|i| resolve_group_album(album_artist_cache, albums, catalog, i).3)
        .collect()
}

/// Builds the `(artist, year, album_name)` display info for every album,
/// consuming the settled catalog when available (no artist derivation) and
/// falling back to a synchronous best-effort chain otherwise.
pub(in crate::app::render) fn group_album_info(
    album_artist_cache: &HashMap<String, String>,
    albums: &[mbv_core::api::EmbyItem],
    catalog: Option<&GroupedAlbumCatalog>,
) -> Vec<(String, String, String)> {
    (0..albums.len())
        .map(|i| {
            let (artist, year, name, _) =
                resolve_group_album(album_artist_cache, albums, catalog, i);
            (artist, year, name)
        })
        .collect()
}
