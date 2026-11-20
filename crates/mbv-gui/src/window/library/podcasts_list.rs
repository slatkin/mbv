//! The Podcasts list's row model (`add-gui-library-panel` design D12, row
//! 10.1, the port of `lib/library/podcasts-list.ts`): the active pill's
//! episode rows, under their letter headings and the A–Z rail while the list
//! sorts by title, and flat while it sorts by date (gui-flat-list-sort design
//! D8). The list paints fixed-height rows over native scroll and only the rows
//! in view, as the Shows and Home lists do, and the keys act on the row the
//! pointer marks (`gui-pointer-target-interaction` D2). The model is plain data
//! over fixed heights, so the geometry is asserted without a layout, which the
//! headless tests do not run.

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiEntryRowFrame;
use crate::window::library::list::{
    GeometryRow, LIBRARY_ROW_HEIGHT, LetteredRow, SortLetter, build_lettered_rows, filter_match,
};

/// The episode row's height in logical pixels (gui-delineated-rows design D2):
/// the shared top-level row part's height, which paints the episode's
/// two-line title and its date gutter.
pub const PODCASTS_ROW_HEIGHT: f32 = LIBRARY_ROW_HEIGHT;

/// One painted item row of the Podcasts list (design D12). The `id` is the
/// row's stable identity as one opaque string, the pointer marker's value, the
/// each-block's key, and the id the scroll geometry reads.
#[derive(Debug, Clone, PartialEq)]
pub struct PodcastsItemRow {
    /// The row's stable key: the pointer marker's value, the each-block's key,
    /// and the id the scroll geometry reads.
    pub id: String,
    /// The row's top edge from the list's first row.
    pub top: f32,
    /// The row's painted height.
    pub height: f32,
    /// The carried episode the row paints.
    pub row: GuiEntryRowFrame,
}

impl GeometryRow for PodcastsItemRow {
    fn id(&self) -> &str {
        &self.id
    }

    fn top(&self) -> f32 {
        self.top
    }

    fn height(&self) -> f32 {
        self.height
    }
}

impl SortLetter for PodcastsItemRow {
    fn sort_letter(&self) -> Option<char> {
        self.row.sort_letter
    }
}

/// The painted rows of the Podcasts list (design D12; gui-flat-list-sort
/// design D8): the frame's episode rows at one height, in the core's order,
/// each under the letter heading of its rail letter while the sort compares a
/// name — and flat, with no heading and no rail, under a date sort. The filter
/// narrows the rows to the titles that contain its text, without case (design
/// D7's filter rule). Every row carries its top edge and height, so the list
/// paints a window of rows over native scroll.
#[must_use]
pub fn build_podcasts_rows(
    rows: &[GuiEntryRowFrame],
    filter: &str,
) -> Vec<LetteredRow<PodcastsItemRow>> {
    build_lettered_rows(
        rows,
        PODCASTS_ROW_HEIGHT,
        "podcasts-letter",
        |row| filter_match(&row.title, filter),
        |row| row.sort_letter,
        |row, top, height| PodcastsItemRow {
            id: row.id.clone(),
            row: row.clone(),
            top,
            height,
        },
    )
}

/// The rows the memo handed back last, with the cheap key and the interaction
/// facts they were built from. The rows sit behind an `Arc` because the former client
/// semantics the memo preserves are shared state: one scroll step reads the
/// same rows the last one built, and the list repaints only its visible
/// window.
#[derive(Debug, Clone)]
struct PodcastsMemoEntry {
    /// The `libraryList` section revision the rows came from (design D6): a
    /// section's value at one revision is immutable, so equal revisions carry
    /// equal rows and the hit test never compares the union.
    revision: Option<u64>,
    filter: String,
    rows: Arc<[LetteredRow<PodcastsItemRow>]>,
}

/// The Podcasts rows, memoized on the carried rows and the filter (design
/// D9). One slot, because a list paints one content at a time.
#[derive(Clone, Default)]
pub struct PodcastsRowsMemo {
    last: Signal<Option<PodcastsMemoEntry>>,
}

impl std::fmt::Debug for PodcastsRowsMemo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PodcastsRowsMemo").finish_non_exhaustive()
    }
}

impl PodcastsRowsMemo {
    /// The rows `build_podcasts_rows` leaves over the carried rows and the
    /// filter: the ones it built last while the arguments are the same, and a
    /// fresh build otherwise. The rows come keyed on the `libraryList` section
    /// revision they borrowed from (design D6); the caller must pass the slice
    /// the section's current value carries.
    #[must_use]
    pub fn rows(
        &self,
        revision: Option<u64>,
        rows: &[GuiEntryRowFrame],
        filter: &str,
    ) -> Arc<[LetteredRow<PodcastsItemRow>]> {
        // `peek`, never `read`: a caller inside an effect that writes the
        // memo (a filter's scroll clamp) would re-queue itself on the write
        // below otherwise.
        if let Some(entry) = self.last.peek().as_ref()
            && entry.revision == revision
            && entry.filter == filter
        {
            return Arc::clone(&entry.rows);
        }
        let built: Arc<[LetteredRow<PodcastsItemRow>]> = build_podcasts_rows(rows, filter).into();
        let mut last = self.last;
        last.set(Some(PodcastsMemoEntry {
            revision,
            filter: filter.to_owned(),
            rows: Arc::clone(&built),
        }));
        built
    }
}
