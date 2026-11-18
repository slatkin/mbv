//! The two dividers (`replace-tauri-with-blitz` D9, D14; the port of
//! `lib/bridge/resize.ts`): the Queue panel's column and the Wide
//! arrangement's list column are resized by dragging. The widths persist
//! across launches: a drag end and a double-click reset write them through
//! `mbv-config`'s `gui-panel-widths.toml` (D14), and the next start reads them
//! back. A width the user dragged also outlives tab and breakpoint changes,
//! because the store lives at the root as a context value.
//!
//! Every painted width resolves here, so the columns the window paints and
//! the width the arrangement classifies are one value: a drag can never paint
//! a column the thresholds do not measure. The Queue panel keeps its canvas
//! width as its minimum, and its maximum is its 800 px cap, or the width
//! that leaves the Library column its Mini threshold on a narrower window,
//! so a drag removes no panel and the thresholds still decide Mini, Narrow,
//! and Wide.

use std::sync::Arc;

use dioxus_native::prelude::*;
use mbv_config::GuiPanelWidths;
use num_traits::ToPrimitive;

use crate::bridge::frame::GuiPanelToggles;
use crate::bridge::layout::LayoutSize;
use crate::window::arrangement::{
    LIBRARY_MINI_MAX_WIDTH_PX, QUEUE_PANEL_MAX_WIDTH_PX, QUEUE_PANEL_WIDTH_PX,
    library_column_width, queue_column_width,
};
use crate::window::input::f32_from_f64;

/// The list column's default width: the canvas's 320 px index, 15 % wider.
pub const LIBRARY_LIST_DEFAULT_WIDTH_PX: f32 = 368.0;

/// The list column's narrowest: the canvas's 320 px index.
pub const LIBRARY_LIST_MIN_WIDTH_PX: f32 = 320.0;

/// The narrowest the Hero pane keeps beside the list, so neither pane squeezes
/// the other away: the list column's own minimum.
pub const HERO_MIN_WIDTH_PX: f32 = LIBRARY_LIST_MIN_WIDTH_PX;

/// A divider's grab band width. The band overlays the seam between two
/// columns and takes no width in the row, so the columns meet directly and the
/// Library column the Queue column leaves is the container minus the Queue
/// column.
pub const DIVIDER_WIDTH_PX: f32 = 6.0;

/// The widths the user dragged this session, in CSS pixels: `None` while the
/// divider holds its default.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DraggedWidths {
    /// The Queue column's dragged width, `None` at its default.
    pub queue: Option<f32>,
    /// The list column's dragged width, `None` at its default.
    pub list: Option<f32>,
}

/// The save edge (D14): every width change writes through it. The window's
/// instance writes `mbv-config`'s `gui-panel-widths.toml`; a test records.
pub type WidthsSaver = Arc<dyn Fn(&GuiPanelWidths) + Send + Sync>;

/// A Queue column width held between the panel's canvas width and the lesser
/// of its 800 px cap and the width that leaves the Library column its Mini
/// threshold: the Queue panel never narrows below its artboard and never
/// grows past its cap or enough to remove the Library column's two-panel
/// states.
#[must_use]
pub fn clamp_queue_width(width: f32, container_width: f32) -> f32 {
    let room = container_width.round() - LIBRARY_MINI_MAX_WIDTH_PX;
    let max = QUEUE_PANEL_WIDTH_PX.max(QUEUE_PANEL_MAX_WIDTH_PX.min(room));
    width.round().clamp(QUEUE_PANEL_WIDTH_PX, max)
}

/// A list column width held between the canvas index and the room the Hero
/// pane leaves beside it at its minimum. A Library column too narrow for both
/// panes gives the list its minimum.
#[must_use]
pub fn clamp_list_width(width: f32, library_column: f32) -> f32 {
    let max = LIBRARY_LIST_MIN_WIDTH_PX.max(library_column.round() - HERO_MIN_WIDTH_PX);
    width.round().clamp(LIBRARY_LIST_MIN_WIDTH_PX, max)
}

/// The widths the window paints, and the Library column the Queue column
/// leaves: the one resolution the columns and the arrangement both read (D9).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelWidths {
    /// The Queue panel's painted column width: the rail while collapsed.
    pub queue: f32,
    /// The list column's painted width, which only Wide paints.
    pub list: f32,
    /// The Library column's width: the container minus the Queue column.
    pub library_column: f32,
}

/// The painted widths from the measured container, the core's toggles, and
/// the session's drags (D9). The stored widths are clamped again here, so a
/// width dragged on a wider window narrows with the window instead of
/// squeezing a pane away.
#[must_use]
pub fn panel_widths(
    container: LayoutSize,
    panel: &GuiPanelToggles,
    dragged: &DraggedWidths,
) -> PanelWidths {
    let queue = queue_column_width(
        panel,
        clamp_queue_width(
            dragged.queue.unwrap_or(QUEUE_PANEL_WIDTH_PX),
            container.width,
        ),
    );
    let library_column = library_column_width(container, queue);
    PanelWidths {
        queue,
        list: clamp_list_width(
            dragged.list.unwrap_or(LIBRARY_LIST_DEFAULT_WIDTH_PX),
            library_column,
        ),
        library_column,
    }
}

