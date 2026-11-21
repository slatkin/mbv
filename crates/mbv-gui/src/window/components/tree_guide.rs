//! The tree rows' indent guides (`gui-tree-indent-guides` design D1–D3, D5):
//! one vertical 1 px segment that a tree row paints, so an open parent hangs a
//! guide under its leading element through its whole subtree and the reader
//! sees where the parent's rows stop.
//!
//! The module paints one segment per row, never one line over a container: the
//! Music and Shows rows paint through the virtual lists, so only the rows in
//! view paint, and a container line would need the subtree's height and the
//! scroll window. A row inside an open subtree paints each ancestor's guide
//! from its own top to its own bottom, and an open parent that shows child rows
//! paints its own guide's start; the segments of adjacent rows meet, so the
//! guide reads as one line under any scroll. The Playlists tree follows the
//! same rule, so all three trees share this part.
//!
//! A row's guide span is `pointer-events-none absolute`, so the row it paints
//! in must be its containing block: a row that paints a guide carries the
//! `relative` class. Its tone names the parent's depth — [`GuideTone::Top`]
//! under a top-level parent (artist, series, playlist) and
//! [`GuideTone::Nested`] under a nested parent (album, season).

use std::collections::HashSet;

use dioxus_native::prelude::*;

use crate::window::components::library_item_row::{ROW_DIVIDER_PX, ROW_PADDING_PX, THUMB_WIDTH_PX};
use crate::window::library::list::GeometryRow;
use crate::window::theme::canvas_var;

/// A top-level parent's guide's left edge in the row box, in pixels (design
/// D2): under the thumb's centre, so it follows the row padding and the thumb.
pub const TOP_GUIDE_X: f32 = ROW_PADDING_PX + THUMB_WIDTH_PX / 2.0;

/// A nested parent's label inset, in pixels from the row box's left edge
/// (design D2): the album and season label clears the top-level guide that
/// passes through the row.
pub const NESTED_INSET_X: f32 = TOP_GUIDE_X + 16.0;

/// A nested parent's guide's left edge in the row box, in pixels (design D2):
/// under the label's first letter.
pub const NESTED_GUIDE_X: f32 = NESTED_INSET_X + 4.0;

/// A leaf row's inset — the track and episode number box — in pixels from the
/// row box's left edge (design D2): clear of both guides that pass through the
/// row.
pub const LEAF_INSET_X: f32 = NESTED_GUIDE_X + 8.0;

/// A playlist row's left padding, in pixels from the row box's left edge
/// (design D2): [`crate::window::components::playlists_panel`]'s row style
/// paints it, and the tree's items indent from it.
pub const PLAYLIST_LABEL_X: f32 = 8.0;

/// A playlist's guide's left edge in the row box, in pixels (design D2): a
/// playlist row has no thumb, so it hangs the guide under the label's first
/// letter, as a nested row does.
pub const PLAYLIST_GUIDE_X: f32 = PLAYLIST_LABEL_X + 4.0;

/// How far an open parent's guide clears its own row's bottom edge, in pixels
/// (design D3): the guide starts this far above the row's bottom, so the gap
/// under the parent's leading element reads as a start.
pub const GUIDE_START_INSET_PX: f32 = 6.0;

/// The tone a guide paints in (design D5): the depth of the parent row the
/// guide hangs under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuideTone {
    /// A guide under a top-level parent (artist, series, playlist): the
    /// neutral tone, which reads as a rule rather than as content.
    Top,
    /// A guide under a nested parent (album, season): the accent tone, so an
    /// open inner branch stands out.
    Nested,
}

impl GuideTone {
    /// The mark the guide span carries in `data-tree-guide` (design D1).
    #[must_use]
    pub fn mark(self) -> &'static str {
        match self {
            GuideTone::Top => "top",
            GuideTone::Nested => "nested",
        }
    }

    /// The canvas token the guide paints in (design D5).
    #[must_use]
    pub fn color(self) -> String {
        match self {
            GuideTone::Top => canvas_var("tree-guide"),
            GuideTone::Nested => canvas_var("tree-guide-nested"),
        }
    }
}

