//! Destination-neutral embedded tree browsing.
//!
//! `TreeBrowser` owns stable-target tree state.  Destinations project their
//! domain data into [`TreeNode`] values and never see the private arena used by
//! the implementation.

mod types;

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::time::Instant;

use ratatui::layout::{Position, Rect};
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::Component;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use crate::app::components::list::PaintRetainedState;
use crate::app::components::media_list::MediaSemanticState;

#[allow(unused_imports)]
pub use types::{
    TreeConsumed, TreeExternalIntent, TreeMarkPolicy, TreeMarkSummary, TreeNode, TreeOperation,
    TreeSelectionChange, TreeTrailing, TreeTransition,
};

/// A typed failure from an attempted tree projection replacement.
///
/// The error deliberately contains only stable targets.  Arena identifiers
/// are an implementation detail and never cross this boundary.
#[allow(dead_code)]
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

#[allow(dead_code)]
impl<Target> TreeReconciliationError<Target> {
    pub fn target(&self) -> &Target {
        match self {
            Self::DuplicateTarget { target }
            | Self::SelfParent { target }
            | Self::Cycle { target }
            | Self::MissingParent { target, .. } => target,
        }
    }
}

/// A row prepared for the destination-neutral render component.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TreePaintRow {
    pub(crate) title: String,
    pub(crate) trailing: Option<String>,
    pub(crate) depth: usize,
    pub(crate) root_index: usize,
    pub(crate) selected: bool,
    pub(crate) marked: bool,
    pub(crate) aggregate_marked: bool,
    pub(crate) semantic_state: MediaSemanticState,
}

#[allow(dead_code)]
#[derive(Clone)]
struct ArenaNode<Target> {
    node: TreeNode<Target>,
    children: Vec<usize>,
    depth: usize,
    root_index: usize,
}

/// The complete embedded owner for one nested stable-target row flow.
///
/// The arena and its identifiers are private.  In particular, callers can
/// only select, expand, mark, and resolve rows through destination targets.
#[allow(dead_code)]
pub struct TreeBrowser<Target> {
    arena: HashMap<usize, ArenaNode<Target>>,
    target_to_node: HashMap<Target, usize>,
    ordered_nodes: Vec<usize>,
    roots: Vec<usize>,
    next_node_id: usize,
    model_revision: u64,
    selected: Option<Target>,
    expanded: HashSet<Target>,
    marks: Vec<Target>,
    viewport_offset: usize,
    configured_geometry: Option<(Rect, Rect)>,
    paint: PaintRetainedState<Target>,
    focused: bool,
    marquee_text: String,
    marquee_started_at: Instant,
}

#[allow(dead_code)]
impl<Target> Default for TreeBrowser<Target> {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl<Target> TreeBrowser<Target> {
    pub fn new() -> Self {
        Self {
            arena: HashMap::new(),
            target_to_node: HashMap::new(),
            ordered_nodes: Vec::new(),
            roots: Vec::new(),
            next_node_id: 0,
            model_revision: 0,
            selected: None,
            expanded: HashSet::new(),
            marks: Vec::new(),
            viewport_offset: 0,
            configured_geometry: None,
            paint: PaintRetainedState::new(),
            focused: true,
            marquee_text: String::new(),
            marquee_started_at: Instant::now(),
        }
    }

    /// Replace the projected forest atomically.
    ///
    /// Validation completes before any owner state is changed.  Therefore a
    /// rejected projection leaves even retained paint geometry untouched.
    pub fn reconcile<I>(&mut self, projection: I) -> Result<(), TreeReconciliationError<Target>>
    where
        I: IntoIterator<Item = TreeNode<Target>>,
        Target: Clone + Eq + Hash,
    {
        let nodes: Vec<TreeNode<Target>> = projection.into_iter().collect();
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

        let content_changed = self.ordered_nodes.len() != nodes.len()
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
            .filter(|target| new_target_to_node.contains_key(*target))
            .cloned()
            .collect();
        let marks = self
            .marks
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
        self.model_revision = self.model_revision.saturating_add(1);
        self.selected = selected;
        self.expanded = expanded;
        self.marks = marks;
        self.viewport_offset = self
            .viewport_offset
            .min(self.ordered_nodes.len().saturating_sub(1));
        self.paint.invalidate();
        Ok(())
    }

