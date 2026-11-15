//! The panel's four states (`replace-tauri-with-blitz` D9, the port of
//! `lib/bridge/panel.ts`): the state is picked from the panel container's
//! measured size plus the Client core's two toggles, never from the window
//! size. The measurement is the same one `LayoutReport.containerSize` carries
//! to the core, so the painted subtree and the core's capacity and hit map
//! follow one size.
//!
//! The state is decided here and not by CSS `@container` rules because each
//! state's acceptance renders from a frame and a container size, and the
//! headless tests evaluate no container query. The window still holds no
//! interaction state (the spec: "The layout that the container size picks is
//! not interaction state"): the size is a measurement and the toggles are the
//! core's.

use crate::bridge::frame::GuiPanelToggles;
use crate::bridge::layout::LayoutSize;

/// The container height below which the panel paints the compact strip (the
/// spec's "shorter than 520 px").
pub const COMPACT_STRIP_MAX_HEIGHT: f32 = 520.0;

/// One of the panel's four canvas states (D9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiPanelState {
    /// The canvas's expanded artboard: the playback block, the queue, and the
    /// footer line.
    Expanded,
    /// The container is too short for the expanded artboard: the 96×54
    /// still, the title and its meta line, play/pause, next, and a thin
    /// progress line.
    CompactStrip,
    /// The `q` toggle folded the queue to its one-line summary.
    ExpandedWithoutQueue,
    /// The `z` toggle collapsed the panel to its 72 px rail.
    Rail,
}

/// The state the panel paints (D9): the collapsed toggle wins over every
/// size, then a container too short for the expanded artboard, then the queue
/// fold. A zero height reads as "not measured yet" and paints the expanded
/// artboard, so the first frame does not flash a strip the window is not.
#[must_use]
pub fn panel_state(container: LayoutSize, panel: &GuiPanelToggles) -> GuiPanelState {
    if panel.collapsed {
        return GuiPanelState::Rail;
    }
    if container.height > 0.0 && container.height < COMPACT_STRIP_MAX_HEIGHT {
        return GuiPanelState::CompactStrip;
    }
    if panel.queue_folded {
        return GuiPanelState::ExpandedWithoutQueue;
    }
    GuiPanelState::Expanded
}

impl GuiPanelState {
    /// The `data-panel-state` value the former markup paints: `expanded`,
    /// `compactStrip`, `expandedWithoutQueue`, `rail`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Expanded => "expanded",
            Self::CompactStrip => "compactStrip",
            Self::ExpandedWithoutQueue => "expandedWithoutQueue",
            Self::Rail => "rail",
        }
    }
}
