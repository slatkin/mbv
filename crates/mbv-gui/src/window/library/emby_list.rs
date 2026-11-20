//! The plain Emby-library list's row model (`add-gui-library-panel` design
//! D12, row 8.1, the port of `lib/library/emby-list.ts`): the name-sorted
//! rows under their letter headings, the feed-group rows with their date
//! gutter, the rows in view, and the A–Z rail. The lettered rows come from
//! the shared builder (gui-flat-list-sort design D8), so a name sort paints
//! headings and a date sort paints none. The list paints fixed-height rows
//! over native scroll and only the rows in view, as the Music and Shows lists
//! do, and the keys act on the row the pointer marks
//! (`gui-pointer-target-interaction` D2). The model is plain data over fixed
//! heights, so the geometry is asserted without a layout.

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiEmbyRowFrame;
use crate::window::library::list::{
    GeometryRow, LIBRARY_ROW_HEIGHT, LetteredRow, SortLetter, build_lettered_rows, filter_match,
};

/// The item row's height in logical pixels: the shared top-level row part's
/// height (gui-delineated-rows design D2), so the virtual-list geometry and
/// the painted rows agree. Both modes paint at this one height, so a mode
/// switch changes the rows and never their geometry.
pub const EMBY_ROW_HEIGHT: f32 = LIBRARY_ROW_HEIGHT;

/// One painted row the keys may act on (row 8.1): every item row, a folder and
/// a boxset included, because the folder-play rule plays a collection row. A
/// heading is structural.
#[derive(Debug, Clone, PartialEq)]
pub struct EmbyItemRow {
    /// The row's stable key: the pointer marker's value, the each-block's key,
    /// and the id the scroll geometry reads.
    pub id: String,
    /// The row's top edge from the list's first row.
    pub top: f32,
    /// The row's painted height.
    pub height: f32,
    /// The carried row the item paints.
    pub row: GuiEmbyRowFrame,
}

impl GeometryRow for EmbyItemRow {
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

impl SortLetter for EmbyItemRow {
    fn sort_letter(&self) -> Option<char> {
        self.row.sort_letter
    }
}

/// The painted rows of the list (design D12, row 8.1; gui-flat-list-sort
/// design D8): the carried rows narrowed by the filter's text, each under the
/// letter heading of its rail letter — one heading per run of the same
/// letter. A date sort's rows carry no letter, so they paint flat, with no
/// heading and no rail. Every row carries its top edge and height, so the list
/// paints a window of rows over native scroll.
#[must_use]
pub fn build_emby_rows(rows: &[GuiEmbyRowFrame], filter: &str) -> Vec<LetteredRow<EmbyItemRow>> {
    build_lettered_rows(
        rows,
        EMBY_ROW_HEIGHT,
        "emby-letter",
        |row| filter_match(&row.title, filter),
        |row| row.sort_letter,
        |row, top, height| EmbyItemRow {
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
struct EmbyMemoEntry {
    /// The `libraryList` section revision the rows came from (design D6): a
    /// section's value at one revision is immutable, so equal revisions carry
    /// equal rows and the hit test never compares the union.
    revision: Option<u64>,
    filter: String,
    rows: Arc<[LetteredRow<EmbyItemRow>]>,
}

/// The Emby rows, memoized on the carried rows and the filter (design D9). One
/// slot, because a list paints one content at a time.
#[derive(Clone, Default)]
pub struct EmbyRowsMemo {
    last: Signal<Option<EmbyMemoEntry>>,
}

impl std::fmt::Debug for EmbyRowsMemo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbyRowsMemo").finish_non_exhaustive()
    }
}

impl EmbyRowsMemo {
    /// The rows `build_emby_rows` leaves over the carried rows and the
    /// interaction facts: the ones it built last while the arguments are the
    /// same, and a fresh build otherwise. The rows come keyed on the
    /// `libraryList` section revision they borrowed from (design D6); the
    /// caller must pass the slice the section's current value carries.
    #[must_use]
    pub fn rows(
        &self,
        revision: Option<u64>,
        rows: &[GuiEmbyRowFrame],
        filter: &str,
    ) -> Arc<[LetteredRow<EmbyItemRow>]> {
        // `peek`, never `read`: a caller inside an effect that writes the
        // memo (a filter's scroll clamp) would re-queue itself on the write
        // below otherwise.
        if let Some(entry) = self.last.peek().as_ref()
            && entry.revision == revision
            && entry.filter == filter
        {
            return Arc::clone(&entry.rows);
        }
        let built: Arc<[LetteredRow<EmbyItemRow>]> = build_emby_rows(rows, filter).into();
        let mut last = self.last;
        last.set(Some(EmbyMemoEntry {
            revision,
            filter: filter.to_owned(),
            rows: Arc::clone(&built),
        }));
        built
    }
}

#[cfg(test)]
mod tests;
