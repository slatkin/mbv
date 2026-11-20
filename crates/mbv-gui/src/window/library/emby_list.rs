//! The plain Emby-library list's row model (`add-gui-library-panel` design
//! D12, row 8.1, the port of `lib/library/emby-list.ts`): the plain mode's
//! letter headings over its name-sorted rows, the feed-group mode's flat rows
//! with their date gutter, the rows in view, and the A–Z rail. The list paints
//! fixed-height rows over native scroll and only the rows in view, as the Music
//! and Shows lists do, and the keys act on the row the pointer marks
//! (`gui-pointer-target-interaction` D2). The model is plain data over fixed
//! heights, so the geometry is asserted without a layout.

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiEmbyRowFrame;
use crate::window::library::list::{
    FlatItem, GeometryRow, LETTER_ROW_HEIGHT, LIBRARY_ROW_HEIGHT, RailLetter, build_flat_rows,
    filter_match, letter_rail_by,
};

/// The fields of a carried Emby row the flat feed-group list reads (design
/// D12): the stable id and the title the filter narrows on.
impl FlatItem for GuiEmbyRowFrame {
    fn row_id(&self) -> &str {
        &self.id
    }

    fn row_title(&self) -> &str {
        &self.title
    }
}

/// The letter heading row's height in logical pixels (design D12): the Music
/// list's letter heading, which the plain list reuses.
pub const EMBY_HEADING_ROW_HEIGHT: f32 = LETTER_ROW_HEIGHT;
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

/// One painted row of the plain Emby-library list (design D12, row 8.1).
#[derive(Debug, Clone, PartialEq)]
pub enum EmbyListRow {
    /// A letter heading over a run of rows.
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
    /// An item row.
    Item(EmbyItemRow),
}

impl EmbyListRow {
    /// The row's stable id.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            EmbyListRow::Heading { id, .. } => id,
            EmbyListRow::Item(item) => &item.id,
        }
    }
}

impl GeometryRow for EmbyListRow {
    fn id(&self) -> &str {
        self.id()
    }

    fn pointer_row(&self) -> Option<&str> {
        (!matches!(self, EmbyListRow::Heading { .. })).then_some(self.id())
    }

    fn top(&self) -> f32 {
        match self {
            EmbyListRow::Heading { top, .. } => *top,
            EmbyListRow::Item(item) => item.top,
        }
    }

    fn height(&self) -> f32 {
        match self {
            EmbyListRow::Heading { height, .. } => *height,
            EmbyListRow::Item(item) => item.height,
        }
    }
}

/// The plain mode's rows: each row under the letter heading of its rail
/// letter, one heading per run of the same letter.
fn heading_emby_rows(rows: &[&GuiEmbyRowFrame]) -> Vec<EmbyListRow> {
    let mut built: Vec<EmbyListRow> = Vec::new();
    let mut top = 0.0_f32;
    let mut heading: Option<char> = None;
    for row in rows {
        let letter = row.sort_letter.unwrap_or('#');
        if heading != Some(letter) {
            heading = Some(letter);
            built.push(EmbyListRow::Heading {
                id: format!("emby-letter:{letter}"),
                letter,
                top,
                height: EMBY_HEADING_ROW_HEIGHT,
            });
            top += EMBY_HEADING_ROW_HEIGHT;
        }
        built.push(EmbyListRow::Item(EmbyItemRow {
            id: row.id.clone(),
            row: (*row).clone(),
            top,
            height: EMBY_ROW_HEIGHT,
        }));
        top += EMBY_ROW_HEIGHT;
    }
    built
}

/// The painted rows of the list (design D12, row 8.1): in plain mode the
/// name-sorted rows, each under the letter heading of its rail letter — the
/// Music list's heading rule, one heading per run of the same letter — and in
/// feed-group mode the flat rows in the server's date order with no heading at
/// all, because that list carries no rail. The filter narrows the rows to the
/// titles that contain its text, without case, and a heading paints only above
/// a kept row (design D7's filter rule, the rail's keyboard path). Every row
/// carries its top edge and height, so the list paints a window of rows over
/// native scroll.
#[must_use]
pub fn build_emby_rows(
    rows: &[GuiEmbyRowFrame],
    feed_group: bool,
    filter: &str,
) -> Vec<EmbyListRow> {
    if feed_group {
        build_flat_rows(rows, EMBY_ROW_HEIGHT, filter)
            .into_iter()
            .map(|flat| {
                EmbyListRow::Item(EmbyItemRow {
                    id: flat.id,
                    row: flat.row.clone(),
                    top: flat.top,
                    height: flat.height,
                })
            })
            .collect()
    } else {
        let kept: Vec<&GuiEmbyRowFrame> = rows
            .iter()
            .filter(|row| filter_match(&row.title, filter))
            .collect();
        heading_emby_rows(&kept)
    }
}

/// The A–Z rail over the painted rows (row 8.1, design D7): each letter with
/// the first row under it, so a press scrolls there. A letter no painted row
/// carries is disabled, as the Music rail's is. The feed-group mode paints no
/// rail, so its rows carry no letter and the rail stays disabled.
#[must_use]
pub fn emby_letter_rail(rows: &[EmbyListRow]) -> Vec<RailLetter> {
    letter_rail_by(rows, |row| match row {
        EmbyListRow::Item(item) => item.row.sort_letter,
        EmbyListRow::Heading { .. } => None,
    })
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
    feed_group: bool,
    filter: String,
    rows: Arc<[EmbyListRow]>,
}

/// The Emby rows, memoized on the carried rows, the feed-group flag, and the
/// filter (design D9). One slot, because a list paints one content at a time.
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
        feed_group: bool,
        rows: &[GuiEmbyRowFrame],
        filter: &str,
    ) -> Arc<[EmbyListRow]> {
        // `peek`, never `read`: a caller inside an effect that writes the
        // memo (a filter's scroll clamp) would re-queue itself on the write
        // below otherwise.
        if let Some(entry) = self.last.peek().as_ref()
            && entry.revision == revision
            && entry.feed_group == feed_group
            && entry.filter == filter
        {
            return Arc::clone(&entry.rows);
        }
        let built: Arc<[EmbyListRow]> = build_emby_rows(rows, feed_group, filter).into();
        let mut last = self.last;
        last.set(Some(EmbyMemoEntry {
            revision,
            feed_group,
            filter: filter.to_owned(),
            rows: Arc::clone(&built),
        }));
        built
    }
}

#[cfg(test)]
mod tests;
