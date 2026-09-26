//! Shared cursor arithmetic over an ordered [`RowFlow`](super::RowFlow).

use super::RowFlow;

/// Primitive cursor state supplied by a list shape.
///
/// The shape owns the selected target; the default methods below own all
/// movement arithmetic. The setter borrows a target so the seam itself only
/// needs `Target: Eq`, leaving any owned-target cloning to the adapter.
pub trait Cursored<Target: Eq> {
    /// The currently selected stable target, if any.
    fn selected_target(&self) -> Option<&Target>;

    /// Replace the selected target. `None` means that no selectable row is
    /// selected.
    fn set_selected_target(&mut self, target: Option<&Target>);

    /// The selected row's position in the complete flow, if it is present.
    fn index(&self, flow: &RowFlow<Target>) -> Option<usize> {
        self.selected_target()
            .and_then(|target| flow.position_of(target))
    }

    /// Select a target, falling back when it is absent from `flow`.
    ///
    /// An absent target falls back to the first selectable row, giving content
    /// replacement one deterministic recovery rule without translating the
    /// old selection's numeric position.
    fn select_target(&mut self, flow: &RowFlow<Target>, target: &Target) -> bool {
        if flow.position_of(target).is_some() {
            self.set_selected_target(Some(target));
            true
        } else {
            self.first(flow);
            false
        }
    }

    /// Select the first selectable row, or clear selection for an empty flow.
    fn first(&mut self, flow: &RowFlow<Target>) -> Option<usize> {
        let position = flow.position_of_selectable(0);
        self.set_selected_target(position.and_then(|position| flow.target_at(position)));
        position
    }

    /// Select the last selectable row, or clear selection for an empty flow.
    fn last(&mut self, flow: &RowFlow<Target>) -> Option<usize> {
        let position = flow
            .selectable_len()
            .checked_sub(1)
            .and_then(|ordinal| flow.position_of_selectable(ordinal));
        self.set_selected_target(position.and_then(|position| flow.target_at(position)));
        position
    }

    /// Select the `ordinal`th selectable row, clamping to the last row.
    fn select_selectable_ordinal(
        &mut self,
        flow: &RowFlow<Target>,
        ordinal: usize,
    ) -> Option<usize> {
        let Some(last) = flow.selectable_len().checked_sub(1) else {
            self.set_selected_target(None);
            return None;
        };
        let position = flow.position_of_selectable(ordinal.min(last));
        self.set_selected_target(position.and_then(|position| flow.target_at(position)));
        position
    }

    /// Move by a selectable-row distance, clamping at both ends.
    ///
    /// Structural rows are skipped because movement is calculated over the
    /// selectable ordinal rather than over raw flow positions. A stale or
    /// absent selection is repaired deterministically from the requested
    /// direction.
    fn move_by(&mut self, flow: &RowFlow<Target>, delta: isize) -> Option<usize> {
        let selectable_len = flow.selectable_len();
        if selectable_len == 0 {
            self.set_selected_target(None);
            return None;
        }

        let current_ordinal = self
            .index(flow)
            .and_then(|position| flow.selectable_ordinal_at(position));
        let ordinal = match current_ordinal {
            Some(current) => shift_clamped(current, delta, selectable_len),
            None if delta < 0 => selectable_len - 1,
            None => 0,
        };
        self.select_selectable_ordinal(flow, ordinal)
    }
}

fn shift_clamped(current: usize, delta: isize, len: usize) -> usize {
    let last = len - 1;
    if delta.is_negative() {
        current.saturating_sub(delta.unsigned_abs())
    } else {
        current.saturating_add(delta.unsigned_abs()).min(last)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::Cursored;
    use crate::app::components::list::{Row, RowFlow, TestListState};

    fn flow() -> RowFlow<u8> {
        RowFlow::new(vec![
            Row::selectable(1),
            Row::structural(),
            Row::selectable(2),
            Row::structural(),
            Row::selectable(3),
        ])
    }

    #[rstest]
    #[case(-1, Some(0), Some(1))]
    #[case(1, Some(0), Some(2))]
    #[case(99, Some(4), Some(3))]
    fn movement_skips_structural_rows_and_clamps(
        #[case] delta: isize,
        #[case] starting_position: Option<usize>,
        #[case] expected_target: Option<u8>,
    ) {
        let rows = flow();
        let mut cursor = TestListState::default();
        cursor.first(&rows);
        if let Some(position) = starting_position {
            cursor.select_target(&rows, rows.row_at(position).and_then(Row::target).unwrap());
        }
        cursor.move_by(&rows, delta);
        assert_eq!(cursor.selected, expected_target);
        assert!(cursor.index(&rows).is_some_and(|position| {
            rows.row_at(position)
                .is_some_and(|row| !row.is_structural())
        }));
    }

    #[test]
    fn first_and_last_skip_structural_rows_without_wrap() {
        let rows = flow();
        let mut cursor = TestListState::default();

        assert_eq!(cursor.first(&rows), Some(0));
        assert_eq!(cursor.move_by(&rows, -1), Some(0));
        assert_eq!(cursor.last(&rows), Some(4));
        assert_eq!(cursor.move_by(&rows, 1), Some(4));
    }

    #[test]
    fn absent_target_falls_back_to_first_selectable_row() {
        let rows = flow();
        let mut cursor = TestListState {
            selected: Some(2),
            ..Default::default()
        };

        assert!(!cursor.select_target(&rows, &99));
        assert_eq!(cursor.selected_target(), Some(&1));
        assert_eq!(cursor.index(&rows), Some(0));
    }

    #[rstest]
    #[case(-1, Some(4), Some(3))]
    #[case(1, Some(0), Some(1))]
    fn movement_repairs_selection_for_rows_absent_from_flow(
        #[case] delta: isize,
        #[case] expected_position: Option<usize>,
        #[case] expected_target: Option<u8>,
    ) {
        let rows = flow();
        let mut cursor = TestListState {
            selected: Some(99),
            ..Default::default()
        };

        assert_eq!(cursor.move_by(&rows, delta), expected_position);
        assert_eq!(cursor.selected, expected_target);
    }

    #[test]
    fn selecting_an_ordinal_in_an_empty_flow_clears_stale_selection() {
        let mut cursor = TestListState {
            selected: Some(2),
            ..Default::default()
        };

        assert_eq!(
            cursor.select_selectable_ordinal(&RowFlow::new(vec![]), 0),
            None
        );
        assert_eq!(cursor.selected, None);
    }
}
