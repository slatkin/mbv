//! Shared viewport arithmetic over a cursored ordered [`RowFlow`](super::RowFlow).

use std::convert::TryFrom;

use super::{Cursored, RowFlow};

/// The deliberate paging policies supported by the current list shapes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PagingPolicy {
    /// Move a fixed number of selectable rows, used by flat lists.
    FixedSelectableDistance(usize),
    /// Move by one visible viewport, used by nested/tree-shaped lists.
    VisibleViewport,
}

impl PagingPolicy {
    /// The flat-list policy with a fixed selectable-row distance.
    pub const fn fixed_selectable_distance(distance: usize) -> Self {
        Self::FixedSelectableDistance(distance)
    }

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

    /// Keep a selected flow position visible with the minimum scroll needed.
    /// Existing offset is preserved when it already satisfies visibility.
    fn keep_selection_visible(
        &mut self,
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
            } else if position >= offset.saturating_add(height) {
                offset = position + 1 - height;
            }
            offset = offset.min(max_offset);
        }
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
            PagingPolicy::FixedSelectableDistance(distance) => {
                let distance = isize::try_from(distance).unwrap_or(isize::MAX);
                let delta = if direction.is_negative() {
                    -distance
                } else {
                    distance
                };
                self.move_by(flow, delta)
            }
            PagingPolicy::VisibleViewport => {
                self.page_by_visible_rows(flow, viewport_len, direction)
            }
        };
        self.keep_selection_visible(flow, viewport_len, selected);
        selected
    }

    /// Page by flow rows rather than selectable ordinals. This keeps the
    /// visible-viewport policy honest if a future nested shape includes
    /// structural rows in its flow.
    fn page_by_visible_rows(
        &mut self,
        flow: &RowFlow<Target>,
        viewport_len: usize,
        direction: isize,
    ) -> Option<usize> {
        if flow.is_empty() {
            self.set_selected_target(None);
            return None;
        }
        let current = self.index(flow).unwrap_or_else(|| {
            if direction.is_negative() {
                flow.len()
            } else {
                0
            }
        });
        let distance = viewport_len.max(1);
        let desired = if direction.is_negative() {
            current.saturating_sub(distance)
        } else {
            current.saturating_add(distance).min(flow.len() - 1)
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
        self.set_selected_target(position.and_then(|position| flow.target_at(position)));
        position
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
    use crate::app::components::list::{Cursored, Row, RowFlow};

    #[derive(Default)]
    struct List {
        selected: Option<u8>,
        offset: usize,
    }

    impl Cursored<u8> for List {
        fn selected_target(&self) -> Option<&u8> {
            self.selected.as_ref()
        }

        fn set_selected_target(&mut self, target: Option<&u8>) {
            self.selected = target.copied();
        }
    }

    impl Viewported<u8> for List {
        fn viewport_offset(&self) -> usize {
            self.offset
        }

        fn set_viewport_offset(&mut self, offset: usize) {
            self.offset = offset;
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
    fn preserves_offset_when_selection_is_visible() {
        let rows = flow();
        let mut list = List {
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
        let mut list = List {
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
        let mut list = List {
            selected: Some(2),
            offset: 3,
        };

        assert_eq!(list.clamp_viewport(&rows, 3), 3);
        assert_eq!(list.offset, 3);
        assert_eq!(list.clamp_viewport(&rows, 99), 0);
        assert_eq!(list.offset, 0);
    }

    #[test]
    fn fixed_selectable_distance_pages_by_named_policy() {
        let rows = flow();
        let mut list = List::default();
        list.first(&rows);

        assert_eq!(
            list.page(&rows, 3, 1, PagingPolicy::fixed_selectable_distance(2)),
            Some(3)
        );
        assert_eq!(list.selected, Some(3));
    }

    #[test]
    fn visible_viewport_pages_by_visible_geometry() {
        let rows = flow();
        let mut list = List::default();
        list.first(&rows);

        assert_eq!(
            list.page(&rows, 3, 1, PagingPolicy::visible_viewport()),
            Some(3)
        );
        assert_eq!(list.selected, Some(3));
        assert_eq!(list.offset, 1);
    }
}
