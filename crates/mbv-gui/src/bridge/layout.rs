//! The layout report that the front end sends after each paint that moved a
//! region (design D4). It carries the whole visible rows of each list and the
//! painted regions, each with the semantic target a press on it resolves to.
//!
//! The target ids mirror the surface hit target enums variant for variant:
//! `QueueTargetId` names `QueueHitTarget`, `QueuePlaybackTargetId` names
//! `QueuePlaybackHitTarget`, and `StatusBarTargetId` names `StatusBarHitTarget`.

use mbv_surfaces::{HitMap, HitRegion};
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
    /// The panel container's painted size in logical pixels (design D10):
    /// the size the front end's container queries pick the panel state from.
    /// The core stores it so the capacity and the hit map the same report
    /// carries stay matched to the layout the front end painted; the
    /// panel-state decision itself stays in the front end. A front end that
    /// omits it reads as the zero size.
    #[serde(default)]
    pub container_size: LayoutSize,
}

/// A size in logical pixels, as the front end measures the panel container.
#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutSize {
    /// The painted width in logical pixels.
    pub width: f32,
    /// The painted height in logical pixels.
    pub height: f32,
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
    /// A pre-attach screen's button (design D9). The mismatch state's window
    /// paints Restart and Quit; before attach the core thread resolves them
    /// itself, so the press routes to the campaign, not to a surface leaf.
    Attach(AttachTargetId),
}

impl RegionTarget {
    /// Whether a press on this region lands on an open overlay's own target
    /// (add-tauri-gui-client row 6.5): the overlay's rows, or a control that
    /// swaps or closes it. Every other region is "outside", and a press
    /// there dismisses the open overlay instead of reaching the surface
    /// beneath.
    #[must_use]
    pub(crate) fn owns_gui_overlay(&self) -> bool {
        match self {
            // A chooser, popover, or row-menu row; the row menu's opener; and
            // the chips and header target whose presses toggle their overlay
            // (a sibling chip's press swaps the open popover).
            Self::QueuePlayback(
                QueuePlaybackTargetId::TargetChoice(_)
                | QueuePlaybackTargetId::TrackPopoverChoice(_)
                | QueuePlaybackTargetId::AudioChip
                | QueuePlaybackTargetId::SubtitleChip
                | QueuePlaybackTargetId::PlaybackTarget,
            )
            | Self::Queue(QueueTargetId::RowMenuEntry(_) | QueueTargetId::RowActions(_)) => true,
            Self::Queue(_) | Self::QueuePlayback(_) | Self::StatusBar(_) | Self::Attach(_) => false,
        }
    }
}

/// The pre-attach screen's button targets (design D9), as the mismatch
/// state's window paints them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AttachTargetId {
    /// The Restart button: the same choice key R makes.
    Restart,
    /// The Quit button: the same choice key Q makes.
    Quit,
}

/// The hit map of one report's painted regions (design D4): the map a pointer
/// press resolves against, in paint order. The core thread rebuilds it after
/// every report, in the attached slice's step and in the attach campaign's
/// mismatch state alike.
#[must_use]
pub(crate) fn hit_map(regions: &[LayoutRegion]) -> HitMap<RegionTarget> {
    let mut map = HitMap::new();
    for region in regions {
        map.push(
            HitRegion::new(
                region.rect.x,
                region.rect.y,
                region.rect.width,
                region.rect.height,
            ),
            region.target,
        );
    }
    map
}

/// The Queue target ids, mirroring `QueueHitTarget` variant for variant, plus
/// the canvas queue's own row chrome (add-tauri-gui-client row 6.5), which the
/// TUI paints no counterpart of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QueueTargetId {
    /// A queue row for one stable slot. The number is `QueueSlotId::raw`.
    Slot(u64),
    /// The playlist pill in the queue footer.
    PlaylistPill,
    /// The row's drag grip, which the frame paints on the hovered row. A
    /// press starts the row drag; the number is the row's `QueueSlotId::raw`.
    RowGrip(u64),
    /// The row's actions button, which the frame paints on the hovered row.
    /// A press opens the row menu for that slot.
    RowActions(u64),
    /// One entry of the open row menu: the entry's index in the frame's
    /// overlay `entries` list. A press executes the entry.
    RowMenuEntry(usize),
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
    /// The back-10 s control: the `seek_back` chord's pointer target
    /// (add-tauri-gui-client row 6.4).
    SeekBack,
    /// The forward-30 s control: the `seek_forward` chord's pointer target.
    SeekForward,
    /// The header's collapse control: the `z` toggle's pointer target.
    Collapse,
    /// The header's queue-fold control: the `q` toggle's pointer target.
    QueueFold,
    /// One row of the open playback-target chooser: the choice's index in
    /// the frame's `targetChoices` list.
    TargetChoice(usize),
    /// The audio chip: a press toggles its track popover.
    AudioChip,
    /// The subtitle chip: a press toggles its track popover. The video
    /// chip is label-only (design D4a's ruling) and reports no target.
    SubtitleChip,
    /// One row of the open track popover: the choice's index in the
    /// popover's `choices` list.
    TrackPopoverChoice(usize),
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
