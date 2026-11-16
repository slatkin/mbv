//! The Feeds list's row model (`add-gui-library-panel` design D12, row
//! 11.1, the port of `lib/library/feeds-list.ts`): the active pill's flat
//! entry rows — no letter headings, because the list is sorted by date, and
//! so no letter rail. The list paints fixed-height rows over native scroll
//! and only the rows in view, as the Shows and Home lists do, and the keys act
//! on the row the pointer marks (`gui-pointer-target-interaction` D2). The
//! model is plain data over fixed heights, so the geometry is asserted without
//! a layout, which jsdom does not have.
//!
//! The carried rows are the shared entry-list ones, so the [`FlatItem`] impl
//! they read lives in [`super::podcasts_list`] and this module names no impl
//! of its own.

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiEntryRowFrame;
use crate::window::library::list::{GeometryRow, build_flat_rows};

/// The entry row's height in logical pixels (design D12): the room a
/// two-line entry title and the date gutter paint into, as a Shows flat row.
pub const FEEDS_ROW_HEIGHT: f32 = 48.0;

/// One painted row of the Feeds list (design D12). The list is one flat
/// entry list in core order — newest first with undated entries last — so
/// every painted row is an item row: the `id` is the row's stable identity as
/// one opaque string, the pointer marker's value, the each-block's key, and the
/// id the scroll geometry reads. There is no heading and no rail to step over.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedsItemRow {
    /// The row's stable key: the pointer marker's value, the each-block's key,
    /// and the id the scroll geometry reads.
    pub id: String,
    /// The row's top edge from the list's first row.
    pub top: f32,
    /// The row's painted height.
    pub height: f32,
    /// The carried entry the row paints.
    pub row: GuiEntryRowFrame,
}

impl GeometryRow for FeedsItemRow {
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

/// The painted rows of the Feeds list (design D12): the frame's entry rows at
/// one height, with no heading and no letter, in the core's order — the
/// Latest pill's merged newest-first order, or a subscription's own entries.
/// The filter narrows the rows to the titles that contain its text, without
/// case (design D7's filter rule). Every row carries its top edge and height,
/// so the list paints a window of rows over native scroll.
#[must_use]
pub fn build_feeds_rows(rows: &[GuiEntryRowFrame], filter: &str) -> Vec<FeedsItemRow> {
    build_flat_rows(rows, FEEDS_ROW_HEIGHT, filter)
        .into_iter()
        .map(|flat| FeedsItemRow {
            id: flat.id,
            row: flat.row.clone(),
            top: flat.top,
            height: flat.height,
        })
        .collect()
}

/// The rows the memo handed back last, with the cheap key and the interaction
/// facts they were built from. The rows sit behind an `Arc` because the former client
/// semantics the memo preserves are shared state: one scroll step reads the
/// same rows the last one built, and the list repaints only its visible
/// window.
#[derive(Debug, Clone)]
struct FeedsMemoEntry {
    /// The `libraryList` section revision the rows came from (design D6): a
    /// section's value at one revision is immutable, so equal revisions carry
    /// equal rows and the hit test never compares the union.
    revision: Option<u64>,
    filter: String,
    rows: Arc<[FeedsItemRow]>,
}

/// The Feeds rows, memoized on the carried rows and the filter (design D9).
/// One slot, because a list paints one content at a time.
#[derive(Clone, Default)]
pub struct FeedsRowsMemo {
    last: Signal<Option<FeedsMemoEntry>>,
}

impl std::fmt::Debug for FeedsRowsMemo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FeedsRowsMemo").finish_non_exhaustive()
    }
}

impl FeedsRowsMemo {
    /// The rows `build_feeds_rows` leaves over the carried rows and the
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
    ) -> Arc<[FeedsItemRow]> {
        // `peek`, never `read`: a caller inside an effect that writes the
        // memo (a filter's scroll clamp) would re-queue itself on the write
        // below otherwise.
        if let Some(entry) = self.last.peek().as_ref()
            && entry.revision == revision
            && entry.filter == filter
        {
            return Arc::clone(&entry.rows);
        }
        let built: Arc<[FeedsItemRow]> = build_feeds_rows(rows, filter).into();
        let mut last = self.last;
        last.set(Some(FeedsMemoEntry {
            revision,
            filter: filter.to_owned(),
            rows: Arc::clone(&built),
        }));
        built
    }
}
