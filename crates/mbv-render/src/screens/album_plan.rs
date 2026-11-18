use mbv_ui_model::music_grouping::{
    ArtistKey, GroupedAlbumCatalog, derive_album_artist, derive_album_display_name,
};
use std::collections::HashMap;
use std::hash::BuildHasher;

/// Resolves the display artist for an album item in the grouped music views
/// synchronously, given the album-artist cache. Mirrors
/// `App::resolve_group_album_artist` without borrowing `App`.
fn resolve_group_album_artist<S: BuildHasher>(
    album_artist_cache: &HashMap<String, String, S>,
    item: &mbv_emby_model::EmbyItem,
) -> String {
    derive_album_artist(item, album_artist_cache.get(&item.id).map(String::as_str))
}

/// One album's display info plus its stable artist identity: the settled
/// catalog's resolved entry when available, otherwise the deterministic
/// fallback over the synchronous best-effort artist chain (design D2 of
/// `add-grouped-music-tree-browser`).
fn resolve_group_album<S: BuildHasher>(
    album_artist_cache: &HashMap<String, String, S>,
    albums: &[mbv_emby_model::EmbyItem],
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
    albums: &[mbv_emby_model::EmbyItem],
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
