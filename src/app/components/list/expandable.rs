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
}
