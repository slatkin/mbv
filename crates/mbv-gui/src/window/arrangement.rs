//! The GUI's two-panel arrangement (`replace-tauri-with-blitz` D9, the port
//! of `lib/bridge/arrangement.ts`): the Library column sits beside the Queue
//! panel, and the window picks its arrangement from the measured Library
//! column width — the container width minus the Queue panel's current width —
//! with the canvas sizes as thresholds. The Queue column width the arrangement
//! subtracts is the painted one, which the session's divider moves
//! (`window/resize.rs`), so a drag and the classification cannot disagree.

use crate::bridge::frame::GuiPanelToggles;
use crate::bridge::layout::LayoutSize;

/// The Queue panel's expanded width: the canvas's expanded artboard.
pub const QUEUE_PANEL_WIDTH_PX: f32 = 400.0;

/// The Queue panel's collapsed width: the `z` toggle's 72 px rail.
pub const QUEUE_RAIL_WIDTH_PX: f32 = 72.0;

/// The Settings screen's sidebar width: the canvas's 260 px nav. The sidebar
/// sits fixed at the left window edge, in the Queue column's former place,
/// and takes neither the session's drag nor the collapse toggle; the screen's
/// Mini check subtracts it from the window width (the `gui-settings-screen`
/// design's width decisions).
pub const SETTINGS_SIDEBAR_WIDTH_PX: f32 = 260.0;

/// The Library column width below which only the Queue panel shows: the
/// 320 px list plus the pill row.
pub const LIBRARY_MINI_MAX_WIDTH_PX: f32 = 480.0;

/// The Library column width from which the Hero pane sits beside the list:
/// the canvas's 880 px, its 320 px list plus a 560 px Hero. The session's list
/// width and the divider come out of it, so the Hero pane flexes from just
/// under its artboard at the threshold.
pub const LIBRARY_WIDE_MIN_WIDTH_PX: f32 = 880.0;

/// The container height Wide also needs: the Hero pane's 600 px artboard.
pub const LIBRARY_WIDE_MIN_HEIGHT_PX: f32 = 600.0;

/// The arrangement of the Library column beside the Queue panel (D9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiArrangement {
    /// Only the Queue panel shows.
    Mini,
    /// The Library column paints alone: no Hero pane beside it.
    Narrow,
    /// The Hero pane sits beside the list.
    Wide,
}

impl GuiArrangement {
    /// The `data-arrangement` and `data-library-arrangement` value the
    /// markup paints: the wire name the former attribute carried.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Mini => "mini",
            Self::Narrow => "narrow",
            Self::Wide => "wide",
        }
    }
}

/// The Queue panel's painted column width: the 72 px rail while the collapse
/// toggle holds, the given expanded width otherwise — the 400 px canvas
/// artboard until the session's divider moves it (D9).
#[must_use]
pub fn queue_column_width(panel: &GuiPanelToggles, expanded_width: f32) -> f32 {
    if panel.collapsed {
        QUEUE_RAIL_WIDTH_PX
    } else {
        expanded_width
    }
}

/// The measured Library column width: the container width minus the Queue
/// panel's painted column width (D9). The client rect the container measures
/// is already CSS pixels, the unit every threshold here names.
#[must_use]
pub fn library_column_width(container: LayoutSize, queue_width: f32) -> f32 {
    (container.width - queue_width).max(0.0)
}

/// The arrangement the Library column paints (D9): Mini below 480 px, Wide
/// from 880 px when the container is at least 600 px tall, Narrow between. A
/// zero width reads as "not measured yet" and is Mini, which shows only the
/// Queue panel. The caller passes the painted Queue column width, so the
/// arrangement follows a divider drag.
#[must_use]
pub fn library_arrangement(container: LayoutSize, queue_width: f32) -> GuiArrangement {
    let width = library_column_width(container, queue_width);
    if width < LIBRARY_MINI_MAX_WIDTH_PX {
        return GuiArrangement::Mini;
    }
    if width >= LIBRARY_WIDE_MIN_WIDTH_PX && container.height >= LIBRARY_WIDE_MIN_HEIGHT_PX {
        return GuiArrangement::Wide;
    }
    GuiArrangement::Narrow
}

/// The arrangement the Settings screen paints (the former client
/// `libraryArrangement(container, SETTINGS_SIDEBAR_WIDTH_PX)`): the Library
/// column's measurement with the screen's fixed 260 px sidebar at the left
/// window edge — the Queue column's former place. The sidebar takes neither
/// the session's dragged Queue width nor the collapse toggle, so the screen's
/// Mini threshold is the window width minus 260 px.
#[must_use]
pub fn settings_arrangement(container: LayoutSize) -> GuiArrangement {
    library_arrangement(container, SETTINGS_SIDEBAR_WIDTH_PX)
}

#[cfg(test)]
mod tests;
