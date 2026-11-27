//! The tree rows' rail and Branch swatch (`gui-tree-row-roles` design D1, D2,
//! D4, D6): one vertical 3 px span at a row's left edge that tells the reader
//! which group the row belongs to, and the token pick for the flat colour
//! square that tells a branch row apart from its siblings.
//!
//! An open root paints the bright [`RailTone::Open`] rail at the list's left
//! edge, and the rows inside its subtree paint the muted [`RailTone::Group`]
//! rail. The rail paints one segment per row, never one line over a
//! container: the Music and Shows rows paint through the virtual lists, so
//! only the rows in view paint, and a container line would need the group's
//! height and the scroll window. The segments of adjacent rows meet, so the
//! rail reads as one line under any scroll. The Playlists tree follows the
//! same rule, so all three trees share this part.
//!
//! A row's rail span is `pointer-events-none absolute`, so the row it paints
//! in must be its containing block: a row that paints a rail carries the
//! `relative` class.
//!
//! The module also holds the ids of the rows that close an open subtree
//! (moved from `tree_guide.rs`): the group's last row paints the root's
//! divider instead of the open root, so the divider falls under the children.

use std::collections::HashSet;

use dioxus_native::prelude::*;

use crate::window::library::list::GeometryRow;
use crate::window::theme::canvas_var;

/// The rail's width, in logical pixels (design D6): the mockup's 3 px span.
pub const RAIL_WIDTH_PX: f32 = 3.0;

/// The open root rail's canvas token name (design D6).
const RAIL_OPEN_TOKEN: &str = "tree-rail";

/// The group rail's canvas token name (design D6).
const RAIL_GROUP_TOKEN: &str = "tree-rail-muted";

/// The tone a rail paints in (design D1): the open root's bright rail, or the
/// muted rail of the rows inside its subtree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RailTone {
    /// The rail an open root row paints at the list's left edge: the bright
    /// accent tone, so the open tree reads at a glance.
    Open,
    /// The rail every row of an open subtree paints: the muted tone over the
    /// group fill.
    Group,
}

impl RailTone {
    /// The mark the rail span carries in `data-tree-rail` (design D9).
    #[must_use]
    pub fn mark(self) -> &'static str {
        match self {
            RailTone::Open => "open",
            RailTone::Group => "group",
        }
    }

    /// The canvas token the rail paints in (design D6).
    #[must_use]
    pub fn color(self) -> String {
        match self {
            RailTone::Open => canvas_var(RAIL_OPEN_TOKEN),
            RailTone::Group => canvas_var(RAIL_GROUP_TOKEN),
        }
    }
}

/// The props of the rail (design D1).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct TreeRailProps {
    /// The tone, which names the rail's role.
    pub tone: RailTone,
}

/// One row's rail segment (design D1, D2): a 3 px span at the row box's left
/// edge, from the row's top to its bottom, over the row's own fill and hover
/// look and under its content.
#[component]
pub fn TreeRail(props: TreeRailProps) -> Element {
    let style = format!(
        "left: 0px; top: 0px; bottom: 0px; width: {RAIL_WIDTH_PX}px; background: {};",
        props.tone.color()
    );
    rsx! {
        span {
            class: "pointer-events-none absolute",
            style: "{style}",
            "data-tree-rail": props.tone.mark(),
        }
    }
}

/// The five Branch swatch canvas token names, in sibling order (design D6).
pub const SWATCH_TOKENS: [&str; 5] = [
    "tree-swatch-1",
    "tree-swatch-2",
    "tree-swatch-3",
    "tree-swatch-4",
    "tree-swatch-5",
];

/// The swatch a branch row paints for its position among the painted siblings
/// (design D4): swatches 1 to 5 over the first five siblings, then the cycle
/// restarts at swatch 2, so orange is never followed by red.
#[must_use]
pub fn swatch_var(sibling: usize) -> String {
    let index = if sibling < SWATCH_TOKENS.len() {
        sibling
    } else {
        1 + (sibling - SWATCH_TOKENS.len()) % (SWATCH_TOKENS.len() - 1)
    };
    canvas_var(SWATCH_TOKENS[index])
}

/// The ids of the rows that close an open subtree: a descendant row whose next
/// row is no descendant, or that is the list's last. An open top-level parent
/// that shows children paints no divider under itself, so the rails do not
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

#[cfg(test)]
mod tests;
