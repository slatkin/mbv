//! Ordered stable-target membership shared by flat and tree list shapes.

use super::RowFlow;

/// State carrier for multi-selection membership.
///
/// The vector is intentionally ordered by addition.  Actions should use
/// [`MarkSelection::marked_targets_in_flow_order`] when they need display order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkSelectionState<Target> {
    marked: Vec<Target>,
}

impl<Target> Default for MarkSelectionState<Target> {
    fn default() -> Self {
        Self { marked: Vec::new() }
    }
}

impl<Target> MarkSelectionState<Target> {
    /// Create an empty ordered-mark carrier.
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks in their stored addition order.
    pub fn targets(&self) -> &[Target] {
        &self.marked
    }

    pub fn is_empty(&self) -> bool {
        self.marked.is_empty()
    }

    pub fn len(&self) -> usize {
        self.marked.len()
    }
}

impl<Target: PartialEq> MarkSelectionState<Target> {
    /// Add `target` at the end of the addition order.  Returns `false` when it
    /// was already marked.
    pub fn add(&mut self, target: Target) -> bool {
        if self.marked.iter().any(|marked| marked == &target) {
            return false;
        }
        self.marked.push(target);
        true
    }

    /// Remove `target`, preserving the relative order of the remaining marks.
    pub fn remove(&mut self, target: &Target) -> bool {
        let Some(index) = self.marked.iter().position(|marked| marked == target) else {
            return false;
        };
        self.marked.remove(index);
        true
    }

    /// Toggle membership while retaining addition order for newly added marks.
    #[allow(dead_code)]
    pub fn toggle(&mut self, target: Target) -> bool {
        if self.remove(&target) {
            false
        } else {
            self.add(target);
            true
        }
    }

    /// Whether `target` is currently marked.
    pub fn contains(&self, target: &Target) -> bool {
        self.marked.iter().any(|marked| marked == target)
    }

    /// Remove all marks.
    pub fn clear(&mut self) {
        self.marked.clear();
    }

    /// Replace all marks in the caller-provided order.
    pub fn set_targets<I>(&mut self, targets: I)
    where
        I: IntoIterator<Item = Target>,
    {
        self.marked.clear();
        self.marked.extend(targets);
    }

    /// Retain marks matching the supplied predicate without changing the
    /// relative addition order of survivors.
    pub fn retain<F>(&mut self, mut keep: F)
    where
        F: FnMut(&Target) -> bool,
    {
        self.marked.retain(|target| keep(target));
    }
}

/// Shared ordered-mark operations supplied by a list shape's state owner.
pub trait MarkSelection<Target: PartialEq> {
    /// Borrow the shape-owned ordered membership carrier.
    fn mark_selection(&self) -> &MarkSelectionState<Target>;

    /// Borrow the shape-owned ordered membership carrier mutably.
    fn mark_selection_mut(&mut self) -> &mut MarkSelectionState<Target>;

    /// Add a stable target, retaining its addition order.
    fn add_mark(&mut self, target: Target) -> bool {
        let added = self.mark_selection_mut().add(target);
        self.after_mark_mutation();
        added
    }

    /// Remove a stable target without reordering the remaining membership.
    fn remove_mark(&mut self, target: &Target) -> bool {
        let removed = self.mark_selection_mut().remove(target);
        self.after_mark_mutation();
        removed
    }

    /// Toggle a stable target's membership.
    #[allow(dead_code)]
    fn toggle_mark(&mut self, target: Target) -> bool {
        let added = self.mark_selection_mut().toggle(target);
        self.after_mark_mutation();
        added
    }

    /// Clear the selection.
    fn clear_marks(&mut self) {
        self.mark_selection_mut().clear();
        self.after_mark_mutation();
    }

    /// Let an owner keep any coupled selection state in sync with a mutation.
    fn after_mark_mutation(&mut self) {}

    /// Whether a stable target is marked.
    fn is_marked(&self, target: &Target) -> bool {
        self.mark_selection().contains(target)
    }

    /// Stored membership in addition order.
    #[cfg_attr(not(test), allow(dead_code))]
    fn marked_targets(&self) -> &[Target] {
        self.mark_selection().targets()
    }

    /// Marked targets in the current row-flow (display) order.
    ///
    /// The returned references point at the flow's stable targets.  This keeps
    /// the operation read-only and makes it impossible for action ordering to
    /// rewrite the carrier's addition order.
    fn marked_targets_in_flow_order<'a>(&'a self, flow: &'a RowFlow<Target>) -> Vec<&'a Target> {
        (0..flow.len())
            .filter_map(|position| flow.row_at(position).and_then(|row| row.target()))
            .filter(|target| self.is_marked(target))
            .collect()
    }

    /// Owned action targets in current display order for a destination that
    /// needs to cross the component boundary.
    fn action_targets(&self, flow: &RowFlow<Target>) -> Vec<Target>
    where
        Target: Clone,
    {
        self.marked_targets_in_flow_order(flow)
            .into_iter()
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{MarkSelection, MarkSelectionState};
    use crate::app::components::list::{Row, RowFlow};

    #[derive(Default)]
    struct TestMarks {
        marks: MarkSelectionState<u8>,
    }

    impl MarkSelection<u8> for TestMarks {
        fn mark_selection(&self) -> &MarkSelectionState<u8> {
            &self.marks
        }

        fn mark_selection_mut(&mut self) -> &mut MarkSelectionState<u8> {
            &mut self.marks
        }
    }

    #[test]
    fn interleaved_addition_and_removal_preserves_membership_order() {
        let mut marks = TestMarks::default();
        assert!(marks.add_mark(4));
        assert!(marks.add_mark(1));
        assert!(marks.add_mark(3));
        assert!(marks.remove_mark(&1));
        assert!(marks.add_mark(2));
        assert_eq!(marks.marked_targets(), &[4, 3, 2]);
    }

    #[test]
    fn action_targets_follow_display_order_not_click_order() {
        let mut marks = TestMarks::default();
        marks.add_mark(3);
        marks.add_mark(1);
        marks.add_mark(4);
        let flow = RowFlow::new(vec![
            Row::selectable(4),
            Row::structural(),
            Row::selectable(1),
            Row::selectable(3),
        ]);

        assert_eq!(marks.marked_targets(), &[3, 1, 4]);
        assert_eq!(marks.action_targets(&flow), vec![4, 1, 3]);
        assert_eq!(marks.marked_targets(), &[3, 1, 4]);
    }
}
