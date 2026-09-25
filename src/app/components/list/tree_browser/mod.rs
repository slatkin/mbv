//! Destination-neutral embedded tree browsing.
//!
//! `TreeBrowser` owns stable-target tree state.  Destinations project their
//! domain data into [`TreeNode`] values and never see the private arena used by
//! the implementation.

mod operations;
mod render;
mod types;

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::time::Instant;

use ratatui::layout::{Position, Rect};

use crate::app::components::list::{MarkSelectionState, PaintRetainedState};
use crate::app::components::media_list::MediaSemanticState;

pub use types::{
    TreeConsumed, TreeEntry, TreeExternalIntent, TreeMarkPolicy, TreeMarkSummary, TreeNode,
    TreeOperation, TreeSelectionChange, TreeTitleRole, TreeTransition,
};

/// A typed failure from an attempted tree projection replacement.
///
/// The error deliberately contains only stable targets.  Arena identifiers
/// are an implementation detail and never cross this boundary.
#[derive(Clone, PartialEq, Eq)]
pub enum TreeReconciliationError<Target> {
    DuplicateTarget { target: Target },
    MissingParent { target: Target, parent: Target },
    SelfParent { target: Target },
    Cycle { target: Target },
}

impl<Target> std::fmt::Debug for TreeReconciliationError<Target> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::DuplicateTarget { .. } => "DuplicateTarget",
            Self::MissingParent { .. } => "MissingParent",
            Self::SelfParent { .. } => "SelfParent",
            Self::Cycle { .. } => "Cycle",
        })
    }
}

/// The derived aggregate-mark state of one aggregate-policy row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TreeAggregateMark {
    /// No visible direct descendant is marked.
    None,
    /// Some, but not all, visible direct descendants are marked.
    Partial,
    /// Every visible direct descendant is marked.
    Full,
}