/// The props of the guide (design D1).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct TreeGuideProps {
    /// The guide's left edge, in pixels from the row box's left edge.
    pub x: f32,
    /// The tone, which names the guide's parent depth.
    pub tone: GuideTone,
    /// The guide's top edge, in pixels from the row's top: `0.0` on a row the
    /// guide passes through.
    pub top: f32,
    /// The guide's bottom edge, in pixels from the row's bottom: `0.0` to the
    /// row's edge, or negative on a top-level row, whose bottom divider the
    /// guide crosses (design D3).
    pub bottom: f32,
}

/// One row's guide segment (design D1): a pixel line at the parent's guide
/// position, from `top` down to `bottom`, over the row's own fill and hover
/// look and under its hint.
#[component]
pub fn TreeGuide(props: TreeGuideProps) -> Element {
    let style = format!(
        "left: {}px; top: {}px; bottom: {}px; width: 1px; background: {};",
        props.x,
        props.top,
        props.bottom,
        props.tone.color()
    );
    rsx! {
        span {
            class: "pointer-events-none absolute",
            style: "{style}",
            "data-tree-guide": props.tone.mark(),
        }
    }
}

/// The guides a tree leaf row paints (design D3): every ancestor's guide that
/// passes through the row, from its top to its bottom, so the segments of
/// adjacent rows meet. The two library trees carry both an artist or series
/// guide and an album or season guide, so a track and an episode row share
/// this paint.
pub fn leaf_guides() -> Element {
    rsx! {
        TreeGuide { x: TOP_GUIDE_X, tone: GuideTone::Top, top: 0.0, bottom: 0.0 }
        TreeGuide { x: NESTED_GUIDE_X, tone: GuideTone::Nested, top: 0.0, bottom: 0.0 }
    }
}

/// The guides a nested parent row paints (design D2, D3, D4): the top-level
/// ancestor's guide passes through the row, and while the row shows child rows
/// it starts its own under its label, so an album and a season row share this
/// paint.
pub fn nested_parent_guides(row_height: f32, shows_children: bool) -> Element {
    rsx! {
        TreeGuide { x: TOP_GUIDE_X, tone: GuideTone::Top, top: 0.0, bottom: 0.0 }
        if shows_children {
            TreeGuide {
                x: NESTED_GUIDE_X,
                tone: GuideTone::Nested,
                top: row_height - GUIDE_START_INSET_PX,
                bottom: 0.0,
            }
        }
    }
}

/// The guide an open parent row starts under its leading element (design D3):
/// `x` is the parent's guide position, `row_height` its painted height, and
/// `bottom` how far the segment reaches past the row's bottom edge — `0.0` to
/// the edge, negative where the row paints a divider the guide crosses.
pub fn parent_guide_start(x: f32, row_height: f32, bottom: f32) -> Element {
    rsx! {
        TreeGuide {
            x,
            tone: GuideTone::Top,
            top: row_height - GUIDE_START_INSET_PX,
            bottom,
        }
    }
}

/// The ids of the rows that close an open subtree: a descendant row whose next
/// row is no descendant, or that is the list's last. An open top-level parent
/// that shows children paints no divider under itself, so the guides do not
/// cross it; the subtree's last row paints the divider instead, and the
/// divider falls under the children.
pub fn subtree_tails<R: GeometryRow>(
    rows: &[R],
    is_descendant: impl Fn(&R) -> bool,
) -> HashSet<String> {
    rows.iter()
        .enumerate()
        .filter(|(index, row)| {
            is_descendant(row) && rows.get(index + 1).is_none_or(|next| !is_descendant(next))
        })
        .map(|(_, row)| row.id().to_owned())
        .collect()
}

/// The style a subtree's last row appends to take the divider the open parent
/// gave up: the shared row divider under the row, inside its height. Empty on
/// any other row.
#[must_use]
pub fn tail_divider_style(tail: bool) -> String {
    if tail {
        format!(
            " box-sizing: border-box; border-bottom: {ROW_DIVIDER_PX}px solid {};",
            canvas_var("border")
        )
    } else {
        String::new()
    }
}
