//! The one tree row text look (user request 2026-11-21): every tree row in the
//! window — a Playlists playlist or item, a Music album or track, a Shows
//! season or episode, and a top-level tree parent row — paints at the font size
//! declared here, and every one of their `Truncate` boxes measures at the same
//! size. The row boxes differ (height, inset, fill, guides), but the text does
//! not: a tree row cannot be born at another size, and a change here lands in
//! every tree at once.
//!
//! Painted and measured size are one number. The row box's inline style carries
//! [`TREE_ROW_FONT_PX`] so the title inherits it, and [`tree_row_text`] hands the
//! `Truncate` box the same size, so a cut lands where the paint shows it.

use crate::window::truncate::TextStyle;

/// The tree rows' font size, in CSS pixels: one value for every tree row in
/// the window, painted and measured.
pub const TREE_ROW_FONT_PX: f32 = 14.0;

/// The tree rows' title weight (a selected row may raise it, as a Shows season
/// does; the size never changes).
pub const TREE_ROW_TITLE_WEIGHT: f32 = 400.0;

/// The text style a tree row's title `Truncate` box measures at: exactly the
/// size and weight the row paints.
#[must_use]
pub fn tree_row_text() -> TextStyle {
    TextStyle::sans(TREE_ROW_FONT_PX, TREE_ROW_TITLE_WEIGHT)
}
