//! The two tree row text looks (user request 2026-11-21, 2026-11-24): every tree
//! row in the window — a Playlists playlist or item, a Music album or track, a
//! Shows season or episode, and a top-level tree parent row — paints the look
//! its depth names, and every one of their `Truncate` boxes measures at that
//! look. A parent row (a playlist, an artist, an album, a series, a season)
//! paints the sans face at [`TREE_ROW_FONT_PX`]; a leaf row (a track, an
//! episode, a playlist item) paints the mono face at [`LEAF_ROW_FONT_PX`] with
//! its own label inset. The row boxes differ (height, inset, fill, guides), but
//! a leaf and a parent cannot be born at another look, and a change here lands
//! in every tree at once.
//!
//! Painted and measured type are one number. A parent row box's inline style
//! carries [`TREE_ROW_FONT_PX`] so the title inherits it, and the row painter
//! hands the `Truncate` box the same size and weight. A leaf row's label sits
//! in a wrapper that carries [`leaf_label_css`], and its `Truncate` box
//! measures the same mono face, so a cut lands where the paint shows it.

/// The parent tree rows' font size, in CSS pixels: one value for every parent
/// row in the window, painted and measured.
pub const TREE_ROW_FONT_PX: f32 = 14.0;

/// The leaf tree rows' font size, in CSS pixels: the mono face's size on a
/// track, an episode, and a playlist item, painted and measured.
pub const LEAF_ROW_FONT_PX: f32 = 13.0;

/// The leaf tree rows' font weight, painted and measured.
pub const LEAF_ROW_FONT_WEIGHT: f32 = 500.0;

/// The leaf label's inset, as its `padding` shorthand (user request
/// 2026-11-24): it sits on the label's wrapper, so the row box keeps the tree's
/// indent and the measured box keeps its whole width.
const LEAF_LABEL_PADDING: &str = "2px 6px";

/// The leaf label wrapper's inline style: the label inset and the mono type the
/// `Truncate` box inside it inherits. The inset sits on the wrapper, never on
/// the measuring box: that box's width is the room its text paints in, so
/// padding on it would cut the text 12 px too long and clip the ellipsis. A leaf
/// painter appends its own colour to the box's own style.
#[must_use]
pub fn leaf_label_css() -> String {
    format!(
        "padding: {LEAF_LABEL_PADDING}; font-family: var(--font-mono); font-size: {LEAF_ROW_FONT_PX}px; font-weight: {LEAF_ROW_FONT_WEIGHT};"
    )
}
