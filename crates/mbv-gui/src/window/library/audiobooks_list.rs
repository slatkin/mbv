//! The Audiobooks list's row model (`add-gui-library-panel` design D12, row
//! 9.1, the port of `lib/library/audiobooks-list.ts`): the catalog's book rows
//! under the letter headings of their author sort key, the rows in view, and the
//! A–Z letter rail. There is no pill row and no second mode: the destination is
//! one flat book list, and the letter rail replaces the TUI's surname buckets as
//! it does for Shows and the plain Emby libraries. The list paints fixed-height
//! rows over native scroll and only the rows in view, as the Music and Emby
//! lists do, and the keys act on the row the pointer marks
//! (`gui-pointer-target-interaction` D2). The model is plain data over fixed
//! heights, so the geometry is asserted without a layout.

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiAudiobooksRowFrame;
use crate::window::library::list::{
    GeometryRow, LETTER_ROW_HEIGHT, LIBRARY_ROW_HEIGHT, RailLetter, filter_match, letter_rail_by,
};

/// The letter heading row's height in logical pixels (design D12, row 9.1):
/// the Music list's letter heading, which the book list reuses.
pub const AUIB_HEADING_ROW_HEIGHT: f32 = LETTER_ROW_HEIGHT;
/// The book row's height in logical pixels: the shared top-level row part's
/// height (gui-delineated-rows design D2).
pub const AUIB_ROW_HEIGHT: f32 = LIBRARY_ROW_HEIGHT;

/// One painted item row: a book (row 9.1). A heading names no book.
#[derive(Debug, Clone, PartialEq)]
pub struct AudiobooksItemRow {
    /// The row's stable key: the pointer marker's value, the each-block's key,
    /// and the id the scroll geometry reads.
    pub id: String,
    /// The row's top edge from the list's first row.
    pub top: f32,
    /// The row's painted height.
    pub height: f32,
    /// The carried book the row paints.
    pub row: GuiAudiobooksRowFrame,
}

/// One painted row of the Audiobooks list (design D12, row 9.1). A heading is
/// structural; an item row is a book.
#[derive(Debug, Clone, PartialEq)]
pub enum AudiobooksListRow {
    /// A letter heading over a run of author sort keys.
    Heading {
        /// The row's stable key.
        id: String,
        /// The row's top edge from the list's first row.
        top: f32,
        /// The row's painted height.
        height: f32,
        /// The heading's rail letter.
        letter: char,
    },
    /// A book row.
    Item(AudiobooksItemRow),
}

impl AudiobooksListRow {
    /// The row's stable id.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            AudiobooksListRow::Heading { id, .. } => id,
            AudiobooksListRow::Item(item) => &item.id,
        }
    }
}

impl GeometryRow for AudiobooksListRow {
    fn id(&self) -> &str {
        AudiobooksListRow::id(self)
    }

    fn pointer_row(&self) -> Option<&str> {
        (!matches!(self, AudiobooksListRow::Heading { .. })).then_some(AudiobooksListRow::id(self))
    }

    fn top(&self) -> f32 {
        match self {
            AudiobooksListRow::Heading { top, .. } => *top,
            AudiobooksListRow::Item(item) => item.top,
        }
    }

    fn height(&self) -> f32 {
        match self {
            AudiobooksListRow::Heading { height, .. } => *height,
            AudiobooksListRow::Item(item) => item.height,
        }
    }
}

/// The painted rows of the book list (design D12, row 9.1): the catalog's book
/// rows in the TUI's sort — author sort key, then title — each under the letter
/// heading of its rail letter, one heading per run of the same letter. The
/// filter narrows the rows to the titles that contain its text, without case,
/// and a heading paints only above a kept row (design D7's filter rule, the
/// rail's keyboard path). Every row carries its top edge and height, so the
/// list paints a window of rows over native scroll.
#[must_use]
pub fn build_audiobooks_rows(
    rows: &[GuiAudiobooksRowFrame],
    filter: &str,
) -> Vec<AudiobooksListRow> {
    let mut built: Vec<AudiobooksListRow> = Vec::new();
    let mut top = 0.0_f32;
    let mut heading: Option<char> = None;
    for row in rows
        .iter()
        .filter(|candidate| filter_match(&candidate.title, filter))
    {
        if let Some(letter) = row.sort_letter
            && heading != Some(letter)
        {
            heading = Some(letter);
            built.push(AudiobooksListRow::Heading {
                id: format!("abs-letter:{letter}"),
                letter,
                top,
                height: AUIB_HEADING_ROW_HEIGHT,
            });
            top += AUIB_HEADING_ROW_HEIGHT;
        }
        built.push(AudiobooksListRow::Item(AudiobooksItemRow {
            id: row.id.clone(),
            row: row.clone(),
            top,
            height: AUIB_ROW_HEIGHT,
        }));
        top += AUIB_ROW_HEIGHT;
    }
    built
}

/// The A–Z rail over the painted rows (row 9.1, design D7): each letter with
/// the first book under it, so a press scrolls there. A letter no painted row
/// carries is disabled, as the Music rail's is.
#[must_use]
pub fn audiobooks_letter_rail(rows: &[AudiobooksListRow]) -> Vec<RailLetter> {
    letter_rail_by(rows, |row| match row {
        AudiobooksListRow::Item(item) => item.row.sort_letter,
        AudiobooksListRow::Heading { .. } => None,
    })
}

/// The rows the memo handed back last, with the cheap key and the interaction
/// facts they were built from. The rows sit behind an `Arc` because the former client
/// semantics the memo preserves are shared state: one scroll step reads the
/// same rows the last one built, and the list repaints only its visible
/// window.
#[derive(Debug, Clone)]
struct AudiobooksMemoEntry {
    /// The `libraryList` section revision the rows came from (design D6): a
    /// section's value at one revision is immutable, so equal revisions carry
    /// equal rows and the hit test never compares the union.
    revision: Option<u64>,
    filter: String,
    rows: Arc<[AudiobooksListRow]>,
}

/// The Audiobooks rows, memoized on the carried rows and the filter (design
/// D9). One slot, because a list paints one content at a time.
#[derive(Clone, Default)]
pub struct AudiobooksRowsMemo {
    last: Signal<Option<AudiobooksMemoEntry>>,
}

impl std::fmt::Debug for AudiobooksRowsMemo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudiobooksRowsMemo").finish_non_exhaustive()
    }
}

impl AudiobooksRowsMemo {
    /// The rows `build_audiobooks_rows` leaves over the carried rows and the
    /// filter: the ones it built last while the arguments are the same, and a
    /// fresh build otherwise. The rows come keyed on the `libraryList` section
    /// revision they borrowed from (design D6); the caller must pass the slice
    /// the section's current value carries.
    #[must_use]
    pub fn rows(
        &self,
        revision: Option<u64>,
        rows: &[GuiAudiobooksRowFrame],
        filter: &str,
    ) -> Arc<[AudiobooksListRow]> {
        // `peek`, never `read`: a caller inside an effect that writes the
        // memo (a filter's scroll clamp) would re-queue itself on the write
        // below otherwise.
        if let Some(entry) = self.last.peek().as_ref()
            && entry.revision == revision
            && entry.filter == filter
        {
            return Arc::clone(&entry.rows);
        }
        let built: Arc<[AudiobooksListRow]> = build_audiobooks_rows(rows, filter).into();
        let mut last = self.last;
        last.set(Some(AudiobooksMemoEntry {
            revision,
            filter: filter.to_owned(),
            rows: Arc::clone(&built),
        }));
        built
    }
}

#[cfg(test)]
mod tests;
