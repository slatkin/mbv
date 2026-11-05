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