/// A row prepared for the destination-neutral render component.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TreePaintRowKind {
    Node,
    Heading,
    Spacer,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TreePaintRow {
    pub(crate) kind: TreePaintRowKind,
    pub(crate) title: String,
    pub(crate) title_role: TreeTitleRole,
    pub(crate) trailing: Option<String>,
    pub(crate) depth: usize,
    pub(crate) root_index: usize,
    pub(crate) group_root_index: usize,
    pub(crate) zebra_striped: bool,
    pub(crate) selected: bool,
    pub(crate) marked: bool,
    pub(crate) aggregate_mark: TreeAggregateMark,
    pub(crate) semantic_state: MediaSemanticState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum StructuralRow {
    Heading(String),
    Spacer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum VisibleRow {
    Node(usize),
    Structural(usize, usize),
}

#[derive(Clone)]
pub(super) struct ArenaNode<Target> {
    pub(super) node: TreeNode<Target>,
    pub(super) children: Vec<usize>,
    pub(super) depth: usize,
    pub(super) root_index: usize,
}

/// The complete embedded owner for one nested stable-target row flow.
///
/// The arena and its identifiers are private.  In particular, callers can
/// only select, expand, mark, and resolve rows through destination targets.
pub struct TreeBrowser<Target> {
    pub(super) arena: HashMap<usize, ArenaNode<Target>>,
    pub(super) target_to_node: HashMap<Target, usize>,
    pub(super) ordered_nodes: Vec<usize>,
    pub(super) roots: Vec<usize>,
    pub(super) root_structures: HashMap<Target, Vec<StructuralRow>>,
    pub(super) next_node_id: usize,
    pub(super) model_revision: u64,
    pub(super) selected: Option<Target>,
    pub(super) expanded: HashSet<Target>,
    pub(super) marks: MarkSelectionState<Target>,
    pub(super) viewport_offset: usize,
    pub(super) configured_geometry: Option<(Rect, Rect)>,
    pub(super) filter_active: bool,
    pub(super) filter_query: String,
    pub(super) filter_anchor: Option<Target>,
    pub(super) filter_matches: HashSet<usize>,
    pub(super) paint: PaintRetainedState<Target>,
    pub(super) focused: bool,
    pub(super) marquee_text: String,
    pub(super) marquee_started_at: Instant,
    /// The area of the latest completed `view`, the paging viewport fallback
    /// when the panel has not declared geometry yet.
    pub(super) last_painted: Option<Rect>,
}

impl<Target> Default for TreeBrowser<Target> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Target> TreeBrowser<Target> {
    pub fn new() -> Self {
        Self {
            arena: HashMap::new(),
            target_to_node: HashMap::new(),
            ordered_nodes: Vec::new(),
            roots: Vec::new(),
            root_structures: HashMap::new(),
            next_node_id: 0,
            model_revision: 0,
            selected: None,
            expanded: HashSet::new(),
            marks: MarkSelectionState::new(),
            viewport_offset: 0,
            configured_geometry: None,
            filter_active: false,
            filter_query: String::new(),
            filter_anchor: None,
            filter_matches: HashSet::new(),
            paint: PaintRetainedState::new(),
            focused: true,
            marquee_text: String::new(),
            marquee_started_at: Instant::now(),
            last_painted: None,
        }
    }

    /// Replace the projected forest atomically.
    ///
    /// Validation completes before any owner state is changed.  Therefore a
    /// rejected projection leaves even retained paint geometry untouched.
    pub fn reconcile<I>(&mut self, projection: I) -> Result<(), TreeReconciliationError<Target>>
    where
        I: IntoIterator,
        I::Item: Into<TreeEntry<Target>>,
        Target: Clone + Eq + Hash,
    {
        let entries: Vec<TreeEntry<Target>> = projection.into_iter().map(Into::into).collect();
        let nodes: Vec<TreeNode<Target>> = entries
            .iter()
            .filter_map(|entry| match entry {
                TreeEntry::Node(node) => Some(node.clone()),
                TreeEntry::Heading(_) | TreeEntry::Spacer => None,
            })
            .collect();
        let mut target_to_index = HashMap::with_capacity(nodes.len());
        for (index, node) in nodes.iter().enumerate() {
            if target_to_index.insert(node.target.clone(), index).is_some() {
                return Err(TreeReconciliationError::DuplicateTarget {
                    target: node.target.clone(),
                });
            }
        }

        for node in &nodes {
            if let Some(parent) = &node.parent {
                if parent == &node.target {
                    return Err(TreeReconciliationError::SelfParent {
                        target: node.target.clone(),
                    });
                }
                if !target_to_index.contains_key(parent) {
                    return Err(TreeReconciliationError::MissingParent {
                        target: node.target.clone(),
                        parent: parent.clone(),
                    });
                }
            }
        }

        // Following each parent chain is sufficient for a forest and avoids
        // mutating the current owner while cycle validation is in progress.
        for node in &nodes {
            let mut seen = HashSet::new();
            let mut current = Some(node.target.clone());
            while let Some(target) = current {
                if !seen.insert(target.clone()) {
                    return Err(TreeReconciliationError::Cycle { target });
                }
                current = target_to_index
                    .get(&target)
                    .and_then(|&index| nodes[index].parent.clone());
            }
        }

        let mut root_structures = HashMap::new();
        let mut pending_structure = Vec::new();
        for entry in &entries {
            match entry {
                TreeEntry::Heading(title) => {
                    pending_structure.push(StructuralRow::Heading(title.clone()));
                }
                TreeEntry::Spacer => pending_structure.push(StructuralRow::Spacer),
                TreeEntry::Node(node) if node.parent.is_none() => {
                    if !pending_structure.is_empty() {
                        root_structures
                            .insert(node.target.clone(), std::mem::take(&mut pending_structure));
                    }
                }
                TreeEntry::Node(_) => {}
            }
        }

        let content_changed = self.ordered_nodes.len() != nodes.len()
            || self.root_structures != root_structures
            || self
                .ordered_nodes
                .iter()
                .zip(nodes.iter())
                .any(|(id, node)| self.arena.get(id).is_none_or(|entry| entry.node != *node));
        if !content_changed {
            return Ok(());
        }

        let mut arena = HashMap::with_capacity(nodes.len());
        let mut new_target_to_node = HashMap::with_capacity(nodes.len());
        let mut ordered_nodes = Vec::with_capacity(nodes.len());
        let mut roots = Vec::new();
        let mut ids_by_index = Vec::with_capacity(nodes.len());

        for node in nodes {
            let id = self.next_node_id;
            self.next_node_id = self.next_node_id.saturating_add(1);
            ids_by_index.push(id);
            new_target_to_node.insert(node.target.clone(), id);
            ordered_nodes.push(id);
            arena.insert(
                id,
                ArenaNode {
                    node,
                    children: Vec::new(),
                    depth: 0,
                    root_index: 0,
                },
            );
        }

        // Build parent links by the already validated stable-target map.
        for id in ids_by_index.iter().copied() {
            let parent = arena.get(&id).and_then(|entry| entry.node.parent.clone());
            if let Some(parent) = parent {
                let parent_id = new_target_to_node[&parent];
                arena
                    .get_mut(&parent_id)
                    .expect("validated parent")
                    .children
                    .push(id);
            } else {
                roots.push(id);
            }
        }

        // Compute depth and group-relative stripe phase from the roots.  This
        // is derived data and does not expose the private node identifiers.
        let mut stack: Vec<(usize, usize, usize)> = roots
            .iter()
            .copied()
            .enumerate()
            .map(|(root_index, id)| (id, 0, root_index))
            .collect();
        while let Some((id, depth, root_index)) = stack.pop() {
            if let Some(entry) = arena.get_mut(&id) {
                entry.depth = depth;
                entry.root_index = root_index;
                for child in entry.children.iter().rev().copied() {
                    stack.push((child, depth + 1, root_index));
                }
            }
        }

        let selected = self
            .selected
            .as_ref()
            .filter(|target| new_target_to_node.contains_key(*target))
            .cloned()
            .or_else(|| {
                ordered_nodes
                    .first()
                    .map(|id| arena[id].node.target.clone())
            });
        let expanded = self
            .expanded
            .iter()
            .filter_map(|target| {
                let id = *new_target_to_node.get(target)?;
                let node = &arena[&id];
                (node.node.expandable || !node.children.is_empty()).then(|| target.clone())
            })
            .collect();
        let marks: Vec<Target> = self
            .marks
            .targets()
            .iter()
            .filter(|target| new_target_to_node.contains_key(*target))
            .filter(|target| {
                let id = new_target_to_node[*target];
                arena[&id].node.mark_policy == TreeMarkPolicy::Direct
            })
            .cloned()
            .collect();

        self.arena = arena;
        self.target_to_node = new_target_to_node;
        self.ordered_nodes = ordered_nodes;
        self.roots = roots;
        self.root_structures = root_structures;
        self.model_revision = self.model_revision.saturating_add(1);
        self.selected = selected;
        self.expanded = expanded;
        self.marks.set_targets(marks);
        self.viewport_offset = self
            .viewport_offset
            .min(self.visible_len().saturating_sub(1));
        self.filter_matches.clear();
        self.filter_matches.extend(
            self.filter_matches_for_query()
                .into_iter()
                .filter_map(|target| self.target_to_node.get(&target).copied()),
        );
        self.paint.invalidate();
        self.reconcile_selection();
        Ok(())
    }

    pub fn selected_target(&self) -> Option<&Target> {
        self.selected.as_ref()
    }

    pub fn roots(&self) -> Vec<&Target> {
        self.roots
            .iter()
            .filter_map(|id| self.arena.get(id).map(|entry| &entry.node.target))
            .collect()
    }

    pub fn children_of(&self, target: &Target) -> Option<Vec<&Target>>
    where
        Target: Eq + Hash,
    {
        let id = *self.target_to_node.get(target)?;
        Some(
            self.arena[&id]
                .children
                .iter()
                .filter_map(|child| self.arena.get(child).map(|entry| &entry.node.target))
                .collect(),
        )
    }

    pub fn node(&self, target: &Target) -> Option<&TreeNode<Target>>
    where
        Target: Eq + Hash,
    {
        self.target_to_node
            .get(target)
            .and_then(|id| self.arena.get(id))
            .map(|entry| &entry.node)
    }

    pub fn is_expanded(&self, target: &Target) -> bool
    where
        Target: Eq + Hash,
    {
        self.expanded.contains(target)
    }

    pub fn marked_targets(&self) -> &[Target] {
        self.marks.targets()
    }

    /// The current visible row flow's stable targets in projection order.
    /// This is a read-only stable-target query: destinations translate their
    /// own windows and scopes over it, never over positions or internals.
    pub fn visible_targets(&self) -> Vec<Target>
    where
        Target: Clone + Eq + Hash,
    {
        self.visible_node_ids()
            .into_iter()
            .filter_map(|id| self.arena.get(&id).map(|entry| entry.node.target.clone()))
            .collect()
    }

    /// Whether a completed frame is currently retained (the latest-render
    /// pointer contract's validity read).
    pub fn has_completed_paint(&self) -> bool {
        self.paint.is_valid()
    }

    /// The `AnchorSelection` operation's implementation: select `target`,
    /// revealing its ancestor path, and anchor the viewport at the persisted
    /// fully-expanded flow `offset`. The persisted value names a row in the
    /// complete settled order, not the current projection, so the anchor
    /// rounds forward to the next visible row instead of parking the viewport
    /// on a hidden one. A target the owner does not hold is an explicit
    /// absent result.
    fn anchor_selection_to(&mut self, target: &Target, flow_offset: usize) -> bool
    where
        Target: Clone + Eq + Hash,
    {
        let Some(id) = self.target_to_node.get(target).copied() else {
            return false;
        };
        // Reveal the target's ancestor path without touching any other
        // branch's expansion.
        let mut cursor = id;
        while let Some(parent) = self.arena[&cursor].node.parent.clone() {
            self.expanded.insert(parent.clone());
            cursor = self.target_to_node[&parent];
        }
        self.selected = Some(target.clone());
        if let Some(row) = self.visible_row_for_flow_offset(flow_offset) {
            self.viewport_offset = row;
        }
        // The next view reconciles the viewport against its real height, the
        // same deferred keep-visible rule a Panel-driven restore follows.
        self.invalidate_paint();
        true
    }

    /// The visible row at or after a persisted fully-expanded flow offset.
    /// The walk follows the complete settled tree, independent of expansion.
    fn visible_row_for_flow_offset(&self, offset: usize) -> Option<usize>
    where
        Target: Clone + Eq + Hash,
    {
        let visible = self.visible_flow_rows();
        let mut settled = Vec::new();
        for &root in &self.roots {
            if let Some(entry) = self.arena.get(&root) {
                if let Some(structures) = self.root_structures.get(&entry.node.target) {
                    settled.extend(
                        structures
                            .iter()
                            .enumerate()
                            .map(|(index, _)| VisibleRow::Structural(root, index)),
                    );
                }
            }
            let mut stack = vec![root];
            while let Some(id) = stack.pop() {
                settled.push(VisibleRow::Node(id));
                if let Some(entry) = self.arena.get(&id) {
                    stack.extend(entry.children.iter().rev().copied());
                }
            }
        }
        settled
            .into_iter()
            .skip(offset)
            .find_map(|row| visible.iter().position(|visible_row| *visible_row == row))
    }

    pub fn filter_active(&self) -> bool {
        self.filter_active
    }

    pub fn search_bar(&self) -> Option<(String, bool)> {
        self.filter_active
            .then(|| (self.filter_query.clone(), false))
    }

    /// Clamp the retained flow offset to a Panel-provided viewport without
    /// changing the selected stable target.
    pub fn clamp_viewport_to(&mut self, viewport_height: usize)
    where
        Target: Clone + Eq + Hash,
    {
        let max_offset = self.visible_len().saturating_sub(viewport_height);
        self.viewport_offset = self.viewport_offset.min(max_offset);
        self.invalidate_paint();
    }

    pub fn selected_row_rect(&self) -> Option<Rect> {
        self.paint.selected_row_rect()
    }

    pub fn set_geometry(&mut self, claim_rect: Rect, content_rect: Rect)
    where
        Target: Clone + Eq + Hash,
    {
        if self.configured_geometry != Some((claim_rect, content_rect)) {
            self.configured_geometry = Some((claim_rect, content_rect));
            self.reconcile_selection();
        }
        self.paint.invalidate();
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    #[cfg(test)]
    pub(in crate::app) fn set_marquee_started_at(&mut self, text: &str, at: Instant) {
        self.marquee_text.clear();
        self.marquee_text.push_str(text);
        self.marquee_started_at = at;
    }

    pub fn invalidate_paint(&mut self) {
        self.paint.invalidate();
    }

    pub fn claims_current_point(&self, point: Position) -> bool {
        self.paint.claims_point(point)
    }

    pub fn resolve_current_point(&self, point: Position) -> Option<&Target> {
        self.paint.resolve_point(point)
    }
}

#[cfg(test)]
mod tests;
