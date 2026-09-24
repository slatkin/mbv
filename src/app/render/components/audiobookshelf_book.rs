use crate::app::components::media_list::{
    MediaKind, MediaListRow, MediaListTrailing, MediaSemanticState,
};
use crate::app::state::types::audiobookshelf_browse::AudiobookshelfBookBrowseState;
use crate::app::ui_util::fmt_duration_gutter;

/// Canonical row projection for the book catalog: one selectable `Item` per
/// book in the selected surname bucket, keyed by its stable `library_item_id`.
/// Books carry no in-list letter headings (the surname buckets are a pill row)
/// and no played/active semantic state (matching the legacy book rows). Their
/// total runtime occupies the shared green gutter.
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
            secondary: None,
            trailing: (book.duration_seconds > 0.0)
                .then(|| fmt_duration_gutter(book.duration_seconds as i64))
                .map(MediaListTrailing::Gutter),
            duration: None,
            kind: MediaKind::Collection,
            semantic_state: MediaSemanticState::Ordinary,
        })
        .collect()
}
