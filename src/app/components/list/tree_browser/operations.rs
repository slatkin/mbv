//! Semantic tree operations and the private adapter for shared list arithmetic.

use std::hash::Hash;

use fuzzy_matcher::skim::SkimMatcherV2;
use ratatui::layout::Rect;

use crate::app::components::list::{
    AggregateMarkState, Cursored, Expandable, MarkSelection, MarkSelectionState, PagingPolicy, Row,
    RowFlow, Viewported,
};
// The full-width bar is paint policy, so its predicate lives with the shared
// tree painter and is imported through the app-level render seam.
use crate::app::render::tree_row_is_full_width;

use super::{
    StructuralRow, TreeAggregateMark, TreeBrowser, TreeConsumed, TreeExternalIntent,
    TreeMarkPolicy, TreeMarkSummary, TreePaintRow, TreeSelectionChange, TreeTransition, VisibleRow,
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
}

impl<Target: Clone + Eq + Hash> TreeBrowser<Target> {
    fn is_expandable(&self, target: &Target) -> bool {
        self.target_to_node
            .get(target)
            .and_then(|id| self.arena.get(id))
            .is_some_and(|entry| entry.node.expandable || !entry.children.is_empty())
    }

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
                crate::app::infra::fuzzy_match::word_match_score(
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

    pub(super) fn visible_flow_rows(&self) -> Vec<VisibleRow> {
        let mut output = Vec::new();
        for &root in &self.roots {
            let mut nodes = Vec::new();
            self.push_visible(root, &mut nodes);
            if nodes.is_empty() {
                continue;
            }
            if let Some(entry) = self.arena.get(&root) {
                if let Some(structures) = self.root_structures.get(&entry.node.target) {
                    output.extend(
                        structures
                            .iter()
                            .enumerate()
                            .map(|(index, _)| VisibleRow::Structural(root, index)),
                    );
                }
            }
            output.extend(nodes.into_iter().map(VisibleRow::Node));
        }
        output
    }

    pub(super) fn visible_len(&self) -> usize {
        self.visible_flow_rows().len()
    }

    pub(super) fn current_flow(&self) -> RowFlow<Target> {
        RowFlow::new(
            self.visible_flow_rows()
                .into_iter()
                .map(|row| match row {
                    VisibleRow::Node(id) => self
                        .arena
                        .get(&id)
                        .map(|entry| Row::selectable(entry.node.target.clone()))
                        .unwrap_or_else(Row::structural),
                    VisibleRow::Structural(_, _) => Row::structural(),
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
        // Keep the selection visible against the established viewport: the
        // panel-declared content height, or the latest completed frame's.
        // Before either exists there is no viewport to reconcile, so the
        // offset stays untouched and the next view applies the real rule.
        let height = self
            .configured_geometry
            .map(|(_, content)| usize::from(content.height))
            .or_else(|| self.last_painted.map(|area| usize::from(area.height)));
        if let Some(height) = height {
            self.with_state(|state| Viewported::reconcile_viewport(state, &flow, height));
        }
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
        let flow = self.current_flow();
        let Some((disposition, external_intent)) = self.apply_operation(operation, &flow) else {
            return self.transition(previous, previous_marks, TreeConsumed::Unhandled, None);
        };
        self.invalidate_paint();
        self.transition(previous, previous_marks, disposition, external_intent)
    }

    fn apply_operation(
        &mut self,
        operation: super::TreeOperation<Target>,
        flow: &RowFlow<Target>,
    ) -> Option<(TreeConsumed, Option<TreeExternalIntent<Target>>)> {
        let mut disposition = TreeConsumed::Consumed;
        let mut external_intent = None;
        match operation {
            super::TreeOperation::Move(delta) => {
                self.with_state(|state| {
                    Cursored::move_by(
                        state,
                        flow,
                        delta.clamp(isize::MIN as i64, isize::MAX as i64) as isize,
                    )
                });
                self.reconcile_selection();
            }
            super::TreeOperation::Page(direction) => {
                if !self.apply_page(direction, flow) {
                    return None;
                }
            }
            operation @ (super::TreeOperation::First
            | super::TreeOperation::Last
            | super::TreeOperation::Parent) => self.apply_cursor_operation(operation, flow),
            super::TreeOperation::Right => {
                (disposition, external_intent) = self.apply_right();
            }
            super::TreeOperation::ToggleExpansionTarget(target) => {
                disposition = self.apply_toggle_expansion(target);
            }
            super::TreeOperation::AnchorSelection {
                target,
                flow_offset,
            } => {
                if !self.anchor_selection_to(&target, flow_offset) {
                    disposition = TreeConsumed::Unhandled;
                }
            }
            super::TreeOperation::Select(target) => {
                if !self.with_state(|state| Cursored::select_target(state, flow, &target)) {
                    disposition = TreeConsumed::Unhandled;
                }
                self.reconcile_selection();
            }
            super::TreeOperation::PointerToggleMark(point) => {
                if !self.apply_pointer_toggle_mark(point) {
                    return None;
                }
            }
            super::TreeOperation::Activate => {
                (disposition, external_intent) = self.apply_activate();
            }
            super::TreeOperation::Context => {
                (disposition, external_intent) = self.apply_context();
            }
            operation @ (super::TreeOperation::EditFilter(_)
            | super::TreeOperation::ClearFilter
            | super::TreeOperation::ClearMarks) => self.apply_filter_operation(operation),
        }
        Some((disposition, external_intent))
    }

    fn apply_cursor_operation(
        &mut self,
        operation: super::TreeOperation<Target>,
        flow: &RowFlow<Target>,
    ) {
        match operation {
            super::TreeOperation::First => {
                self.with_state(|state| Cursored::first(state, flow));
            }
            super::TreeOperation::Last => {
                self.with_state(|state| Cursored::last(state, flow));
            }
            super::TreeOperation::Parent => {
                self.with_state(|state| Expandable::select_parent(state, flow));
            }
            _ => return,
        }
        self.reconcile_selection();
    }

    fn apply_filter_operation(&mut self, operation: super::TreeOperation<Target>) {
        match operation {
            super::TreeOperation::EditFilter(query) => self.filter_edit(query),
            super::TreeOperation::ClearFilter => self.clear_filter(),
            super::TreeOperation::ClearMarks => self.marks.clear(),
            _ => return,
        }
    }

    fn apply_page(&mut self, direction: i64, flow: &RowFlow<Target>) -> bool {
        // The page distance is the established visible viewport: the
        // panel-declared content height, or the latest completed frame's
        // height. No viewport means no page.
        let height = self
            .configured_geometry
            .map(|(_, content)| usize::from(content.height))
            .or_else(|| self.last_painted.map(|area| usize::from(area.height)));
        let Some(height) = height else {
            return false;
        };
        self.with_state(|state| {
            Viewported::page(
                state,
                flow,
                height,
                direction.clamp(isize::MIN as i64, isize::MAX as i64) as isize,
                PagingPolicy::visible_viewport(),
            )
        });
        true
    }

    fn apply_right(&mut self) -> (TreeConsumed, Option<TreeExternalIntent<Target>>) {
        let Some(target) = self.selected.clone() else {
            return (TreeConsumed::Unhandled, None);
        };
        if !self.is_expandable(&target) {
            return (TreeConsumed::Unhandled, None);
        }
        if self.expanded.contains(&target) {
            let is_root = self
                .target_to_node
                .get(&target)
                .and_then(|id| self.arena.get(id))
                .is_some_and(|entry| entry.node.parent.is_none());
            return if is_root {
                (
                    TreeConsumed::Consumed,
                    Some(TreeExternalIntent::Activate(target)),
                )
            } else {
                (TreeConsumed::Consumed, None)
            };
        }
        self.with_state(|state| Expandable::toggle_expanded(state, &target));
        self.reconcile_selection();
        (TreeConsumed::Consumed, None)
    }

    fn apply_toggle_expansion(&mut self, target: Target) -> TreeConsumed {
        if !self.is_expandable(&target) {
            return TreeConsumed::Unhandled;
        }
        self.with_state(|state| Expandable::toggle_expanded(state, &target));
        self.reconcile_selection();
        TreeConsumed::Consumed
    }

    fn apply_pointer_toggle_mark(&mut self, point: ratatui::layout::Position) -> bool {
        // Legacy parity (the retired `toggle_mark_at`): a modified click
        // resolves the painted row and moves the cursor to it before toggling.
        // An `Excluded` row (a cached track) is not markable, so it still takes
        // the cursor and reports no mark change rather than leaving selection behind.
        let Some(target) = self.resolve_current_point(point).cloned() else {
            return false;
        };
        self.selected = Some(target.clone());
        let _ = self.toggle_mark_target(&target);
        self.reconcile_selection();
        true
    }

    fn apply_activate(&self) -> (TreeConsumed, Option<TreeExternalIntent<Target>>) {
        self.selected
            .clone()
            .map_or((TreeConsumed::Unhandled, None), |target| {
                (
                    TreeConsumed::Consumed,
                    Some(TreeExternalIntent::Activate(target)),
                )
            })
    }

    fn apply_context(&self) -> (TreeConsumed, Option<TreeExternalIntent<Target>>) {
        let Some(target) = self.selected.clone() else {
            return (TreeConsumed::Unhandled, None);
        };
        let targets = self.action_targets();
        let intent = if targets.is_empty() {
            TreeExternalIntent::Context(target)
        } else {
            TreeExternalIntent::ContextSelection(targets)
        };
        (TreeConsumed::Consumed, Some(intent))
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

    pub(super) fn visible_rows(&self, visible_rows: &[VisibleRow]) -> Vec<TreePaintRow> {
        let mut rows = Vec::new();
        let mut group_root_index = 0;
        let grouped = self.root_structures.values().any(|structures| {
            structures
                .iter()
                .any(|structure| matches!(structure, StructuralRow::Heading(_)))
        });
        let mut group_item_index = 0;
        for (flow_index, &visible) in visible_rows.iter().enumerate() {
            if let VisibleRow::Structural(root, index) = visible {
                let Some(entry) = self.arena.get(&root) else {
                    continue;
                };
                let Some(structure) = self
                    .root_structures
                    .get(&entry.node.target)
                    .and_then(|structures| structures.get(index))
                else {
                    continue;
                };
                let (kind, title) = match structure {
                    StructuralRow::Heading(title) => {
                        group_root_index = entry.root_index;
                        group_item_index = 0;
                        (super::TreePaintRowKind::Heading, title.clone())
                    }
                    StructuralRow::Spacer => (super::TreePaintRowKind::Spacer, String::new()),
                };
                if flow_index >= self.viewport_offset {
                    rows.push(TreePaintRow {
                        kind,
                        title,
                        title_role: super::TreeTitleRole::Standard,
                        trailing: None,
                        depth: 0,
                        root_index: entry.root_index,
                        group_root_index,
                        zebra_striped: false,
                        selected: false,
                        marked: false,
                        aggregate_mark: TreeAggregateMark::None,
                        semantic_state:
                            crate::app::components::media_list::MediaSemanticState::Ordinary,
                    });
                }
                continue;
            }
            let VisibleRow::Node(id) = visible else {
                continue;
            };
            let Some(entry) = self.arena.get(&id) else {
                continue;
            };
            let target = &entry.node.target;
            let zebra_striped = if grouped {
                let striped = group_item_index % 2 == 0;
                group_item_index += 1;
                striped
            } else {
                entry.root_index.saturating_sub(group_root_index) % 2 == 0
            };
            if flow_index < self.viewport_offset {
                continue;
            }
            let marked = self.marks.contains(target);
            let aggregate_mark = if entry.node.mark_policy == TreeMarkPolicy::Aggregate {
                match self.aggregate_mark_state_for(target) {
                    AggregateMarkState::Marked => TreeAggregateMark::Full,
                    AggregateMarkState::Partial => TreeAggregateMark::Partial,
                    AggregateMarkState::Unmarked => TreeAggregateMark::None,
                }
            } else {
                TreeAggregateMark::None
            };
            rows.push(TreePaintRow {
                kind: super::TreePaintRowKind::Node,
                title: entry.node.title.clone(),
                title_role: entry.node.title_role,
                trailing: entry
                    .node
                    .trailing
                    .as_ref()
                    .map(|trailing| trailing.text.clone()),
                depth: entry.depth,
                root_index: entry.root_index,
                group_root_index,
                zebra_striped,
                selected: self.selected.as_ref() == Some(target),
                marked,
                aggregate_mark,
                semantic_state: entry.node.semantic_state.clone(),
            });
        }
        rows
    }

    pub(super) fn retained_rows(
        &self,
        visible_rows: &[VisibleRow],
        claim_rect: Rect,
        content_rect: Rect,
    ) -> Vec<(Rect, Target)> {
        visible_rows
            .iter()
            .copied()
            .skip(self.viewport_offset)
            .enumerate()
            .filter_map(|(index, row)| {
                let VisibleRow::Node(id) = row else {
                    return None;
                };
                let y = content_rect.y.checked_add(index as u16)?;
                (y < content_rect.bottom()).then(|| {
                    self.arena.get(&id).map(|entry| {
                        let target = entry.node.target.clone();
                        let marked = self.marks.contains(&target);
                        let selected = self.focused && self.selected.as_ref() == Some(&target);
                        let full_width = tree_row_is_full_width(selected, marked);
                        let rect = if full_width {
                            Rect::new(claim_rect.x, y, claim_rect.width, 1)
                        } else {
                            Rect::new(content_rect.x, y, content_rect.width, 1)
                        };
                        (rect, target)
                    })
                })?
            })
            .collect()
    }
}
