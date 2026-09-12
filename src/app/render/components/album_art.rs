/// The album cover's cache key (design D5): the shared Square Music artwork
/// policy uses the same `{album_id}:P` key the wide hero projection and the
/// idle pre-warm issue, so painting and fetching never disagree.
pub(in crate::app::render) fn inline_album_art_cache_key(album_id: &str) -> String {
    format!("{album_id}:P")
}
