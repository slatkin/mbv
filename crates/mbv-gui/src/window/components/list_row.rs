//! The shared row vocabulary (`gui-unified-list-rows` design D1, D4;
//! `gui-tree-row-roles` design D1, D7): the rules every Library and Playlists
//! row part derives its look from, and the row constants. [`RowLevel`] names
//! the level a part paints and derives the title's tone and weight and the
//! selection's fill from it, so no part and no list restates a tone; a row
//! that carries a play state derives its title look from
//! [`RowPlay`](crate::window::components::play_mark::RowPlay), the one
//! played-state rule; the selection paints square at every level, because the
//! leaf well's corners clip the leaf's fill and a branch fills its row
//! (design D7). [`context_press`] is the one right-press handler. [`thumb`]
//! holds the art thumb the parts paint, [`flat`] the top-level row,
//! [`branch`] the branch row, [`leaf`] the leaf row, and [`tree`] the nested
//! part the two role parts replace.
//!
//! A part names its level and its play state, and never a look value: a
//! destination that restyles a row changes this module, and a list that passes
//! a look value does not compile.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiHomeRowStateFrame;
use crate::window::components::play_mark::RowPlay;
use crate::window::components::tree_row::LEAF_ROW_FONT_WEIGHT;
use crate::window::input::f32_from_f64;
use crate::window::library::LIBRARY_ROW_TITLE_WEIGHT;
use crate::window::theme::canvas_var;

pub mod branch;
pub mod flat;
pub mod leaf;
pub mod thumb;
pub mod tree;

pub use crate::window::library::list::{
    BRANCH_ROW_HEIGHT, LIBRARY_ROW_HEIGHT, NODE_ROW_HEIGHT, TRACK_ROW_HEIGHT, WELL_GAP_BOTTOM_PX,
    WELL_GAP_TOP_PX, WellEdge,
};
pub use branch::{BranchRow, BranchRowProps};
pub use flat::FlatRow;
pub use leaf::{LeafRow, LeafRowProps};
pub use thumb::{LibraryRowThumb, paint_thumb};
pub use tree::{TreeDepth, TreeLevel, TreeRow, TreeTrail};

/// The top-level row's left and right padding, in logical pixels
/// (gui-delineated-rows design D2; `gui-tree-indent-guides` design D2): the
/// row's style reads it, so a top-level tree parent's guide follows the thumb
/// when the padding changes.
pub const ROW_PADDING_PX: f32 = 8.0;

/// The divider width under a top-level row, in logical pixels (gui-delineated-rows
/// design D2): the row's style paints it and a guide that starts on the row
/// crosses it (`gui-tree-indent-guides` design D3).
pub const ROW_DIVIDER_PX: f32 = 1.0;

/// The thumb box's width in logical pixels (gui-delineated-rows design D2).
pub const THUMB_WIDTH_PX: f32 = 72.0;

/// The thumb box's height in logical pixels (gui-delineated-rows design D2).
pub const THUMB_HEIGHT_PX: f32 = 40.0;

/// The side of a square image inside the thumb box (gui-delineated-rows design
/// D2): the box's height, centred, so the image shows whole.
pub const THUMB_SQUARE_PX: f32 = THUMB_HEIGHT_PX;

/// The thumb box's corner radius (gui-delineated-rows design D2).
pub const THUMB_RADIUS_PX: f32 = 3.0;

/// The media kind icon's size inside a thumb with no image (gui-delineated-rows
/// design D2).
pub const THUMB_ICON_PX: u16 = 16;

/// The thumb's progress track and fill height, along the box's bottom edge
/// (gui-delineated-rows design D2).
pub const THUMB_PROGRESS_PX: f32 = 3.0;

/// The trailing text's hook part: the date gutter or the duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryTrailPart {
    /// The date gutter.
    Date,
    /// The runtime.
    Duration,
}

