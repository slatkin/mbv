//! Semantic tree operations and the private adapter for shared list arithmetic.

use std::hash::Hash;

use fuzzy_matcher::skim::SkimMatcherV2;
use ratatui::layout::Rect;

use crate::app::components::list::{
    AggregateMarkState, Cursored, Expandable, MarkSelection, MarkSelectionState, PagingPolicy, Row,
    RowFlow, Viewported,
};

use super::{
    TreeBrowser, TreeConsumed, TreeExternalIntent, TreeMarkPolicy, TreeMarkSummary, TreePaintRow,
    TreeSelectionChange, TreeTransition,
};

/// The shared list traits are implemented only for this private adapter.
/// Destinations therefore cannot invoke their mutating default methods on the
/// public tree owner; semantic mutation enters through `TreeBrowser::apply`.
struct TreeState<'a, Target> {
    browser: &'a mut TreeBrowser<Target>,
}

impl<Target: Clone + Eq + Hash> Cursored<Target> for TreeState<'_, Target> {
    fn selected_target(&self) -> Option<&Target> {
        self.browser.selected.as_ref()
    }

    fn set_selected_target(&mut self, target: Option<&Target>) {
        self.browser.selected = target.cloned();
    }
}

impl<Target: Clone + Eq + Hash> Viewported<Target> for TreeState<'_, Target> {
    fn viewport_offset(&self) -> usize {
        self.browser.viewport_offset
    }

    fn set_viewport_offset(&mut self, offset: usize) {
        self.browser.viewport_offset = offset;
    }
}

impl<Target: Clone + Eq + Hash> MarkSelection<Target> for TreeState<'_, Target> {
    fn mark_selection(&self) -> &MarkSelectionState<Target> {
        &self.browser.marks
    }

    fn mark_selection_mut(&mut self) -> &mut MarkSelectionState<Target> {
        &mut self.browser.marks
    }
}

