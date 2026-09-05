use crate::app::music_grouping::{
    derive_album_artist, derive_album_display_name, GroupedAlbumCatalog,
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

/// Builds the `(artist, year, album_name)` display info for every album,
/// consuming the settled catalog when available (no artist derivation) and
/// falling back to a synchronous best-effort chain otherwise.
pub(in crate::app::render) fn group_album_info(
    album_artist_cache: &HashMap<String, String>,
    albums: &[mbv_core::api::EmbyItem],
    catalog: Option<&GroupedAlbumCatalog>,
) -> Vec<(String, String, String)> {
    match catalog {
        Some(cat) => (0..albums.len())
            .map(|i| {
                let pos = cat.index_to_entry.get(&i).copied().unwrap_or(0);
                let entry = &cat.entries[pos];
                (entry.artist.clone(), entry.year.clone(), entry.name.clone())
            })
            .collect(),
        None => albums
            .iter()
            .map(|item| {
                let artist = resolve_group_album_artist(album_artist_cache, item);
                let (year, name) = derive_album_display_name(item);
                (artist, year, name)
            })
            .collect(),
    }
}