impl LibraryTrailPart {
    /// The part name the row's hook paints (`data-library-<attr>-<part>`).
    #[must_use]
    pub fn part(self) -> &'static str {
        match self {
            LibraryTrailPart::Date => "date",
            LibraryTrailPart::Duration => "duration",
        }
    }
}

/// The level a row part paints (`gui-unified-list-rows` design D2, D3;
/// `gui-tree-row-roles` design D1, D7): a top-level row, a branch row, or a
/// leaf row. The selection fill is one fill at every level and paints square;
/// the title tone is what the level decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowLevel {
    /// A top-level row: every flat row and a top-level tree parent (an artist,
    /// a series, a playlist).
    Top,
    /// A branch row: an album and a season (`gui-tree-row-roles` design D1).
    Branch,
    /// A nested leaf row: a track, an episode, and a playlist item.
    Leaf,
}

impl RowLevel {
    /// The title's tone (`gui-unified-list-rows` design D4;
    /// gui-played-row-marks design D1; `gui-tree-row-roles` design D7): a row
    /// that carries a play state paints the tone [`RowPlay`] derives from it;
    /// a row that carries none paints the text tone at every level.
    #[must_use]
    pub fn title_tone(self, play: Option<&GuiHomeRowStateFrame>) -> String {
        if let Some(play) = play {
            return RowPlay::of_row(play).title_tone().to_owned();
        }
        canvas_var("text")
    }

    /// The title's weight (gui-played-row-marks design D1, D4): a row that
    /// carries a play state paints the weight [`RowPlay`] derives from it; a
    /// row that carries none keeps its level's present weight — the Library
    /// title weight on a top-level row, 400 on a branch, and the leaf weight
    /// on a leaf.
    #[must_use]
    pub fn title_weight(self, play: Option<&GuiHomeRowStateFrame>) -> f32 {
        if let Some(play) = play {
            return RowPlay::of_row(play).title_weight();
        }
        match self {
            RowLevel::Top => LIBRARY_ROW_TITLE_WEIGHT,
            RowLevel::Branch => 400.0,
            RowLevel::Leaf => LEAF_ROW_FONT_WEIGHT,
        }
    }

    /// The selection's fill (`gui-unified-list-rows` design D2, D3): the raised
    /// library fill while the selection marks the row, `None` otherwise.
    #[must_use]
    pub fn selection_fill(self, selected: bool) -> Option<String> {
        selected.then(|| canvas_var("library-raise"))
    }
}

/// The trailing text's class (`gui-row-hover-actions` design D2): the mono face,
/// plus the trail marker the stylesheet hides while the hover controls cover the
/// text. A row that offers no control keeps the trail visible, so its text never
/// steps aside for nothing.
#[must_use]
pub fn hover_trail_class(paints_actions: bool) -> &'static str {
    if paints_actions {
        "row-hover-trail flex-none font-mono tabular-nums"
    } else {
        "flex-none font-mono tabular-nums"
    }
}

/// The host's `style` (`gui-row-hover-actions` design D2): the row's fill as
/// `--row-fill`, which the row button and the hover controls' cover both read,
/// and the width of the divider the row paints under itself as `--row-divider`,
/// which the cover stops above so its fill never blanks the line.
#[must_use]
pub fn row_host_style(fill: &str, divider_px: f32) -> String {
    format!("--row-fill: {fill}; --row-divider: {divider_px}px;")
}

/// The one right-press handler every row part shares (`gui-unified-list-rows`
/// design D4): a right-press on a row that names a menu stops the window's
/// default, presses the row first — so the menu's View Detail opens the Hero
/// for this row — and then opens the menu at the pointer position. A row that
/// names no menu keeps the default.
pub fn context_press(
    event: &MouseEvent,
    on_press: Callback<()>,
    on_context: Option<Callback<(f32, f32)>>,
) {
    let Some(on_context) = on_context else {
        return;
    };
    event.prevent_default();
    on_press.call(());
    let coordinates = event.client_coordinates();
    on_context.call((f32_from_f64(coordinates.x), f32_from_f64(coordinates.y)));
}

#[cfg(test)]
mod tests;
