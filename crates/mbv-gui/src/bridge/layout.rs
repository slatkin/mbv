//! The layout report that the front end sends after each paint that moved a
//! region (design D4). It carries the whole visible rows of each list and the
//! painted regions, each with the semantic target a press on it resolves to.
//!
//! The target ids mirror the surface hit target enums variant for variant:
//! `QueueTargetId` names `QueueHitTarget`, `QueuePlaybackTargetId` names
//! `QueuePlaybackHitTarget`, and `StatusBarTargetId` names `StatusBarHitTarget`.

use serde::Deserialize;

/// One layout report (design D4). The front end sends it after each paint that
/// moved a region.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutReport {
    /// The whole visible rows of each list that the front end painted.
    pub lists: Vec<ListRows>,
    /// The painted regions, each with the semantic target a press on it resolves to.
    pub regions: Vec<LayoutRegion>,
    /// The window's inner width in logical pixels (design D10). The core
    /// thread classifies it with the shared breakpoint classes. A front end
    /// that omits it reads as 0.0.
    #[serde(default)]
    pub width: f32,
}

/// The whole visible rows of one list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListRows {
    /// The list that the row count applies to.
    pub list: ListId,
    /// The rows that the list's viewport fully shows.
    pub whole_rows: usize,
}

/// The lists that report whole visible rows. Only the Queue reports a count
/// today (`QueueFrameReport::capacity`); a list that reports one adds a variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ListId {
    Queue,
}

/// One painted region and the semantic target a press on it resolves to.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutRegion {
    /// The region's rectangle in logical pixels, relative to the window.
    pub rect: LayoutRect,
    pub target: RegionTarget,
}

/// A rectangle in logical pixels. It mirrors `HitRegion`, the rectangle the
/// `mbv-surfaces` hit map uses.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// The semantic target of one painted region: the surface that owns the region
/// and the target id a press there resolves to. On the wire it is
/// `{ "surface": ..., "id": ... }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(tag = "surface", content = "id", rename_all = "camelCase")]
pub enum RegionTarget {
    /// A Queue region.
    Queue(QueueTargetId),
    /// A Queue playback region.
    QueuePlayback(QueuePlaybackTargetId),
    /// A Status bar region.
    StatusBar(StatusBarTargetId),
}

/// The Queue target ids, mirroring `QueueHitTarget`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QueueTargetId {
    /// A queue row for one stable slot. The number is `QueueSlotId::raw`.
    Slot(u64),
    /// The playlist pill in the queue footer.
    PlaylistPill,
}

/// The Queue playback target ids, mirroring `QueuePlaybackHitTarget`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QueuePlaybackTargetId {
    /// The play/pause glyph.
    PlayPause,
    /// The stop glyph.
    Stop,
    /// The previous glyph.
    Previous,
    /// The next glyph.
    Next,
    /// The seekbar.
    Seekbar,
    /// The playback-target indicator in the header row.
    PlaybackTarget,
    /// The visual slot's image rectangle.
    VisualSlot,
}

/// The Status bar target ids, mirroring `StatusBarHitTarget`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StatusBarTargetId {
    /// The volume pill.
    VolumePill,
    /// The mute pill.
    MutePill,
    /// The visual-mode indicator.
    VisualClear,
}
