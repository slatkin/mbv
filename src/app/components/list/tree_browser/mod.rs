//! Destination-neutral embedded tree browsing.
//!
//! `TreeBrowser` owns stable-target tree state.  Destinations project their
//! domain data into [`TreeNode`] values and never see the private arena used by
//! the implementation.

mod types;

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::time::Instant;

use fuzzy_matcher::skim::SkimMatcherV2;
use ratatui::layout::{Position, Rect};
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::Component;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use crate::app::components::list::{
    AggregateMarkState, Cursored, Expandable, MarkSelection, MarkSelectionState, PagingPolicy,
    PaintRetained, PaintRetainedState, Row, RowFlow, Viewported,
};
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
    marks: MarkSelectionState<Target>,
    viewport_offset: usize,
    configured_geometry: Option<(Rect, Rect)>,
    filter_active: bool,
    filter_query: String,
    filter_anchor: Option<Target>,
    filter_matches: HashSet<usize>,
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
        self.model_revision = self.model_revision.saturating_add(1);
        self.selected = selected;
        self.expanded = expanded;
        self.marks.set_targets(marks);
        self.viewport_offset = self
            .viewport_offset
            .min(self.ordered_nodes.len().saturating_sub(1));
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

    pub fn is_expanded(&self, target: &Target) -> bool
    where
        Target: Eq + Hash,
    {
        self.expanded.contains(target)
    }

    pub fn marked_targets(&self) -> &[Target] {
        self.marks.targets()
    }

    pub fn marked_action_targets(&self) -> Vec<Target>
    where
        Target: Clone + Eq + Hash,
    {
        self.action_targets()
    }

    pub fn filter_active(&self) -> bool {
        self.filter_active
    }

    pub fn filter_query(&self) -> &str {
        &self.filter_query
    }

    pub fn search_bar(&self) -> Option<(String, bool)> {
        self.filter_active
            .then(|| (self.filter_query.clone(), false))
    }

    pub fn viewport_offset(&self) -> usize {
        self.viewport_offset
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

    fn invalidate_paint(&mut self) {
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
    fn filter_matches_for_query(&self) -> Vec<Target> {
        if !self.filter_active || self.filter_query.trim().is_empty() {
            return self
                .ordered_nodes
                .iter()
                .filter_map(|id| self.arena.get(id).map(|entry| entry.node.target.clone()))
                .collect();
        }
        let matcher = SkimMatcherV2::default().ignore_case();
        self.ordered_nodes
            .iter()
            .filter_map(|id| self.arena.get(id))
            .filter(|entry| {
                crate::app::fuzzy_match::word_match_score(
                    &matcher,
                    &entry.node.search_text,
                    &self.filter_query,
                )
                .is_some()
            })
            .map(|entry| entry.node.target.clone())
            .collect()
    }

    fn has_matching_descendant(&self, id: usize) -> bool {
        self.arena.get(&id).is_some_and(|entry| {
            entry.children.iter().any(|child| {
                self.filter_matches.contains(child) || self.has_matching_descendant(*child)
            })
        })
    }

    fn push_visible(&self, id: usize, visible: &mut Vec<usize>) {
        let Some(entry) = self.arena.get(&id) else {
            return;
        };
        let filtering = self.filter_active && !self.filter_query.trim().is_empty();
        let matched = self.filter_matches.contains(&id);
        let descendant_match = filtering && self.has_matching_descendant(id);
        if filtering && !matched && !descendant_match {
            return;
        }
        visible.push(id);
        if self.expanded.contains(&entry.node.target) || descendant_match {
            for child in &entry.children {
                self.push_visible(*child, visible);
            }
        }
    }

    fn visible_node_ids(&self) -> Vec<usize> {
        let mut visible = Vec::new();
        for id in &self.roots {
            self.push_visible(*id, &mut visible);
        }
        visible
    }

    fn current_flow(&self) -> RowFlow<Target> {
        RowFlow::new(
            self.visible_node_ids()
                .into_iter()
                .filter_map(|id| {
                    self.arena
                        .get(&id)
                        .map(|entry| Row::selectable(entry.node.target.clone()))
                })
                .collect(),
        )
    }

    fn reconcile_selection(&mut self) {
        let flow = self.current_flow();
        if Cursored::index(self, &flow).is_none() {
            Cursored::first(self, &flow);
        }
        let height = self
            .configured_geometry
            .map(|(_, content)| usize::from(content.height))
            .unwrap_or(1);
        Viewported::reconcile_viewport(self, &flow, height);
    }

    fn selected_summary(&self) -> TreeMarkSummary {
        TreeMarkSummary {
            marked_count: self
                .visible_node_ids()
                .into_iter()
                .filter_map(|id| self.arena.get(&id))
                .filter(|entry| self.marks.contains(&entry.node.target))
                .count(),
            has_partial_aggregate: self.visible_node_ids().into_iter().any(|id| {
                self.arena.get(&id).is_some_and(|entry| {
                    entry.node.mark_policy == TreeMarkPolicy::Aggregate
                        && self.aggregate_mark_state(&entry.node.target)
                            == AggregateMarkState::Partial
                })
            }),
        }
    }

    fn transition(
        &self,
        previous: Option<Target>,
        previous_marks: TreeMarkSummary,
        disposition: TreeConsumed,
        external_intent: Option<TreeExternalIntent<Target>>,
    ) -> TreeTransition<Target> {
        let current = self.selected.clone();
        let current_marks = self.selected_summary();
        TreeTransition {
            disposition,
            selected_target: current.clone(),
            selected_target_change: (previous != current)
                .then_some(TreeSelectionChange { previous, current }),
            mark_summary: Some(current_marks.clone()),
            mark_summary_change: (previous_marks != current_marks).then_some(current_marks),
            external_intent,
        }
    }

    fn filter_edit(&mut self, query: String) {
        if !self.filter_active {
            self.filter_anchor = self.selected.clone();
            self.filter_active = true;
        }
        self.filter_query = query;
        self.filter_matches = self
            .filter_matches_for_query()
            .into_iter()
            .filter_map(|target| self.target_to_node.get(&target).copied())
            .collect();
        self.reconcile_selection();
        self.invalidate_paint();
    }

    fn clear_filter(&mut self) {
        let anchor = self.filter_anchor.take();
        self.filter_active = false;
        self.filter_query.clear();
        self.filter_matches.clear();
        self.selected = anchor.filter(|target| self.target_to_node.contains_key(target));
        self.reconcile_selection();
        self.invalidate_paint();
    }

    /// Apply one complete semantic operation. All cursor, viewport, expansion,
    /// filter, mark, pointer, activation, and context mutations enter here.
    pub fn apply(&mut self, operation: TreeOperation<Target>) -> TreeTransition<Target> {
        let previous = self.selected.clone();
        let previous_marks = self.selected_summary();
        let mut disposition = TreeConsumed::Consumed;
        let mut external_intent = None;
        let flow = self.current_flow();
        match operation {
            TreeOperation::Move(delta) => {
                Cursored::move_by(
                    self,
                    &flow,
                    delta.clamp(isize::MIN as i64, isize::MAX as i64) as isize,
                );
                self.reconcile_selection();
            }
            TreeOperation::Page(direction) => {
                let height = self
                    .configured_geometry
                    .map_or(1, |(_, content)| usize::from(content.height));
                Viewported::page(
                    self,
                    &flow,
                    height,
                    direction.clamp(isize::MIN as i64, isize::MAX as i64) as isize,
                    PagingPolicy::visible_viewport(),
                );
            }
            TreeOperation::First => {
                Cursored::first(self, &flow);
                self.reconcile_selection();
            }
            TreeOperation::Last => {
                Cursored::last(self, &flow);
                self.reconcile_selection();
            }
            TreeOperation::Parent => {
                Expandable::select_parent(self, &flow);
                self.reconcile_selection();
            }
            TreeOperation::Child => {
                Expandable::select_first_child(self, &flow);
                self.reconcile_selection();
            }
            TreeOperation::ToggleExpansion => {
                if let Some(target) = self.selected.clone() {
                    if self
                        .children_of(&target)
                        .is_some_and(|children| !children.is_empty())
                    {
                        Expandable::toggle_expanded(self, &target);
                        self.reconcile_selection();
                        self.invalidate_paint();
                    } else {
                        disposition = TreeConsumed::Unhandled;
                    }
                } else {
                    disposition = TreeConsumed::Unhandled;
                }
            }
            TreeOperation::Select(target) => {
                if !Cursored::select_target(self, &flow, &target) {
                    disposition = TreeConsumed::Unhandled;
                }
                self.reconcile_selection();
            }
            TreeOperation::PointerSelect(point) => {
                let Some(target) = self.resolve_current_point(point).cloned() else {
                    disposition = TreeConsumed::Unhandled;
                    return self.transition(previous, previous_marks, disposition, external_intent);
                };
                if !Cursored::select_target(self, &flow, &target) {
                    disposition = TreeConsumed::Unhandled;
                }
                self.reconcile_selection();
            }
            TreeOperation::ToggleMark
            | TreeOperation::ToggleMarkTarget(_)
            | TreeOperation::PointerToggleMark(_) => {
                let target = match operation {
                    TreeOperation::ToggleMark => self.selected.clone(),
                    TreeOperation::ToggleMarkTarget(target) => Some(target),
                    TreeOperation::PointerToggleMark(point) => {
                        self.resolve_current_point(point).cloned()
                    }
                    _ => None,
                };
                if let Some(target) = target {
                    if self.toggle_mark_target(&target) {
                        self.selected = Some(target);
                    } else {
                        disposition = TreeConsumed::Unhandled;
                    }
                } else {
                    disposition = TreeConsumed::Unhandled;
                }
            }
            TreeOperation::Activate => {
                if let Some(target) = self.selected.clone() {
                    external_intent = Some(TreeExternalIntent::Activate(target));
                } else {
                    disposition = TreeConsumed::Unhandled;
                }
            }
            TreeOperation::ActivateTarget(target) => {
                if Cursored::select_target(self, &flow, &target) {
                    external_intent = Some(TreeExternalIntent::Activate(target));
                    self.reconcile_selection();
                } else {
                    disposition = TreeConsumed::Unhandled;
                }
            }
            TreeOperation::Context => {
                if let Some(target) = self.selected.clone() {
                    let targets = self.action_targets();
                    external_intent = Some(if targets.is_empty() {
                        TreeExternalIntent::Context(target)
                    } else {
                        TreeExternalIntent::ContextSelection(targets)
                    });
                } else {
                    disposition = TreeConsumed::Unhandled;
                }
            }
            TreeOperation::ContextTarget(target) => {
                if self.target_to_node.contains_key(&target) {
                    external_intent = Some(TreeExternalIntent::Context(target));
                } else {
                    disposition = TreeConsumed::Unhandled;
                }
            }
            TreeOperation::EditFilter(query) => self.filter_edit(query),
            TreeOperation::ClearFilter => self.clear_filter(),
        }
        self.invalidate_paint();
        self.transition(previous, previous_marks, disposition, external_intent)
    }

    fn toggle_mark_target(&mut self, target: &Target) -> bool {
        let Some(id) = self.target_to_node.get(target).copied() else {
            return false;
        };
        let policy = self.arena[&id].node.mark_policy;
        match policy {
            TreeMarkPolicy::Excluded => false,
            TreeMarkPolicy::Direct => {
                if self.marks.contains(target) {
                    MarkSelection::remove_mark(self, target);
                } else {
                    MarkSelection::add_mark(self, target.clone());
                }
                true
            }
            TreeMarkPolicy::Aggregate => {
                let children: Vec<Target> = self
                    .visible_descendants(id)
                    .into_iter()
                    .filter_map(|child| {
                        let entry = self.arena.get(&child)?;
                        (entry.node.mark_policy == TreeMarkPolicy::Direct)
                            .then_some(entry.node.target.clone())
                    })
                    .collect();
                if children.is_empty() {
                    return false;
                }
                let all_marked = children.iter().all(|child| self.marks.contains(child));
                for child in children {
                    if all_marked {
                        MarkSelection::remove_mark(self, &child);
                    } else {
                        MarkSelection::add_mark(self, child);
                    }
                }
                true
            }
        }
    }

    fn visible_descendants(&self, id: usize) -> Vec<usize> {
        let mut output = Vec::new();
        let mut stack = self
            .arena
            .get(&id)
            .map(|entry| entry.children.clone())
            .unwrap_or_default();
        while let Some(child) = stack.pop() {
            let filtering = self.filter_active && !self.filter_query.trim().is_empty();
            if !filtering
                || self.filter_matches.contains(&child)
                || self.has_matching_descendant(child)
            {
                output.push(child);
            }
            if let Some(entry) = self.arena.get(&child) {
                stack.extend(entry.children.iter().copied());
            }
        }
        output
    }

    fn action_targets(&self) -> Vec<Target> {
        self.visible_node_ids()
            .into_iter()
            .filter_map(|id| self.arena.get(&id))
            .filter(|entry| self.marks.contains(&entry.node.target))
            .map(|entry| entry.node.target.clone())
            .collect()
    }

    fn visible_rows(&self) -> Vec<TreePaintRow> {
        let mut rows = Vec::new();
        for id in self
            .visible_node_ids()
            .into_iter()
            .skip(self.viewport_offset)
        {
            let Some(entry) = self.arena.get(&id) else {
                continue;
            };
            let target = &entry.node.target;
            let marked = self.marks.contains(target);
            let aggregate_marked = entry.node.mark_policy == TreeMarkPolicy::Aggregate
                && matches!(
                    self.aggregate_mark_state(target),
                    AggregateMarkState::Marked | AggregateMarkState::Partial
                );
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
            .skip(self.viewport_offset)
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

impl<Target: Clone + Eq + Hash> Cursored<Target> for TreeBrowser<Target> {
    fn selected_target(&self) -> Option<&Target> {
        self.selected.as_ref()
    }

    fn set_selected_target(&mut self, target: Option<&Target>) {
        self.selected = target.cloned();
    }
}

impl<Target: Clone + Eq + Hash> Viewported<Target> for TreeBrowser<Target> {
    fn viewport_offset(&self) -> usize {
        self.viewport_offset
    }

    fn set_viewport_offset(&mut self, offset: usize) {
        self.viewport_offset = offset;
    }
}

impl<Target: Clone + Eq + Hash> MarkSelection<Target> for TreeBrowser<Target> {
    fn mark_selection(&self) -> &MarkSelectionState<Target> {
        &self.marks
    }

    fn mark_selection_mut(&mut self) -> &mut MarkSelectionState<Target> {
        &mut self.marks
    }
}

impl<Target: Clone + Eq + Hash> Expandable<Target> for TreeBrowser<Target> {
    fn is_expanded(&self, target: &Target) -> bool {
        self.expanded.contains(target)
    }

    fn set_expanded(&mut self, target: &Target, expanded: bool) {
        if !self.target_to_node.contains_key(target) {
            return;
        }
        if expanded {
            self.expanded.insert(target.clone());
        } else {
            self.expanded.remove(target);
        }
    }

    fn parent_target(&self, target: &Target) -> Option<&Target> {
        let id = *self.target_to_node.get(target)?;
        self.arena.get(&id)?.node.parent.as_ref()
    }

    fn child_targets(&self, target: &Target) -> Vec<&Target> {
        let Some(id) = self.target_to_node.get(target) else {
            return Vec::new();
        };
        self.arena
            .get(id)
            .map(|entry| {
                entry
                    .children
                    .iter()
                    .filter_map(|child| self.arena.get(child).map(|entry| &entry.node.target))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn aggregate_mark_state(&self, target: &Target) -> AggregateMarkState {
        let Some(id) = self.target_to_node.get(target).copied() else {
            return AggregateMarkState::Unmarked;
        };
        let direct: Vec<&Target> = self
            .visible_descendants(id)
            .into_iter()
            .filter_map(|child| self.arena.get(&child))
            .filter(|entry| entry.node.mark_policy == TreeMarkPolicy::Direct)
            .map(|entry| &entry.node.target)
            .collect();
        if direct.is_empty() {
            return AggregateMarkState::Unmarked;
        }
        let marked = direct
            .iter()
            .filter(|target| self.marks.contains(*target))
            .count();
        match marked {
            0 => AggregateMarkState::Unmarked,
            n if n == direct.len() => AggregateMarkState::Marked,
            _ => AggregateMarkState::Partial,
        }
    }
}

impl<Target: Clone + Eq + Hash> PaintRetained<Target> for TreeBrowser<Target> {
    fn paint_retained(&self) -> &PaintRetainedState<Target> {
        &self.paint
    }
    fn paint_retained_mut(&mut self) -> &mut PaintRetainedState<Target> {
        &mut self.paint
    }
}

impl<Target: Clone + Eq + Hash> Component for TreeBrowser<Target> {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let (claim_rect, content_rect) = self.configured_geometry.unwrap_or((area, area));
        self.reconcile_selection();
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
