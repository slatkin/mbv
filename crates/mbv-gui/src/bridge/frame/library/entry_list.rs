//! The collection-scoped entry-list destinations' shared wire frames (rows
//! 10.1, 11.1): the entry-row frame the Feeds and Podcasts sections paint, over
//! the one conversion from the core's shared entry-list projection vocabulary. The
//! destination sections in `feeds.rs` and `podcasts.rs` keep what is truly
//! their own — the frame struct (Podcasts names its library index), the Hero,
//! and the docs. The collection selector both destinations paint is the one
//! `GuiCollections` (gui-collection-dropdown design D6).

use mbv::GuiEntryRow;

use super::{GuiHomeRowStateFrame, GuiRowArtFrame, GuiRowMediaKindFrame};

/// One row of a collection-scoped destination's entry list (rows 10.1, 11.1). The
/// identity is the entry's stable Service identity, as one opaque string a
/// select, play, or enqueue command sends, never a row position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiEntryRowFrame {
    /// The entry's stable identity, as the destination's id encoding writes
    /// it.
    pub id: String,
    /// The entry's display title, the row's primary label.
    pub title: String,
    /// The row's secondary label — the fact that names the row's scope (the
    /// subscription, the show) in the merged Latest view.
    pub subtitle: Option<String>,
    /// The gutter date: the TUI row's fixed right-aligned `day month` text.
    /// `None` on an undated entry, and `Some("")` never rides here — the
    /// TUI's rows drop an empty gutter too.
    pub date: Option<String>,
    /// The row's rail letter (gui-flat-list-sort design D6), or `#` for a
    /// digit- or symbol-led title. `Some` exactly while the list's sort is a
    /// name sort, so a date-ordered list shows no rail.
    pub sort_letter: Option<char>,
    /// The row's played state, with the resume percent on a partly played
    /// entry.
    pub state: GuiHomeRowStateFrame,
    /// The row's art (gui-delineated-rows design D4).
    pub art: GuiRowArtFrame,
    /// The media kind the row shows when it has no image (design D4).
    pub media: GuiRowMediaKindFrame,
}

impl From<&GuiEntryRow> for GuiEntryRowFrame {
    fn from(row: &GuiEntryRow) -> Self {
        Self {
            id: row.id.clone(),
            title: row.title.clone(),
            subtitle: row.subtitle.clone(),
            date: row.date.clone(),
            sort_letter: row.sort_letter,
            state: GuiHomeRowStateFrame::from(&row.state),
            art: GuiRowArtFrame::from(&row.art),
            media: GuiRowMediaKindFrame::from(row.media),
        }
    }
}
