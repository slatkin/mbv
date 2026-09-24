//! Audiobookshelf browse tabs: the one-shot browse-kind dispatch plus the
//! podcast and book browse states, which live in sibling files by tab.

pub(in crate::app) mod books;
mod podcast;

pub(in crate::app) use books::{AudiobookshelfBookBrowseState, BookRow};
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
mod tests;