    pub fn model_revision(&self) -> u64 {
        self.model_revision
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

    pub fn nodes(&self) -> impl Iterator<Item = &TreeNode<Target>> {
        self.ordered_nodes
            .iter()
            .filter_map(|id| self.arena.get(id).map(|entry| &entry.node))
    }

    pub fn expanded_targets(&self) -> impl Iterator<Item = &Target> {
        self.expanded.iter()
    }

    pub fn marked_targets(&self) -> &[Target] {
        &self.marks
    }

    pub fn viewport_offset(&self) -> usize {
        self.viewport_offset
    }

    pub fn set_geometry(&mut self, claim_rect: Rect, content_rect: Rect) {
        self.configured_geometry = Some((claim_rect, content_rect));
        self.paint.invalidate();
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
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

#[allow(dead_code)]
impl<Target: Clone + Eq + Hash> TreeBrowser<Target> {
    fn visible_node_ids(&self) -> Vec<usize> {
        let mut visible = Vec::new();
        let mut stack: Vec<usize> = self.roots.iter().rev().copied().collect();
        while let Some(id) = stack.pop() {
            let Some(entry) = self.arena.get(&id) else {
                continue;
            };
            visible.push(id);
            if self.expanded.contains(&entry.node.target) {
                stack.extend(entry.children.iter().rev().copied());
            }
        }
        visible
    }

    fn visible_rows(&self) -> Vec<TreePaintRow> {
        let mut rows = Vec::new();
        for id in self.visible_node_ids() {
            let Some(entry) = self.arena.get(&id) else {
                continue;
            };
            let target = &entry.node.target;
            let marked = self.marks.iter().any(|mark| mark == target);
            let aggregate_marked = entry.node.mark_policy == TreeMarkPolicy::Aggregate
                && entry.children.iter().any(|child| {
                    self.arena.get(child).is_some_and(|child| {
                        self.marks.iter().any(|mark| mark == &child.node.target)
                    })
                });
            rows.push(TreePaintRow {
                title: entry.node.title.clone(),
                trailing: entry
                    .node
                    .trailing
                    .as_ref()
                    .map(|trailing| trailing.text.clone()),
                depth: entry.depth,
                root_index: entry.root_index,
                selected: self.selected.as_ref() == Some(target),
                marked,
                aggregate_marked,
                semantic_state: entry.node.semantic_state.clone(),
            });
        }
        rows
    }

    fn retained_rows(&self, area: Rect) -> Vec<(Rect, Target)> {
        self.visible_node_ids()
            .into_iter()
            .enumerate()
            .filter_map(|(index, id)| {
                let y = area.y.checked_add(index as u16)?;
                (y < area.bottom()).then(|| {
                    self.arena.get(&id).map(|entry| {
                        (
                            Rect::new(area.x, y, area.width, 1),
                            entry.node.target.clone(),
                        )
                    })
                })?
            })
            .collect()
    }
}

impl<Target: Clone + Eq + Hash> Component for TreeBrowser<Target> {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let (claim_rect, content_rect) = self.configured_geometry.unwrap_or((area, area));
        self.paint.begin();
        let rows = self.visible_rows();
        crate::app::render::render_tree_browser(
            frame,
            claim_rect,
            &rows,
            self.focused,
            &mut self.marquee_text,
            &mut self.marquee_started_at,
        );
        let selected_row = rows
            .iter()
            .position(|row| row.selected)
            .and_then(|index| claim_rect.y.checked_add(index as u16))
            .map(|y| Rect::new(claim_rect.x, y, claim_rect.width, 1));
        self.paint.store_completed(
            claim_rect,
            content_rect,
            self.viewport_offset,
            self.retained_rows(claim_rect),
            selected_row,
        );
    }

    fn query<'a>(&'a self, _attr: Attribute) -> Option<QueryResult<'a>> {
        None
    }

    fn attr(&mut self, _attr: Attribute, _value: AttrValue) {}

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

#[cfg(test)]
mod tests;
