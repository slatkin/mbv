//! The Home list's row model (`add-gui-library-panel` design D12, the
//! USER-APPROVED GUI Home design, the port of `lib/library/home-list.ts`): the
//! flat list's section headings, the spacer between two non-empty sections,
//! and the item rows with their stable identity. The list paints fixed-height
//! rows over native scroll and only the rows in view, as the Music list does,
//! and the cursor moves over the item rows only — headings and spacers are
//! structural, the cursor never rests on them. The model is plain data over
//! fixed heights, so the geometry is asserted without a layout.

use mbv_queue::QueueItemContentId;

use crate::bridge::frame::library::{
    GuiHomeRowFrame, GuiHomeRowIdFrame, GuiHomeRowKindFrame, GuiHomeRowStateFrame, GuiRowArtFrame,
    GuiRowMediaKindFrame,
};
use crate::window::library::list::{GeometryRow, LIBRARY_ROW_HEIGHT, filter_match};

/// The section heading row's height in logical pixels: the letter heading
/// height's look, scaled to the section name.
pub const HOME_HEADING_ROW_HEIGHT: f32 = 28.0;
/// The item row's height in logical pixels: the room the shared top-level row
/// part paints into (gui-delineated-rows design D2).
pub const HOME_ITEM_ROW_HEIGHT: f32 = LIBRARY_ROW_HEIGHT;
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
    /// The row's trailing text (gui-delineated-rows design D2): the Continue
    /// watching time left, or the Recently added row's `day month` date.
    pub trailing: Option<String>,
    /// The row's kind (`collection` when the row targets a container the
    /// play path refuses, `media` on a playable leaf).
    pub item_kind: GuiHomeRowKindFrame,
    /// The row's state, with the Continue Watching resume progress.
    pub state: GuiHomeRowStateFrame,
    /// The row's art (gui-delineated-rows design D4): the cached row image and
    /// the fit its pixels ask for, a fetch in flight, a source with no image,
    /// or a row kind with no art source.
    pub art: GuiRowArtFrame,
    /// The media kind the row's thumb shows while it holds no image (design
    /// D4).
    pub media: GuiRowMediaKindFrame,
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

    fn rows_in_view_id(&self) -> Option<String> {
        match self {
            // The core's own Home encoding (`GuiHomeRowId::rows_in_view_id`),
            // which is not the row key (gui-delineated-rows design D7): a
            // heading and a spacer name no row art.
            HomeListRow::Item { row, .. } => Some(row.row_id.rows_in_view_id()),
            HomeListRow::Heading { .. } | HomeListRow::Spacer { .. } => None,
        }
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
                        trailing: item.trailing.clone(),
                        item_kind: item.kind,
                        state: item.state.clone(),
                        art: item.art.clone(),
                        media: item.media,
                    },
                });
                top += height;
            }
        }
    }
    built
}

#[cfg(test)]
mod tests;
