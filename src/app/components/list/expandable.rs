//! The tree-only expansion seam over stable targets.
//!
//! Expansion storage remains owned by the tree adapter (and, in the shipped
//! tree, by its `TreeListViewState` owner).  This trait supplies only the
//! stable-target contract and the shared post-projection validation; it does
//! not introduce structural nodes or any tree crate type.

use super::{Cursored, RowFlow, Viewported};

/// Aggregate mark state for a parent with visible children.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AggregateMarkState {
    /// No visible child is marked.
    Unmarked,
    /// Some, but not all, visible children are marked.
    Partial,
    /// Every visible child is marked.
    Marked,
}

fn set_selected_position<Target: Eq, State: Cursored<Target> + ?Sized>(
    state: &mut State,
    flow: &RowFlow<Target>,
    position: Option<usize>,
) -> Option<usize> {
    if let Some(position) = position {
        state.set_selected_target(flow.target_at(position));
    }
    position
}

/// Tree-only adapter contract for expansion, hierarchy movement, and parent
/// mark aggregation.
pub trait Expandable<Target: Eq>: Cursored<Target> + Viewported<Target> {
    /// Whether the stable target is currently expanded.
    fn is_expanded(&self, target: &Target) -> bool;

    /// Store expansion for a stable target in the shape's existing state owner.
    fn set_expanded(&mut self, target: &Target, expanded: bool);

    /// The stable parent target, if this target has one.
    fn parent_target(&self, target: &Target) -> Option<&Target>;

    /// Stable child targets in the shape's established child order.
    fn child_targets(&self, target: &Target) -> Vec<&Target>;

    /// Shape-owned aggregate state for a parent.  This is deliberately not
    /// derived from mark membership: tree shapes may have hidden children and
    /// therefore different aggregation policy.
    fn aggregate_mark_state(&self, target: &Target) -> AggregateMarkState;

    /// Toggle expansion without taking ownership of a shape-specific handle.
    fn toggle_expanded(&mut self, target: &Target) {
        let expanded = !self.is_expanded(target);
        self.set_expanded(target, expanded);
    }

    /// Move the cursor to the selected target's visible parent.
    fn select_parent(&mut self, flow: &RowFlow<Target>) -> Option<usize> {
        let position = self
            .selected_target()
            .and_then(|target| self.parent_target(target))
            .and_then(|parent| flow.position_of(parent));
        set_selected_position(self, flow, position)
    }

    /// Move the cursor to the first visible child of the selected target.
    fn select_first_child(&mut self, flow: &RowFlow<Target>) -> Option<usize> {
        let position = self
            .selected_target()
            .map(|target| self.child_targets(target))
            .into_iter()
            .flatten()
            .find_map(|child| flow.position_of(child));
        set_selected_position(self, flow, position)
    }

    /// Move the cursor to the last visible child of the selected target.
    fn select_last_child(&mut self, flow: &RowFlow<Target>) -> Option<usize> {
        let position = self
            .selected_target()
            .map(|target| self.child_targets(target))
            .into_iter()
            .flatten()
            .rev()
            .find_map(|child| flow.position_of(child));
        set_selected_position(self, flow, position)
    }

    /// Validate selection after a projection change, then reuse the shared
    /// viewport clamp/keep-visible arithmetic.  A collapsed descendant is
    /// therefore repaired to the seam's deterministic first-row fallback.
    fn reconcile_after_change(&mut self, flow: &RowFlow<Target>, viewport_len: usize) -> usize {
        if self.index(flow).is_none() {
            self.first(flow);
        }
        self.reconcile_viewport(flow, viewport_len)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{AggregateMarkState, Expandable};
    use crate::app::components::list::{Cursored, Row, RowFlow, TestListState, Viewported};

    struct TestTree {
        state: TestListState,
        expanded: HashSet<u8>,
    }

    impl Default for TestTree {
        fn default() -> Self {
            Self {
                state: TestListState {
                    selected: Some(3),
                    offset: 2,
                },
                expanded: HashSet::from([1]),
            }
        }
    }

    impl Cursored<u8> for TestTree {
        fn selected_target(&self) -> Option<&u8> {
            self.state.selected_target()
        }

        fn set_selected_target(&mut self, target: Option<&u8>) {
            self.state.set_selected_target(target);
        }
    }

    impl Viewported<u8> for TestTree {
        fn viewport_offset(&self) -> usize {
            self.state.viewport_offset()
        }

        fn set_viewport_offset(&mut self, offset: usize) {
            self.state.set_viewport_offset(offset);
        }
    }
    impl Expandable<u8> for TestTree {
        fn is_expanded(&self, target: &u8) -> bool {
            self.expanded.contains(target)
        }

        fn set_expanded(&mut self, target: &u8, expanded: bool) {
            if expanded {
                self.expanded.insert(*target);
            } else {
                self.expanded.remove(target);
            }
        }

        fn parent_target(&self, target: &u8) -> Option<&u8> {
            match target {
                2 | 3 => Some(&1),
                _ => None,
            }
        }

        fn child_targets(&self, target: &u8) -> Vec<&u8> {
            match target {
                1 => vec![&2, &3],
                _ => Vec::new(),
            }
        }

        fn aggregate_mark_state(&self, _target: &u8) -> AggregateMarkState {
            AggregateMarkState::Unmarked
        }
    }

    fn expanded_flow() -> RowFlow<u8> {
        RowFlow::new(vec![
            Row::selectable(1),
            Row::selectable(2),
            Row::selectable(3),
            Row::selectable(4),
        ])
    }

    #[test]
    fn parent_and_child_movement_uses_stable_targets() {
        let mut tree = TestTree::default();
        let flow = expanded_flow();

        assert_eq!(tree.select_parent(&flow), Some(0));
        assert_eq!(tree.selected_target(), Some(&1));
        assert_eq!(tree.select_last_child(&flow), Some(2));
        assert_eq!(tree.selected_target(), Some(&3));
    }

    #[test]
    fn collapse_removes_descendants_and_reconciles_selection_and_viewport() {
        let mut tree = TestTree::default();
        let expanded = expanded_flow();
        tree.select_target(&expanded, &3);
        tree.set_viewport_offset(2);
        tree.set_expanded(&1, false);

        // The collapsed projection contains no structural/tree-node handles;
        // it is simply the current stable-target row flow.
        let collapsed = RowFlow::new(vec![Row::selectable(1), Row::selectable(4)]);
        tree.reconcile_after_change(&collapsed, 1);

        assert_eq!(tree.selected_target(), Some(&1));
        assert_eq!(tree.index(&collapsed), Some(0));
        assert_eq!(tree.viewport_offset(), 0);
        assert!(tree.index(&collapsed).is_some());
    }
}