/// The widths the session holds (D14, the port of the `draggedWidths` store):
/// a drag writes one, a double-click clears it, and every change is saved, so
/// the widths survive a relaunch as well as tab and breakpoint changes. The
/// root creates it once and provides it as context.
#[derive(Clone)]
pub struct PanelWidthsStore {
    queue: Signal<Option<f32>>,
    list: Signal<Option<f32>>,
    /// The `ui_reset_epoch` the window last saw (the `gui-settings-screen`
    /// design's reset counter): the first frame seeds it, and a change resets
    /// the widths.
    last_ui_reset_epoch: Signal<Option<u64>>,
    saver: WidthsSaver,
}

impl std::fmt::Debug for PanelWidthsStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PanelWidthsStore")
            .field("queue", &self.queue.peek().clone())
            .field("list", &self.list.peek().clone())
            .field(
                "last_ui_reset_epoch",
                &self.last_ui_reset_epoch.peek().clone(),
            )
            .finish_non_exhaustive()
    }
}

impl PanelWidthsStore {
    /// The store a start builds (D14): the widths the previous launch saved,
    /// and the writer every later change sends through.
    #[must_use]
    pub fn new(from_launch: GuiPanelWidths, write_widths: WidthsSaver) -> Self {
        Self {
            queue: Signal::new(
                from_launch
                    .queue
                    .map(|width| f32_from_f64(f64::from(width))),
            ),
            list: Signal::new(from_launch.list.map(|width| f32_from_f64(f64::from(width)))),
            last_ui_reset_epoch: Signal::new(None),
            saver: write_widths,
        }
    }

    /// The widths the session holds right now.
    #[must_use]
    pub fn dragged(&self) -> DraggedWidths {
        DraggedWidths {
            queue: self.queue.cloned(),
            list: self.list.cloned(),
        }
    }

    /// A drag on the Queue panel's divider: the panel's new column width,
    /// clamped and saved (D14).
    pub fn drag_queue_width(&mut self, width: f32, container_width: f32) {
        self.queue
            .set(Some(clamp_queue_width(width, container_width)));
        self.save();
    }

    /// A drag on the list column's divider: the column's new width, held
    /// inside the Library column the Queue panel leaves, and saved (D14).
    pub fn drag_list_width(&mut self, width: f32, library_column: f32) {
        self.list.set(Some(clamp_list_width(width, library_column)));
        self.save();
    }

    /// A double-click on the Queue panel's divider: the canvas width again.
    pub fn reset_queue_width(&mut self) {
        self.queue.set(None);
        self.save();
    }

    /// A double-click on the list column's divider: the default width again.
    pub fn reset_list_width(&mut self) {
        self.list.set(None);
        self.save();
    }

    /// Drop the dragged widths and write the defaults (D14's Reset UI State):
    /// the panels paint their defaults, and the next launch starts from them
    /// too.
    pub fn reset_panel_widths(&mut self) {
        self.queue.set(None);
        self.list.set(None);
        self.save();
    }

    /// Fold one frame's `ui_reset_epoch` into the width store (the
    /// `gui-settings-screen` design's Reset UI State): a new value — the
    /// core's Reset UI State — drops the dragged widths and writes the
    /// defaults. Returns whether the value changed, so the caller resets the
    /// other interaction stores on the same fact. The first value only seeds
    /// it.
    ///
    /// The held epoch is read with `peek`, never `cloned`: every signal
    /// write marks its subscribers (an equal value included), so a caller
    /// whose effect reads this signal here would re-queue itself on the
    /// write below and never settle. The caller's effect subscribes to the
    /// `panel` section alone.
    pub fn note_ui_reset_epoch(&mut self, epoch: u64) -> bool {
        let held = *self.last_ui_reset_epoch.peek();
        let changed = held.is_some() && held != Some(epoch);
        self.last_ui_reset_epoch.set(Some(epoch));
        if changed {
            self.reset_panel_widths();
        }
        changed
    }

    /// Write the current widths through the saver (D14): every change saves,
    /// so the widths survive a relaunch.
    fn save(&self) {
        let widths = GuiPanelWidths {
            queue: saved_width(self.queue.cloned()),
            list: saved_width(self.list.cloned()),
        };
        (self.saver)(&widths);
    }
}

/// A held width as the file carries it (D14): the clamps hold every width at
/// the list minimum or above and finite, so the conversion cannot miss; a
/// non-finite outlier saturates at the top rather than storing a wrong small
/// width.
fn saved_width(width: Option<f32>) -> Option<u32> {
    width.map(|width| width.round().max(0.0).to_u32().unwrap_or(u32::MAX))
}

#[cfg(test)]
mod tests;
