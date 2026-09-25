//! Shared viewport arithmetic over a cursored ordered [`RowFlow`](super::RowFlow).

use super::{Cursored, Row, RowFlow};

/// The deliberate paging policies supported by the current list shapes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PagingPolicy {
    /// Move by one visible viewport, used by nested/tree-shaped lists.
    VisibleViewport,
}

impl PagingPolicy {
    /// The tree/list policy whose page is the visible viewport.
    pub const fn visible_viewport() -> Self {
        Self::VisibleViewport
    }
}

/// Primitive viewport state and shared visibility arithmetic.
///
/// A shape supplies only offset access and its [`Cursored`] target hooks. The
/// offset is in complete flow-row space, so structural rows remain part of
/// clamping and visibility calculations.
pub trait Viewported<Target: Eq>: Cursored<Target> {
    /// Current flow-row offset.
    fn viewport_offset(&self) -> usize;

    /// Store a flow-row offset. Shared methods pass a clamped value.
    fn set_viewport_offset(&mut self, offset: usize);

    /// Maximum legal offset for a flow and viewport geometry.
    fn max_viewport_offset(flow: &RowFlow<Target>, viewport_len: usize) -> usize {
        flow.len().saturating_sub(viewport_len.max(1))
    }

    /// Clamp the existing offset in place, preserving it whenever geometry
    /// still permits it.
    fn clamp_viewport(&mut self, flow: &RowFlow<Target>, viewport_len: usize) -> usize {
        let offset = self
            .viewport_offset()
            .min(Self::max_viewport_offset(flow, viewport_len));
        self.set_viewport_offset(offset);
        offset
    }

    /// Whether scrolling a selection into view from above also raises over
    /// the contiguous structural rows directly above it, keeping a grouped
    /// list's heading visible with its first selected item (#731). The
    /// default is the minimum-scroll behavior; a shape with leading structural
    /// rows opts in.
    fn raise_over_leading_structural_rows(&self) -> bool {
        false
    }

    /// The offset that keeps `selected_position` visible without mutating
    /// this owner. Existing offset is preserved when it already satisfies
    /// visibility; otherwise only the minimum scroll is applied.
    fn resolved_viewport_offset(
        &self,
        flow: &RowFlow<Target>,
        viewport_len: usize,
        selected_position: Option<usize>,
    ) -> usize {
        let height = viewport_len.max(1);
        let max_offset = Self::max_viewport_offset(flow, height);
        let mut offset = self.viewport_offset().min(max_offset);
        if let Some(position) = selected_position {
            if position < offset {
                offset = position;
                if self.raise_over_leading_structural_rows() {
                    while offset > 0 && flow.row_at(offset - 1).is_some_and(Row::is_structural) {
                        offset -= 1;
                    }
                }
            } else if position >= offset.saturating_add(height) {
                offset = position + 1 - height;
            }
            offset = offset.min(max_offset);
        }
        offset
    }

    /// Keep a selected flow position visible with the minimum scroll needed.
    /// Existing offset is preserved when it already satisfies visibility.
    fn keep_selection_visible(
        &mut self,
        flow: &RowFlow<Target>,
        viewport_len: usize,
        selected_position: Option<usize>,
    ) -> usize {
        let offset = self.resolved_viewport_offset(flow, viewport_len, selected_position);
        self.set_viewport_offset(offset);
        offset
    }

    /// Keep this cursor visible using its current target, if it is present in
    /// the flow.
    fn keep_cursor_visible(&mut self, flow: &RowFlow<Target>, viewport_len: usize) -> usize {
        let selected_position = self.index(flow);
        self.keep_selection_visible(flow, viewport_len, selected_position)
    }

    /// Page selection under an explicit shape policy, then keep it visible.
    fn page(
        &mut self,
        flow: &RowFlow<Target>,
        viewport_len: usize,
        direction: isize,
        policy: PagingPolicy,
    ) -> Option<usize> {
        let selected = match policy {
            PagingPolicy::VisibleViewport => {
                self.page_by_visible_rows(flow, viewport_len, direction)
            }
        };
        self.keep_selection_visible(flow, viewport_len, selected);
        selected
    }