impl<Target: Clone + Eq + Hash> Expandable<Target> for TreeState<'_, Target> {
    fn is_expanded(&self, target: &Target) -> bool {
        self.browser.expanded.contains(target)
    }

    fn set_expanded(&mut self, target: &Target, expanded: bool) {
        if !self.browser.target_to_node.contains_key(target) {
            return;
        }
        if expanded {
            self.browser.expanded.insert(target.clone());
        } else {
            self.browser.expanded.remove(target);
        }
    }

    fn parent_target(&self, target: &Target) -> Option<&Target> {
        let id = *self.browser.target_to_node.get(target)?;
        self.browser.arena.get(&id)?.node.parent.as_ref()
    }

    fn child_targets(&self, target: &Target) -> Vec<&Target> {
        let Some(id) = self.browser.target_to_node.get(target) else {
            return Vec::new();
        };
        self.browser
            .arena
            .get(id)
            .map(|entry| {
                entry
                    .children
                    .iter()
                    .filter_map(|child| {
                        self.browser
                            .arena
                            .get(child)
                            .map(|entry| &entry.node.target)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn aggregate_mark_state(&self, target: &Target) -> AggregateMarkState {
        self.browser.aggregate_mark_state_for(target)
    }
}

#[allow(dead_code)]
impl<Target: Clone + Eq + Hash> TreeBrowser<Target> {
    fn with_state<R>(&mut self, action: impl FnOnce(&mut TreeState<'_, Target>) -> R) -> R {
        action(&mut TreeState { browser: self })
    }

    pub(super) fn filter_matches_for_query(&self) -> Vec<Target> {
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

    pub(super) fn visible_node_ids(&self) -> Vec<usize> {
        let mut visible = Vec::new();
        for id in &self.roots {
            self.push_visible(*id, &mut visible);
        }
        visible
    }

    pub(super) fn current_flow(&self) -> RowFlow<Target> {
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

    pub(super) fn reconcile_selection(&mut self) {
        let flow = self.current_flow();
        if self
            .with_state(|state| Cursored::index(state, &flow))
            .is_none()
        {
            self.with_state(|state| Cursored::first(state, &flow));
        }
        let height = self
            .configured_geometry
            .map(|(_, content)| usize::from(content.height))
            .unwrap_or(1);
        self.with_state(|state| Viewported::reconcile_viewport(state, &flow, height));
    }

    fn aggregate_mark_state_for(&self, target: &Target) -> AggregateMarkState {
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
                        && self.aggregate_mark_state_for(&entry.node.target)
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
    pub fn apply(&mut self, operation: super::TreeOperation<Target>) -> TreeTransition<Target> {
        let previous = self.selected.clone();
        let previous_marks = self.selected_summary();
        let mut disposition = TreeConsumed::Consumed;
        let mut external_intent = None;
        let flow = self.current_flow();
        match operation {
            super::TreeOperation::Move(delta) => {
                self.with_state(|state| {
                    Cursored::move_by(
                        state,
                        &flow,
                        delta.clamp(isize::MIN as i64, isize::MAX as i64) as isize,
                    )
                });
                self.reconcile_selection();
            }
            super::TreeOperation::Page(direction) => {
                let height = self
                    .configured_geometry
                    .map_or(1, |(_, content)| usize::from(content.height));
                self.with_state(|state| {
                    Viewported::page(
                        state,
                        &flow,
                        height,
                        direction.clamp(isize::MIN as i64, isize::MAX as i64) as isize,
                        PagingPolicy::visible_viewport(),
                    )
                });
            }
            super::TreeOperation::First => {
                self.with_state(|state| Cursored::first(state, &flow));
                self.reconcile_selection();
            }
            super::TreeOperation::Last => {
                self.with_state(|state| Cursored::last(state, &flow));
                self.reconcile_selection();
            }
            super::TreeOperation::Parent => {
                self.with_state(|state| Expandable::select_parent(state, &flow));
                self.reconcile_selection();
            }
            super::TreeOperation::Child => {
                self.with_state(|state| Expandable::select_first_child(state, &flow));
                self.reconcile_selection();
            }
            super::TreeOperation::ToggleExpansion => {
                if let Some(target) = self.selected.clone() {
                    if self
                        .children_of(&target)
                        .is_some_and(|children| !children.is_empty())
                    {
                        self.with_state(|state| Expandable::toggle_expanded(state, &target));
                        self.reconcile_selection();
                        self.invalidate_paint();
                    } else {
                        disposition = TreeConsumed::Unhandled;
                    }
                } else {
                    disposition = TreeConsumed::Unhandled;
                }
            }
            super::TreeOperation::Select(target) => {
                if !self.with_state(|state| Cursored::select_target(state, &flow, &target)) {
                    disposition = TreeConsumed::Unhandled;
                }
                self.reconcile_selection();
            }
            super::TreeOperation::PointerSelect(point) => {
                let Some(target) = self.resolve_current_point(point).cloned() else {
                    disposition = TreeConsumed::Unhandled;
                    return self.transition(previous, previous_marks, disposition, external_intent);
                };
                if !self.with_state(|state| Cursored::select_target(state, &flow, &target)) {
                    disposition = TreeConsumed::Unhandled;
                }
                self.reconcile_selection();
            }
            super::TreeOperation::ToggleMark
            | super::TreeOperation::ToggleMarkTarget(_)
            | super::TreeOperation::PointerToggleMark(_) => {
                let target = match operation {
                    super::TreeOperation::ToggleMark => self.selected.clone(),
                    super::TreeOperation::ToggleMarkTarget(target) => Some(target),
                    super::TreeOperation::PointerToggleMark(point) => {
                        self.resolve_current_point(point).cloned()
                    }
                    _ => None,
                };
                if let Some(target) = target {
                    if self.toggle_mark_target(&target) {
                        self.selected = Some(target);
                        // A target operation may address a filter-hidden row;
                        // repair it before reporting the transition.
                        self.reconcile_selection();
                    } else {
                        disposition = TreeConsumed::Unhandled;
                    }
                } else {
                    disposition = TreeConsumed::Unhandled;
                }
            }
            super::TreeOperation::Activate => {
                if let Some(target) = self.selected.clone() {
                    external_intent = Some(TreeExternalIntent::Activate(target));
                } else {
                    disposition = TreeConsumed::Unhandled;
                }
            }
            super::TreeOperation::ActivateTarget(target) => {
                if self.with_state(|state| Cursored::select_target(state, &flow, &target)) {
                    external_intent = Some(TreeExternalIntent::Activate(target));
                    self.reconcile_selection();
                } else {
                    disposition = TreeConsumed::Unhandled;
                }
            }
            super::TreeOperation::Context => {
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
            super::TreeOperation::ContextTarget(target) => {
                if self.target_to_node.contains_key(&target) {
                    external_intent = Some(TreeExternalIntent::Context(target));
                } else {
                    disposition = TreeConsumed::Unhandled;
                }
            }
            super::TreeOperation::EditFilter(query) => self.filter_edit(query),
            super::TreeOperation::ClearFilter => self.clear_filter(),
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
                    self.with_state(|state| MarkSelection::remove_mark(state, target));
                } else {
                    self.with_state(|state| MarkSelection::add_mark(state, target.clone()));
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
                        self.with_state(|state| MarkSelection::remove_mark(state, &child));
                    } else {
                        self.with_state(|state| MarkSelection::add_mark(state, child));
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

    pub(super) fn action_targets(&self) -> Vec<Target> {
        self.visible_node_ids()
            .into_iter()
            .filter_map(|id| self.arena.get(&id))
            .filter(|entry| self.marks.contains(&entry.node.target))
            .map(|entry| entry.node.target.clone())
            .collect()
    }

    pub(super) fn visible_rows(&self) -> Vec<TreePaintRow> {
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
                    self.aggregate_mark_state_for(target),
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

    pub(super) fn retained_rows(&self, area: Rect) -> Vec<(Rect, Target)> {
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
