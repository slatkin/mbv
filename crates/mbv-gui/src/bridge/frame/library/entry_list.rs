//! The pill-scoped entry-list destinations' shared wire frames (rows 10.1,
//! 11.1): the pill frame and the entry-row frame the Feeds and Podcasts
//! sections paint, over each destination's own pill DTO, with the one
//! conversion per frame from the core's shared entry-list projection
//! vocabulary. The destination sections in `feeds.rs` and `podcasts.rs` keep
//! what is truly their own — the frame struct (Podcasts names its library
//! index), the Hero, and the docs.

use mbv::{GuiEntryRow, GuiPillOption};
use serde::Serialize;

use super::GuiHomeRowStateFrame;

/// One pill of a pill-scoped destination's pill row (rows 10.1, 11.1): the
/// shared wire pill frame over the destination's pill DTO `P`. The pill row
/// is single-select, as the TUI's one-valued `PillSelection` is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiPillFrame<P> {
    /// The pill's selection: the set-pill command carries it back.
    pub pill: P,
    /// The pill's label: `Latest`, or the scope's own name.
    pub name: String,
    /// Whether the core's stored pill is this one.
    pub selected: bool,
    /// Whether the pill is a collection pill (multi-select) rather than a
    /// single-select one. Every pill-scoped destination's pills are
    /// single-select.
    pub collection: bool,
}

/// One pill frame from the core's pill option, with the wire pill the core
/// pill's ref conversion yields — the one `From<&T>` conversion the bridge
/// keeps per Pill DTO in `bridge::library`.
impl<'a, C, P> From<&'a GuiPillOption<C>> for GuiPillFrame<P>
where
    P: From<&'a C>,
{
    fn from(option: &'a GuiPillOption<C>) -> Self {
        Self {
            pill: P::from(&option.pill),
            name: option.name.clone(),
            selected: option.selected,
            collection: false,
        }
    }
}

/// One row of a pill-scoped destination's entry list (rows 10.1, 11.1). The
/// identity is the entry's stable Service identity, as one opaque string a
/// select, play, or enqueue command sends, never a row position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
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
    /// The row's played state, with the resume percent on a partly played
    /// entry.
    pub state: GuiHomeRowStateFrame,
}

impl From<&GuiEntryRow> for GuiEntryRowFrame {
    fn from(row: &GuiEntryRow) -> Self {
        Self {
            id: row.id.clone(),
            title: row.title.clone(),
            subtitle: row.subtitle.clone(),
            date: row.date.clone(),
            state: GuiHomeRowStateFrame::from(&row.state),
        }
    }
}
