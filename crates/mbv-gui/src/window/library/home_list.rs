//! The Home list's row model (`add-gui-library-panel` design D12, the
//! USER-APPROVED GUI Home design, the port of `lib/library/home-list.ts`): the
//! flat list's section headings, the spacer between two non-empty sections,
//! and the item rows with their stable identity. The list paints fixed-height
//! rows over native scroll and only the rows in view, as the Music list does,
//! and the cursor moves over the item rows only — headings and spacers are
//! structural, the cursor never rests on them. The model is plain data over
//! fixed heights, so the geometry is asserted without a layout.

use std::sync::Arc;

use dioxus_native::prelude::*;
use mbv_queue::QueueItemContentId;

use crate::art::GuiArtRef;
use crate::bridge::frame::library::{
    GuiHomeRowFrame, GuiHomeRowIdFrame, GuiHomeRowKindFrame, GuiHomeRowStateFrame,
};
use crate::window::library::list::{GeometryRow, filter_match};

/// The section heading row's height in logical pixels: the letter heading
/// height's look, scaled to the section name.
pub const HOME_HEADING_ROW_HEIGHT: f32 = 28.0;
/// The item row's height in logical pixels: the room a 40 px art thumb and
/// the two-line name paint into.
pub const HOME_ITEM_ROW_HEIGHT: f32 = 48.0;
/// The spacer row's height between two non-empty sections (design D12).
pub const HOME_SPACER_ROW_HEIGHT: f32 = 20.0;

/// One carried item row's facts, as the frame paints them (design D12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeItemRow {
    /// The row's stable identity (design D12): the opaque `GuiHomeRowId` the
    /// core resolves, carried back by the select, play, and enqueue commands.
    pub row_id: GuiHomeRowIdFrame,
    /// The row's primary label.
    pub title: String,
    /// The row's secondary label.
    pub subtitle: Option<String>,
    /// The gutter date.
    pub date: Option<String>,
    /// The row's kind (`collection` when the row targets a container the
    /// play path refuses, `media` on a playable leaf).
    pub item_kind: GuiHomeRowKindFrame,
    /// The row's state, with the Continue Watching resume progress.
    pub state: GuiHomeRowStateFrame,
    /// The row's artwork, present exactly while the row's item obtains art
    /// from the image cache.
    pub art: Option<GuiArtRef>,
}

/// One painted row of the flat Home list (design D12). The `id` is the row's
/// stable key: the cursor's identity, the each-block's key, and the id the
/// scroll geometry read.
#[derive(Debug, Clone, PartialEq)]
pub enum HomeListRow {
    /// A section heading row.
    Heading {
        /// The row's stable key.
        id: String,
        /// The row's top edge from the list's first row.
        top: f32,
        /// The row's painted height.
        height: f32,
        /// The section's name, painted as-is.
        text: String,
    },
    /// The gap between two non-empty sections.
    Spacer {
        /// The row's stable key.
        id: String,
        /// The row's top edge from the list's first row.
        top: f32,
        /// The row's painted height.
        height: f32,
    },
    /// One item row.
    Item {
        /// The row's stable key.
        id: String,
        /// The row's top edge from the list's first row.
        top: f32,
        /// The row's painted height.
        height: f32,
        /// The carried item the row paints.
        row: HomeItemRow,
    },
}

impl HomeListRow {
    /// The row's stable key.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            HomeListRow::Heading { id, .. }
            | HomeListRow::Spacer { id, .. }
            | HomeListRow::Item { id, .. } => id,
        }
    }
}

impl GeometryRow for HomeListRow {
    fn id(&self) -> &str {
        self.id()
    }

    fn pointer_row(&self) -> Option<&str> {
        matches!(self, HomeListRow::Item { .. }).then_some(self.id())
    }

    fn top(&self) -> f32 {
        match self {
            HomeListRow::Heading { top, .. }
            | HomeListRow::Spacer { top, .. }
            | HomeListRow::Item { top, .. } => *top,
        }
    }

    fn height(&self) -> f32 {
        match self {
            HomeListRow::Heading { height, .. }
            | HomeListRow::Spacer { height, .. }
            | HomeListRow::Item { height, .. } => *height,
        }
    }
}

/// The row's stable key (design D12): the `GuiHomeRowId` as one string, so a
/// cursor and an each-block can carry it. A `Continue` row keys on its Emby
/// item id, a `Recent` row on its queue-item content id's provider and value.
#[must_use]
pub fn home_row_key(id: &GuiHomeRowIdFrame) -> String {
    match id {
        GuiHomeRowIdFrame::Continue(item_id) => format!("home-continue:{item_id}"),
        GuiHomeRowIdFrame::Recent(content_id) => {
            let (provider, value) = home_content_parts(content_id);
            format!("home-recent:{provider}:{value}")
        }
    }
}

