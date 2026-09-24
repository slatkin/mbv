//! Audiobookshelf browse tabs: the one-shot browse-kind dispatch plus the
//! podcast and book browse states, which live in sibling files by tab.

#[path = "audiobookshelf_browse_books.rs"]
mod books;
#[path = "audiobookshelf_browse_podcast.rs"]
mod podcast;

pub(in crate::app) use books::{
    build_surname_buckets, AudiobookshelfBookBrowseState, BookRow, SurnameBucket,
};
pub(in crate::app) use podcast::{
    podcast_display_rows, AudiobookshelfBrowseState, AudiobookshelfEpisodeFilter, PillSelection,
    PodcastDisplayRow,
};

/// The resolved browse kind for an Audiobookshelf library tab, derived once
/// from the library's `media_type` at the browse-dispatch seam. Downstream
/// renderers/input handlers branch on this value and never re-read
/// `media_type` per action (service-browse-dispatch capability).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum AudiobookshelfBrowseKind {
    Podcast,
    Book,
}

impl AudiobookshelfBrowseKind {
    /// `media_type` values other than `"book"` resolve to Podcast, matching
    /// the pre-book behavior for the only two media types ABS exposes.
    pub(in crate::app) fn from_media_type(media_type: &str) -> Self {
        if media_type == "book" {
            Self::Book
        } else {
            Self::Podcast
        }
    }
}
#[cfg(test)]
#[path = "audiobookshelf_browse_tests.rs"]
mod tests;
