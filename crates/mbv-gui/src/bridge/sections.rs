//! The section batch the Client core sends as one update (design D1 of
//! `gui-change-driven-sections`). Each section is one optional, skip-if-none
//! field of [`GuiSectionBatch`]; a present field is a [`GuiSection`] that
//! carries the section's revision with its value. An absent field means
//! "unchanged". A section whose content can be absent carries an `Option`
//! value, so `null` clears it.
//!
//! The playhead, the Library list and detail, and the row menu join the batch
//! in the later groups of the change. Until then the Library rides one gated
//! `library` section whose value is the existing content.

use serde::Serialize;

use super::frame::GuiAttachState;
use super::frame::GuiPanelToggles;
use super::frame::chrome::{GuiQueueFooter, GuiStatusBar};
use super::frame::focus::GuiFocus;
use super::frame::library::{GuiLibraryContent, GuiSearchFrame};
use super::frame::overlay::GuiOverlayFrame;
use super::frame::playback::{GuiQueuePlayback, GuiScrubberHover};
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
    /// The panel toggles the front end picks the layout with.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub panel: Option<GuiSection<GuiPanelToggles>>,
    /// The open menu or popover, `null` while none is open.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlay: Option<GuiSection<Option<GuiOverlayFrame>>>,
    /// The transport band's facts, without the hover preview.
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
    /// The Library content of the active destination, `null` while no
    /// presented destination is active. Until the Library groups split it, the
    /// section carries the whole content, row menu included.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library: Option<GuiSection<Option<GuiLibraryContent>>>,
    /// The Search section: the query the top bar answers and its rows, `null`
    /// while the field is closed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<GuiSection<Option<GuiSearchFrame>>>,
    /// The Settings screen, `null` while the screen is closed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<GuiSection<Option<GuiSettingsFrame>>>,
}

impl GuiSectionBatch {
    /// Whether the batch carries no section: a tick that changed nothing sends
    /// no batch (design D2).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}
