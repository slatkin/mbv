//! The section batch the Client core sends as one update (design D1 of
//! `gui-change-driven-sections`). Each section is one optional, skip-if-none
//! field of [`GuiSectionBatch`]; a present field is a [`GuiSection`] that
//! carries the section's revision with its value. An absent field means
//! "unchanged". A section whose content can be absent carries an `Option`
//! value, so `null` clears it.
//!
//! The Library list, detail, and row menu ride the batch as three gated
//! sections, built in one Library pass (design D8).

use serde::Serialize;

use super::frame::GuiAttachState;
use super::frame::GuiPanelToggles;
use super::frame::chrome::{GuiQueueFooter, GuiStatusBar};
use super::frame::focus::GuiFocus;
use super::frame::library::{
    GuiLibraryDetail, GuiLibraryList, GuiLibraryRowMenuFrame, GuiSearchFrame,
};
use super::frame::overlay::GuiOverlayFrame;
use super::frame::playback::{GuiPlayhead, GuiQueuePlayback, GuiScrubberHover};
use super::frame::queue::{GuiQueue, GuiQueueSummary};
use super::frame::settings::GuiSettingsFrame;
use super::frame::theme::GuiTheme;

/// One section's content and the revision it carries (design D1). The window
/// keeps the section in its own store and skips a write whose revision it
/// already holds.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiSection<T> {
    /// The section's revision. It changes whenever the built value differs
    /// from the value built before it.
    pub revision: u64,
    /// The section's value at that revision.
    pub value: T,
}

/// The one update the core sends for a tick (design D1): a present field
/// replaces the window's section at the carried revision, and an absent field
/// leaves the window's section as it is.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiSectionBatch {
    /// The pre-attach state the window shows, `null` once the attach succeeds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attach: Option<GuiSection<Option<GuiAttachState>>>,
    /// The role-to-colour map.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<GuiSection<GuiTheme>>,
    /// The focused surface and the Panel that holds focus.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus: Option<GuiSection<GuiFocus>>,
    /// The Queue rows inside the viewport, with the cursor, marks, and the
    /// active slot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue: Option<GuiSection<GuiQueue>>,
    /// The queue summary: the remaining count, the finish clock, the right
    /// column's mode, and the filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<GuiSection<GuiQueueSummary>>,
    /// The panel toggles the front end picks the layout with, and the UI-reset
    /// counter that resets its interaction stores.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub panel: Option<GuiSection<GuiPanelToggles>>,
    /// The open menu or popover, `null` while none is open.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlay: Option<GuiSection<Option<GuiOverlayFrame>>>,
    /// The transport band's facts, without the hover preview and without the
    /// playhead facts (design D5: they ride the `playhead` anchor).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playback: Option<GuiSection<GuiQueuePlayback>>,
    /// The scrubber's hover preview, `null` while the pointer is away from the
    /// bar.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hover: Option<GuiSection<Option<GuiScrubberHover>>>,
    /// The Queue footer's status pills.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footer: Option<GuiSection<GuiQueueFooter>>,
    /// The Status bar projection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<GuiSection<GuiStatusBar>>,
    /// The Library list: the destination tabs and the active destination's list
    /// part, `null` while no presented destination is active.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library_list: Option<GuiSection<Option<GuiLibraryList>>>,
    /// The Library detail: the active destination's selection, Hero, Workspace,
    /// and playing marks, `null` while no presented destination is active.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library_detail: Option<GuiSection<Option<GuiLibraryDetail>>>,
    /// The right-click item menu the window last opened, `null` while no
    /// destination is active or no row has named an item.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_menu: Option<GuiSection<Option<GuiLibraryRowMenuFrame>>>,
    /// The Search section: the query the top bar answers and its rows, `null`
    /// while the field is closed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<GuiSection<Option<GuiSearchFrame>>>,
    /// The Settings screen, `null` while the screen is closed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<GuiSection<Option<GuiSettingsFrame>>>,
    /// The playhead anchor (design D5): the position, runtime, paused state,
    /// and active slot at the send, from which the window predicts the
    /// position until the next anchor. The core sends one only on the anchor
    /// rule's triggers, so the field's absence also means "no re-anchor".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playhead: Option<GuiSection<GuiPlayhead>>,
}

impl GuiSectionBatch {
    /// Whether the batch carries no section: a tick that changed nothing sends
    /// no batch (design D2).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}