    /// Page by flow rows while using [`Cursored::move_by`] for selection and
    /// its single stale-selection recovery rule.
    fn page_by_visible_rows(
        &mut self,
        flow: &RowFlow<Target>,
        viewport_len: usize,
        direction: isize,
    ) -> Option<usize> {
        let Some(current) = self.index(flow) else {
            let recovery_delta = if direction.is_negative() { -1 } else { 1 };
            return self.move_by(flow, recovery_delta);
        };
        let distance = viewport_len.max(1);
        let desired = if direction.is_negative() {
            current.saturating_sub(distance)
        } else {
            current
                .saturating_add(distance)
                .min(flow.len().saturating_sub(1))
        };
        let position = if direction.is_negative() {
            (0..=desired)
                .rev()
                .find(|&position| flow.target_at(position).is_some())
                .or_else(|| {
                    (desired + 1..flow.len()).find(|&position| flow.target_at(position).is_some())
                })
        } else {
            (desired..flow.len())
                .find(|&position| flow.target_at(position).is_some())
                .or_else(|| {
                    (0..desired)
                        .rev()
                        .find(|&position| flow.target_at(position).is_some())
                })
        };
        let Some(position) = position else {
            return self.move_by(flow, if direction.is_negative() { -1 } else { 1 });
        };
        let (Some(current_ordinal), Some(target_ordinal)) = (
            flow.selectable_ordinal_at(current),
            flow.selectable_ordinal_at(position),
        ) else {
            return self.move_by(flow, if direction.is_negative() { -1 } else { 1 });
        };
        let delta = if target_ordinal >= current_ordinal {
            isize::try_from(target_ordinal - current_ordinal).unwrap_or(isize::MAX)
        } else {
            -isize::try_from(current_ordinal - target_ordinal).unwrap_or(isize::MAX)
        };
        self.move_by(flow, delta)
    }

    /// Clamp geometry and then keep the current cursor visible. This is the
    /// operation used after a resize or content replacement.
    fn reconcile_viewport(&mut self, flow: &RowFlow<Target>, viewport_len: usize) -> usize {
        self.clamp_viewport(flow, viewport_len);
        self.keep_cursor_visible(flow, viewport_len)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{PagingPolicy, Viewported};
    use crate::app::components::list::{Cursored, Row, RowFlow, TestListState};

    /// A shape that opts into the leading-structural-raise policy (#731).
    #[derive(Default)]
    struct RaisingListState {
        selected: Option<u8>,
        offset: usize,
    }

    impl Cursored<u8> for RaisingListState {
        fn selected_target(&self) -> Option<&u8> {
            self.selected.as_ref()
        }

        fn set_selected_target(&mut self, target: Option<&u8>) {
            self.selected = target.copied();
        }
    }

    impl Viewported<u8> for RaisingListState {
        fn viewport_offset(&self) -> usize {
            self.offset
        }

        fn set_viewport_offset(&mut self, offset: usize) {
            self.offset = offset;
        }

        fn raise_over_leading_structural_rows(&self) -> bool {
            true
        }
    }

    fn flow() -> RowFlow<u8> {
        RowFlow::new(vec![
            Row::selectable(1),
            Row::structural(),
            Row::selectable(2),
            Row::selectable(3),
            Row::structural(),
            Row::selectable(4),
            Row::selectable(5),
            Row::selectable(6),
        ])
    }

    #[test]
    fn raising_over_structural_rows_keeps_the_group_heading_visible() {
        let rows = RowFlow::new(vec![
            Row::selectable(1),
            Row::structural(),
            Row::selectable(2),
            Row::selectable(3),
            Row::selectable(4),
            Row::selectable(5),
            Row::selectable(6),
        ]);
        let mut list = RaisingListState {
            selected: Some(2),
            offset: 4,
        };

        assert_eq!(list.keep_cursor_visible(&rows, 3), 1);
        assert_eq!(list.offset, 1);
    }

    #[test]
    fn preserves_offset_when_selection_is_visible() {
        let rows = flow();
        let mut list = TestListState {
            selected: Some(3),
            offset: 2,
        };

        assert_eq!(list.keep_cursor_visible(&rows, 4), 2);
        assert_eq!(list.offset, 2);
    }

    #[rstest]
    #[case(Some(1), 4, 0)]
    #[case(Some(7), 3, 5)]
    #[case(Some(2), 2, 2)]
    fn scrolls_minimally_and_clamps_geometry(
        #[case] selected: Option<u8>,
        #[case] viewport_len: usize,
        #[case] expected_offset: usize,
    ) {
        let rows = flow();
        let mut list = TestListState {
            selected,
            offset: usize::MAX,
        };

        assert_eq!(
            list.reconcile_viewport(&rows, viewport_len),
            expected_offset
        );
        assert_eq!(list.offset, expected_offset);
    }

    #[test]
    fn geometry_clamping_preserves_offset_when_bounds_allow() {
        let rows = flow();
        let mut list = TestListState {
            selected: Some(2),
            offset: 3,
        };

        assert_eq!(list.clamp_viewport(&rows, 3), 3);
        assert_eq!(list.offset, 3);
        assert_eq!(list.clamp_viewport(&rows, 99), 0);
        assert_eq!(list.offset, 0);
    }

    #[rstest]
    #[case(-1, Some(6), 5)]
    #[case(1, Some(1), 0)]
    fn visible_viewport_paging_recovers_missing_selection_and_keeps_it_visible(
        #[case] direction: isize,
        #[case] expected_selected: Option<u8>,
        #[case] expected_offset: usize,
    ) {
        let rows = flow();
        let mut list = TestListState {
            selected: None,
            offset: 4,
        };

        let selected = list.page(&rows, 3, direction, PagingPolicy::VisibleViewport);

        assert_eq!(selected, Some(if direction < 0 { 7 } else { 0 }));
        assert_eq!(list.selected, expected_selected);
        assert_eq!(list.offset, expected_offset);
    }
}