/// The content id's provider and value parts as the key carries them (the
/// wire form's `provider` and `value`): a scalar value as itself, and
/// a struct value as its field list.
fn home_content_parts(id: &QueueItemContentId) -> (&'static str, String) {
    match id {
        QueueItemContentId::Emby(value) => ("Emby", value.clone()),
        QueueItemContentId::Feed(value) => ("Feed", value.clone()),
        QueueItemContentId::Audiobookshelf {
            library_item_id,
            episode_id,
        } => ("Audiobookshelf", format!("{library_item_id}:{episode_id}")),
        QueueItemContentId::AudiobookshelfBook { library_item_id } => {
            ("AudiobookshelfBook", library_item_id.clone())
        }
        QueueItemContentId::Twitch(value) => ("Twitch", value.clone()),
    }
}

/// The painted rows of the flat Home list (design D12): the carried rows
/// sequence — a heading only above its rows, an empty section absent, and the
/// spacer only between two non-empty sections — with each row's top edge and
/// height, so the list paints a window of rows over native scroll. The filter
/// narrows the item rows to the titles that contain its text, without case
/// (design D7's filter rule): a heading stays only above a kept row, and the
/// spacer only between two sections that both kept rows.
#[must_use]
pub fn build_home_rows(rows: &[GuiHomeRowFrame], filter: &str) -> Vec<HomeListRow> {
    let mut built: Vec<HomeListRow> = Vec::new();
    let mut top = 0.0_f32;
    // The heading that opens the current section, until its first kept row
    // emits it.
    let mut heading: Option<String> = None;
    // Whether the section before a carried spacer kept a row: the spacer
    // paints only when the next section keeps one too.
    let mut spacer_pending = false;
    // Whether a section already emitted a row: the spacer marks a join.
    let mut emitted = false;

    for row in rows {
        match row {
            GuiHomeRowFrame::Heading { text } => heading = Some(text.clone()),
            GuiHomeRowFrame::Spacer => spacer_pending = emitted,
            GuiHomeRowFrame::Item(item) => {
                if !filter_match(&item.title, filter) {
                    continue;
                }
                if let Some(text) = heading.take() {
                    if emitted && spacer_pending {
                        let height = HOME_SPACER_ROW_HEIGHT;
                        built.push(HomeListRow::Spacer {
                            id: format!("home-spacer:{top}"),
                            top,
                            height,
                        });
                        top += height;
                    }
                    let height = HOME_HEADING_ROW_HEIGHT;
                    built.push(HomeListRow::Heading {
                        id: format!("home-heading:{text}"),
                        top,
                        height,
                        text,
                    });
                    top += height;
                    spacer_pending = false;
                }
                emitted = true;
                let height = HOME_ITEM_ROW_HEIGHT;
                built.push(HomeListRow::Item {
                    id: home_row_key(&item.id),
                    top,
                    height,
                    row: HomeItemRow {
                        row_id: item.id.clone(),
                        title: item.title.clone(),
                        subtitle: item.subtitle.clone(),
                        date: item.date.clone(),
                        item_kind: item.kind,
                        state: item.state.clone(),
                        art: item.art.clone(),
                    },
                });
                top += height;
            }
        }
    }
    built
}

/// The rows the memo handed back last, with the cheap key and the filter they
/// were built from. The rows sit behind an `Arc` because the former semantics
/// the memo preserves are shared state: one scroll step reads the same rows
/// the last one built, and the list repaints only its visible window.
#[derive(Debug, Clone)]
struct HomeMemoEntry {
    /// The `libraryList` section revision the rows came from (design D6): a
    /// section's value at one revision is immutable, so equal revisions carry
    /// equal rows and the hit test never compares the union.
    revision: Option<u64>,
    filter: String,
    rows: Arc<[HomeListRow]>,
}

/// The Home rows, memoized on the carried rows and the filter (design D9).
/// One slot, because a list paints one content at a time.
#[derive(Clone, Default)]
pub struct HomeRowsMemo {
    last: Signal<Option<HomeMemoEntry>>,
}

impl std::fmt::Debug for HomeRowsMemo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HomeRowsMemo").finish_non_exhaustive()
    }
}

impl HomeRowsMemo {
    /// The rows `build_home_rows` leaves over the carried rows and `filter`:
    /// the ones it built last while the arguments are the same, and a fresh
    /// build otherwise. The rows come keyed on the `libraryList` section
    /// revision they borrowed from (design D6); the caller must pass the slice
    /// the section's current value carries.
    #[must_use]
    pub fn rows(
        &self,
        revision: Option<u64>,
        rows: &[GuiHomeRowFrame],
        filter: &str,
    ) -> Arc<[HomeListRow]> {
        // `peek`, never `read`: a caller inside an effect that writes the
        // memo (a settle that moves the cursor) would re-queue itself on the
        // write below otherwise.
        if let Some(entry) = self.last.peek().as_ref()
            && entry.revision == revision
            && entry.filter == filter
        {
            return Arc::clone(&entry.rows);
        }
        let built: Arc<[HomeListRow]> = build_home_rows(rows, filter).into();
        let mut last = self.last;
        last.set(Some(HomeMemoEntry {
            revision,
            filter: filter.to_owned(),
            rows: Arc::clone(&built),
        }));
        built
    }
}

#[cfg(test)]
mod tests;
