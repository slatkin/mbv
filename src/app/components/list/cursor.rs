//! Shared cursor arithmetic over an ordered [`RowFlow`](super::RowFlow).

use super::{Row, RowFlow};

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

    /// Select a target that is present in `flow`.
    ///
    /// An absent target is not reachable and clears stale selection rather
    /// than leaving a selection that no longer belongs to the flow.
    fn select_target(&mut self, flow: &RowFlow<Target>, target: &Target) -> bool {
        if flow.position_of(target).is_some() {
            self.set_selected_target(Some(target));
            true
        } else {
            self.set_selected_target(None);
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
    fn select_index(&mut self, flow: &RowFlow<Target>, ordinal: usize) -> Option<usize> {
        let last = flow.selectable_len().checked_sub(1)?;
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
            .and_then(|position| ordinal_at(flow, position));
        let ordinal = match current_ordinal {
            Some(current) => shift_clamped(current, delta, selectable_len),
            None if delta < 0 => selectable_len - 1,
            None => 0,
        };
        self.select_index(flow, ordinal)
    }

    /// Alias named for adapters that expose cursor movement as selection.
    fn move_selection(&mut self, flow: &RowFlow<Target>, delta: isize) -> Option<usize> {
        self.move_by(flow, delta)
    }

    /// Alias for [`Cursored::first`].
    fn select_first(&mut self, flow: &RowFlow<Target>) -> Option<usize> {
        self.first(flow)
    }

    /// Alias for [`Cursored::last`].
    fn select_last(&mut self, flow: &RowFlow<Target>) -> Option<usize> {
        self.last(flow)
    }
}

fn ordinal_at<Target: Eq>(flow: &RowFlow<Target>, position: usize) -> Option<usize> {
    flow.rows()[..=position]
        .iter()
        .filter(|row| matches!(row, Row::Selectable(_)))
        .count()
        .checked_sub(1)
}

fn shift_clamped(current: usize, delta: isize, len: usize) -> usize {
    let last = len - 1;
    if delta.is_negative() {
        current.saturating_sub(delta.unsigned_abs())
    } else {
        current.saturating_add(delta as usize).min(last)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::Cursored;
    use crate::app::components::list::{Row, RowFlow};

    #[derive(Default)]
    struct Cursor {
        selected: Option<u8>,
    }

    impl Cursored<u8> for Cursor {
        fn selected_target(&self) -> Option<&u8> {
            self.selected.as_ref()
        }

        fn set_selected_target(&mut self, target: Option<&u8>) {
            self.selected = target.copied();
        }
    }

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
        let mut cursor = Cursor::default();
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
        let mut cursor = Cursor::default();

        assert_eq!(cursor.first(&rows), Some(0));
        assert_eq!(cursor.move_by(&rows, -1), Some(0));
        assert_eq!(cursor.last(&rows), Some(4));
        assert_eq!(cursor.move_by(&rows, 1), Some(4));
    }

    #[test]
    fn absent_targets_are_not_selectable() {
        let rows = flow();
        let mut cursor = Cursor { selected: Some(2) };

        assert!(!cursor.select_target(&rows, &99));
        assert_eq!(cursor.selected_target(), None);
        assert_eq!(cursor.index(&rows), None);
    }
}
