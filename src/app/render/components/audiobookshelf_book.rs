use crate::app::components::media_list::{MediaKind, MediaListRow, MediaSemanticState};
use crate::app::types_audiobookshelf_browse::AudiobookshelfBookBrowseState;

/// Canonical row projection for the book catalog: one selectable `Item` per
/// book in the selected surname bucket, keyed by its stable `library_item_id`.
/// Books carry no in-list letter headings (the surname buckets are a pill row)
/// and no played/active semantic state (matching the legacy book rows).
pub(in crate::app) fn book_rows(
    state: &AudiobookshelfBookBrowseState,
    selected_bucket: usize,
) -> Vec<MediaListRow<String>> {
    let Some(bucket) = state.buckets.get(selected_bucket).copied() else {
        return Vec::new();
    };
    state
        .books
        .get(bucket.start..bucket.end)
        .unwrap_or_default()
        .iter()
        .map(|book| MediaListRow::Item {
            target: book.library_item_id.clone(),
            primary: book.title.clone(),
            trailing: None,
            duration: None,
            kind: MediaKind::Collection,
            semantic_state: MediaSemanticState::Ordinary,
        })
        .collect()
}
